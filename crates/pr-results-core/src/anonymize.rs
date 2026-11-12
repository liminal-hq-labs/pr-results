// Rewrites a JUnit file so that it keeps its shape and none of its words.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A report from a private project can become a public fixture once every word in it is replaced
//! by a token. The structure is kept: the JUnit elements and attributes the readers use, the
//! counts, the times, the line and column of a stack frame, and the punctuation and layout around
//! the words, so a stack frame is still `at wA.wB (/wC/wD.wE:12:5)`. The same word becomes the
//! same token everywhere, so a class name, a `file` attribute and a stack frame still agree.
//!
//! A token is the first six bytes of an HMAC-SHA-256 of the word, keyed by the salt, as twelve
//! lowercase hex digits after a one-letter prefix (`w` for a word, `n` for a number, `x` for an
//! element or attribute name). Without the salt a token cannot be tied to a word, even by someone
//! who can guess the word; a salt that is short or guessable can itself be found by trying it
//! against common words. `docs/anonymizing-reports.md` says what else the output still shows.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::ParseError;
use crate::hmac::hmac_sha256;

/// Words that belong to the reporters and to JavaScript, not to the project. They are kept, so the
/// markers the readers look for (`node_modules`, `[[ATTACHMENT|`, `at`) still work.
const KEPT_WORDS: &[&str] = &[
	"at",
	"async",
	"new",
	"node",
	"internal",
	"file",
	"anonymous",
	"node_modules",
	"ATTACHMENT",
	"Retry",
	// nextest's location line is `thread 'name' panicked at path:line:col`.
	"panicked",
];

/// Root names that say which tool wrote the file.
const KEPT_ROOT_NAMES: &[&str] = &["vitest tests", "bun test", "jest tests", "nextest-run"];

/// Error types that name the reporter's own failure kinds.
const KEPT_TYPES: &[&str] = &[
	"Error",
	"AssertionError",
	"TypeError",
	"RangeError",
	"ReferenceError",
	"SyntaxError",
	"EvalError",
	"URIError",
	"FAILURE",
	"ERROR",
	"test failure",
];

/// Browser project names, which Playwright writes as the `hostname`.
const KEPT_PROJECTS: &[&str] = &["chromium", "firefox", "webkit"];

/// The elements the readers use. Any other element name is replaced.
const KEPT_ELEMENTS: &[&str] = &[
	"testsuites",
	"testsuite",
	"testcase",
	"failure",
	"error",
	"skipped",
	"system-out",
	"system-err",
	"properties",
	"property",
	"rerunFailure",
	"flakyFailure",
	"rerunError",
	"flakyError",
	"stack-trace",
];

/// The attributes the readers use. Any other attribute name is replaced.
const KEPT_ATTRIBUTES: &[&str] = &[
	"name",
	"classname",
	"file",
	"line",
	"time",
	"tests",
	"failures",
	"errors",
	"skipped",
	"disabled",
	"timestamp",
	"hostname",
	"id",
	"message",
	"type",
	"package",
	"status",
	"assertions",
	"xmlns",
];

/// Attributes that hold a count or a line: kept when they are up to four digits.
const COUNT_ATTRIBUTES: &[&str] = &[
	"line",
	"tests",
	"failures",
	"errors",
	"skipped",
	"disabled",
	"assertions",
];

const FIXED_TIMESTAMP: &str = "2026-01-01T00:00:00.000Z";

/// The deepest nesting accepted, as for the parser.
const MAX_DEPTH: usize = 64;

/// Anonymizes one JUnit document. The result depends only on `input` and `salt`.
///
/// Comments, processing instructions and the document's own formatting outside text are dropped;
/// everything else is kept in shape. An input that is not well formed, declares a document type or
/// an entity, or is larger than the parser's limit is an error, as it is for the parser.
pub fn anonymize_junit(input: &[u8], salt: &str) -> Result<String, ParseError> {
	let limit = crate::Limits::default().max_bytes;
	if input.len() > limit {
		return Err(ParseError::TooLarge {
			len: input.len(),
			limit,
		});
	}
	let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
	let text = String::from_utf8_lossy(input);
	if text.trim().is_empty() {
		return Err(ParseError::Empty);
	}
	if let Some(offset) = text.find('\0') {
		return Err(ParseError::Malformed {
			offset: offset as u64,
		});
	}
	let mut job = Job {
		salt,
		out: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
		pending: String::new(),
		depth: 0,
		roots: 0,
	};
	job.run(&text)?;
	Ok(job.out)
}

