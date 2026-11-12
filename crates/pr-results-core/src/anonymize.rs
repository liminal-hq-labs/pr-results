// Rewrites a JUnit file so that it keeps its shape and none of its words.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A report from a private project can become a public fixture once every word in it is replaced
//! by a token. The structure is kept: the elements, the attribute names, the counts, the times, the
//! line and column numbers, and the punctuation and layout around the words, so a stack frame is
//! still `at wA.wB (/wC/wD.wE:12:5)`. The same word becomes the same token everywhere, so a class
//! name, a file attribute and a stack frame still agree.
//!
//! A token is a hash of the word and the salt, shortened. It hides a word from a reader, not from
//! someone who can guess it: with the default (empty) salt, a candidate name can be hashed and
//! compared. A secret salt closes that, and the output stays deterministic for that salt.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::ParseError;

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

/// Attributes whose value is a number the reader uses.
const NUMERIC_ATTRIBUTES: &[&str] = &[
	"time",
	"tests",
	"failures",
	"errors",
	"skipped",
	"assertions",
	"line",
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
					self.out.push_str(e.name().as_ref());
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
		self.out.push_str(&name);
		for attr in e.attributes() {
			let attr = attr.map_err(|_| ParseError::Malformed { offset: 0 })?;
			let key = attr.key.as_ref().to_owned();
			let value = attr
				.normalized_value(XmlVersion::Implicit1_0)
				.map_err(|_| ParseError::BadEntity)?;
			let shown = self.attribute(&name, &key, &value);
			self.out.push(' ');
			self.out.push_str(&key);
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
		let numeric = |v: &str| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.');
		match key {
			k if NUMERIC_ATTRIBUTES.contains(&k) && numeric(value) => value.to_owned(),
			"timestamp" => FIXED_TIMESTAMP.to_owned(),
			"name" if self.depth == 0 && KEPT_ROOT_NAMES.contains(&value) => value.to_owned(),
			"type" if KEPT_TYPES.contains(&value) => value.to_owned(),
			"hostname" if KEPT_PROJECTS.contains(&value) => value.to_owned(),
			// Playwright writes `id=""` on its root, and the reader looks for it.
			"id" if value.is_empty() && element == "testsuites" => String::new(),
			_ => scrub(value, self.salt),
		}
	}
}

/// Replaces every word of `text` with its token and keeps everything between the words.
fn scrub(text: &str, salt: &str) -> String {
	let mut out = String::with_capacity(text.len());
	let mut word = String::new();
	for c in text.chars() {
		if c.is_alphanumeric() || c == '_' {
			word.push(c);
			continue;
		}
		push_word(&mut out, &mut word, salt);
		out.push(c);
	}
	push_word(&mut out, &mut word, salt);
	out
}

fn push_word(out: &mut String, word: &mut String, salt: &str) {
	if word.is_empty() {
		return;
	}
	let short_number = word.len() <= 4 && word.bytes().all(|b| b.is_ascii_digit());
	if short_number || KEPT_WORDS.contains(&word.as_str()) {
		out.push_str(word);
	} else {
		out.push_str(&token(word, salt));
	}
	word.clear();
}

/// `w` and ten hex digits: a keyed hash of the word, so one word is one token and a token says
/// nothing of the word's length or letters.
fn token(word: &str, salt: &str) -> String {
	let mut h: u64 = 0xcbf2_9ce4_8422_2325;
	for b in salt.bytes().chain([0xff]).chain(word.bytes()) {
		h ^= u64::from(b);
		h = h.wrapping_mul(0x0100_0000_01b3);
	}
	// A final mix so that words that differ in a last letter differ in every digit.
	h ^= h >> 33;
	h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
	h ^= h >> 33;
	h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
	h ^= h >> 33;
	format!("w{:010x}", h & 0xff_ffff_ffff)
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