struct Job<'s> {
	salt: &'s str,
	out: String,
	/// Text read since the last tag, written scrubbed when the next tag arrives.
	pending: String,
	depth: usize,
	roots: usize,
}

impl Job<'_> {
	fn run(&mut self, text: &str) -> Result<(), ParseError> {
		let mut reader = Reader::from_str(text);
		loop {
			let event = match reader.read_event() {
				Ok(event) => event,
				Err(quick_xml::errors::Error::Syntax(_)) => return Err(ParseError::Truncated),
				Err(quick_xml::errors::Error::Escape(_)) => return Err(ParseError::BadEntity),
				Err(_) => {
					return Err(ParseError::Malformed {
						offset: reader.error_position(),
					})
				}
			};
			match event {
				Event::Eof => break,
				Event::DocType(_) => return Err(ParseError::Dtd),
				Event::Decl(decl) => {
					if let Some(Ok(encoding)) = decl.encoding() {
						if !encoding.eq_ignore_ascii_case("utf-8") {
							return Err(ParseError::UnsupportedEncoding);
						}
					}
				}
				Event::Comment(_) | Event::PI(_) => {}
				Event::Start(e) => {
					self.flush()?;
					self.open(&e, false)?;
				}
				Event::Empty(e) => {
					self.flush()?;
					self.open(&e, true)?;
				}
				Event::End(e) => {
					self.flush()?;
					self.depth = self
						.depth
						.checked_sub(1)
						.ok_or(ParseError::Structure("a closing tag with no opening tag"))?;
					self.out.push_str("</");
					let shown = self.name(e.name().as_ref(), KEPT_ELEMENTS);
					self.out.push_str(&shown);
					self.out.push('>');
				}
				Event::Text(t) => {
					let content = t.xml_content(XmlVersion::Implicit1_0);
					self.text(&content)?;
				}
				Event::CData(c) => {
					let content = c.xml_content(XmlVersion::Implicit1_0);
					self.text(&content)?;
				}
				Event::GeneralRef(r) => {
					let resolved = match r.resolve_char_ref() {
						Ok(Some(c)) => c,
						Ok(None) => match &*r {
							"lt" => '<',
							"gt" => '>',
							"amp" => '&',
							"apos" => '\'',
							"quot" => '"',
							_ => return Err(ParseError::BadEntity),
						},
						Err(_) => return Err(ParseError::BadEntity),
					};
					let mut buf = [0u8; 4];
					self.text(resolved.encode_utf8(&mut buf))?;
				}
			}
		}
		if self.depth != 0 || self.roots == 0 {
			return Err(ParseError::Truncated);
		}
		Ok(())
	}

	fn text(&mut self, content: &str) -> Result<(), ParseError> {
		if self.depth == 0 {
			return if content.trim().is_empty() {
				Ok(())
			} else {
				Err(ParseError::Structure("text outside the root element"))
			};
		}
		self.pending.push_str(content);
		Ok(())
	}

	fn flush(&mut self) -> Result<(), ParseError> {
		if self.pending.is_empty() {
			return Ok(());
		}
		let text = std::mem::take(&mut self.pending);
		let scrubbed = scrub(&text, self.salt);
		escape_into(&mut self.out, &scrubbed, false);
		Ok(())
	}

	fn open(&mut self, e: &BytesStart<'_>, empty: bool) -> Result<(), ParseError> {
		if self.depth >= MAX_DEPTH {
			return Err(ParseError::TooDeep { limit: MAX_DEPTH });
		}
		if self.depth == 0 {
			self.roots += 1;
			if self.roots > 1 {
				return Err(ParseError::Structure("more than one root element"));
			}
		}
		let name = e.name().as_ref().to_owned();
		self.out.push('<');
		let shown = self.name(&name, KEPT_ELEMENTS);
		self.out.push_str(&shown);
		for attr in e.attributes() {
			let attr = attr.map_err(|_| ParseError::Malformed { offset: 0 })?;
			let key = attr.key.as_ref().to_owned();
			let value = attr
				.normalized_value(XmlVersion::Implicit1_0)
				.map_err(|_| ParseError::BadEntity)?;
			let shown = self.attribute(&name, &key, &value);
			self.out.push(' ');
			let shown_key = self.name(&key, KEPT_ATTRIBUTES);
			self.out.push_str(&shown_key);
			self.out.push_str("=\"");
			escape_into(&mut self.out, &shown, true);
			self.out.push('"');
		}
		if empty {
			self.out.push_str("/>");
		} else {
			self.out.push('>');
			self.depth += 1;
		}
		Ok(())
	}

	/// What an attribute's value becomes.
	fn attribute(&self, element: &str, key: &str, value: &str) -> String {
		let digits = |v: &str, most: usize| {
			!v.is_empty() && v.len() <= most && v.bytes().all(|b| b.is_ascii_digit())
		};
		match key {
			k if COUNT_ATTRIBUTES.contains(&k) && digits(value, 4) => value.to_owned(),
			"time" if is_seconds(value) => value.to_owned(),
			"timestamp" => FIXED_TIMESTAMP.to_owned(),
			"name" if self.depth == 0 && KEPT_ROOT_NAMES.contains(&value) => value.to_owned(),
			"type" if KEPT_TYPES.contains(&value) => value.to_owned(),
			"hostname" if KEPT_PROJECTS.contains(&value) => value.to_owned(),
			// Playwright writes `id=""` on its root, and the reader looks for it.
			"id" if value.is_empty() && element == "testsuites" => String::new(),
			_ => scrub(value, self.salt),
		}
	}

	/// An element or attribute name as it is written out: the readers' vocabulary as it is, any
	/// other name (and any namespace prefix) as a token that is still a valid XML name.
	fn name(&self, name: &str, kept: &[&str]) -> String {
		if kept.contains(&name) {
			return name.to_owned();
		}
		match name.split_once(':') {
			Some(("xmlns", local)) => format!("xmlns:{}", token('x', local, self.salt)),
			Some((prefix, local)) => {
				let local = if kept.contains(&local) {
					local.to_owned()
				} else {
					token('x', local, self.salt)
				};
				format!("{}:{local}", token('x', prefix, self.salt))
			}
			None => token('x', name, self.salt),
		}
	}
}

/// A time in seconds: up to four digits, then optionally a point and up to twelve.
fn is_seconds(v: &str) -> bool {
	let (whole, fraction) = v.split_once('.').unwrap_or((v, ""));
	!whole.is_empty()
		&& whole.len() <= 4
		&& fraction.len() <= 12
		&& whole
			.bytes()
			.chain(fraction.bytes())
			.all(|b| b.is_ascii_digit())
}

/// Replaces every word and number of `text` with its token and keeps everything between them,
/// except the line and column that end a stack frame, a panic location or a Playwright heading
/// (see [`position_starts`]).
fn scrub(text: &str, salt: &str) -> String {
	let mut out = String::with_capacity(text.len());
	for line in text.split_inclusive('\n') {
		scrub_line(&mut out, line, salt);
	}
	out
}

fn scrub_line(out: &mut String, line: &str, salt: &str) {
	let kept = position_starts(line);
	let mut word = String::new();
	let mut start = 0;
	for (at, c) in line.char_indices() {
		if c.is_alphanumeric() || c == '_' {
			if word.is_empty() {
				start = at;
			}
			word.push(c);
			continue;
		}
		push_word(out, &mut word, kept.contains(&start), salt);
		out.push(c);
	}
	push_word(out, &mut word, kept.contains(&start), salt);
}

fn push_word(out: &mut String, word: &mut String, keep_number: bool, salt: &str) {
	if word.is_empty() {
		return;
	}
	if word.bytes().all(|b| b.is_ascii_digit()) {
		if keep_number {
			out.push_str(word);
		} else {
			out.push_str(&token('n', word, salt));
		}
	} else if KEPT_WORDS.contains(&word.as_str()) {
		out.push_str(word);
	} else {
		out.push_str(&token('w', word, salt));
	}
	word.clear();
}

/// Where the numbers that are positions start in `line`: the line and column at the end of
///
/// - a stack frame (the line starts with `at ` or `❯`): `at fn (src/a.js:12:5)`,
/// - a panic location (the line holds `panicked at `): `panicked at src/a.rs:12:5:`,
/// - each part of a Playwright heading (`[chromium] › a.spec.js:24:4 › titles`),
///
/// when the text before the colon looks like a file name (a dot, and letters after the last
/// dot, so that `10.0.0.12:5432` and `localhost:3000` are not positions). A number of five digits
/// or more is never a position. Everything else that is a number is replaced, because a port, an
/// address or an identifier is as private as a word.
fn position_starts(line: &str) -> Vec<usize> {
	let mut starts = Vec::new();
	let lead = line.len() - line.trim_start().len();
	let trimmed = line.trim();
	let end = lead + trimmed.len();
	if trimmed.starts_with("at ")
		|| trimmed.starts_with('\u{276f}')
		|| trimmed.contains("panicked at ")
	{
		let from = trimmed
			.rfind(char::is_whitespace)
			.map_or(lead, |i| lead + i + 1);
		trailing_positions(line, from, end, &mut starts);
	} else if trimmed.starts_with('[') && trimmed.contains(" \u{203a} ") {
		let mut from = lead;
		for part in trimmed.split(" \u{203a} ") {
			let to = from + part.len();
			trailing_positions(line, from, to, &mut starts);
			from = to + " \u{203a} ".len();
		}
	}
	starts
}

/// The positions at the end of `line[start..end]`, as `:line:col` or `:line`, pushed to `out`.
fn trailing_positions(line: &str, start: usize, mut end: usize, out: &mut Vec<usize>) {
	let bytes = line.as_bytes();
	while end > start && matches!(bytes.get(end - 1), Some(b')' | b':' | b',')) {
		end -= 1;
	}
	// The start of the run of up to four digits that ends at `to`.
	let digits_before = |to: usize| -> Option<usize> {
		let mut from = to;
		while from > start && bytes.get(from - 1).is_some_and(u8::is_ascii_digit) {
			from -= 1;
		}
		(to > from && to - from <= 4).then_some(from)
	};
	let Some(last) = digits_before(end) else {
		return;
	};
	if last <= start || bytes.get(last - 1) != Some(&b':') {
		return;
	}
	let mut path_end = last - 1;
	let mut first = None;
	if let Some(s) = digits_before(path_end) {
		if s > start && bytes.get(s - 1) == Some(&b':') {
			first = Some(s);
			path_end = s - 1;
		}
	}
	let Some(path) = line.get(start..path_end) else {
		return;
	};
	let extension = path.rsplit_once('.').map(|(_, e)| e);
	if !extension.is_some_and(|e| e.bytes().any(|b| b.is_ascii_alphabetic())) {
		return;
	}
	out.extend(first);
	out.push(last);
}

/// A prefix letter and twelve hex digits: the first six bytes of the HMAC-SHA-256 of `word`
/// under the salt. The same word always gives the same token, a token says nothing of the word's
/// length or letters, and without the salt it cannot be tied to a word.
fn token(prefix: char, word: &str, salt: &str) -> String {
	let mac = hmac_sha256(salt.as_bytes(), word.as_bytes());
	let mut out = String::with_capacity(13);
	out.push(prefix);
	for byte in mac.iter().take(6) {
		out.push_str(&format!("{byte:02x}"));
	}
	out
}

/// Writes `text` as XML character data (or an attribute value), with characters XML 1.0 forbids
/// replaced by U+FFFD.
fn escape_into(out: &mut String, text: &str, attribute: bool) {
	for c in text.chars() {
		match c {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' if attribute => out.push_str("&quot;"),
			'\n' if attribute => out.push_str("&#10;"),
			'\t' if attribute => out.push_str("&#9;"),
			'\r' => out.push_str("&#13;"),
			'\t' | '\n' => out.push(c),
			c if (c as u32) < 0x20 || matches!(c as u32, 0xfffe | 0xffff) => out.push('\u{fffd}'),
			c => out.push(c),
		}
	}
}
