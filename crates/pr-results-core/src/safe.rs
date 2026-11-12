// The one type through which untrusted text reaches output.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt;

use crate::model::Untrusted;

/// Text that is safe to write into a Markdown comment, a check run or a workflow log.
///
/// A `Safe` can only be made by escaping, from a literal that lives in the program (`trusted`) or
/// from a number, so a renderer that accepts only `Safe` cannot emit a raw string. Every
/// constructor that takes untrusted text takes a size budget, and cutting is always visible: the
/// result ends with a marker that says how many characters were left out.
///
/// The Markdown constructors produce inline text. A caller that places it at the start of a line
/// still gets a safe result, because leading spaces are written as entities, but the line is the
/// caller's to lay out.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Safe(String);

/// Anything untrusted text can be read from: a `str`, a `String` or an [`Untrusted`].
pub trait Source {
	fn raw(&self) -> &str;
	/// Characters already cut off before the text reached the renderer.
	fn omitted(&self) -> usize {
		0
	}
}

impl Source for str {
	fn raw(&self) -> &str {
		self
	}
}

impl Source for String {
	fn raw(&self) -> &str {
		self
	}
}

impl Source for Untrusted {
	fn raw(&self) -> &str {
		self.as_str()
	}
	fn omitted(&self) -> usize {
		Untrusted::omitted(self)
	}
}

/// The budget used by constructors that do not take one.
pub const DEFAULT_MAX_CHARS: usize = 4096;

/// What one input character becomes before escaping.
enum Clean {
	Drop,
	Space,
	/// A character that could reorder or hide text: shown as `\u{…}`.
	Show,
	Keep,
}

fn classify(c: char) -> Clean {
	match c {
		'\n' | '\r' | '\t' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}' => Clean::Space,
		'\0'..='\u{1f}' | '\u{7f}'..='\u{9f}' => Clean::Drop,
		'\u{200b}'..='\u{200f}'
		| '\u{202a}'..='\u{202e}'
		| '\u{2060}'..='\u{2064}'
		| '\u{2066}'..='\u{2069}'
		| '\u{061c}'
		| '\u{180e}'
		| '\u{feff}' => Clean::Show,
		_ => Clean::Keep,
	}
}

fn shown(c: char) -> String {
	format!("\\u{{{:x}}}", u32::from(c))
}

/// Writes `c` as inline Markdown and HTML text. `prev` and `next` are the neighbouring cleaned
/// characters, used to break up `::` and `://`.
fn push_markdown(
	out: &mut String,
	c: char,
	prev: Option<char>,
	next: Option<char>,
	at_start: bool,
) {
	match c {
		'&' => out.push_str("&amp;"),
		'<' => out.push_str("&lt;"),
		'>' => out.push_str("&gt;"),
		'"' => out.push_str("&quot;"),
		'\'' => out.push_str("&#39;"),
		'@' => out.push_str("&#64;"),
		'#' => out.push_str("&#35;"),
		' ' if at_start => out.push_str("&#32;"),
		':' if prev == Some(':') || next == Some('/') => out.push_str("&#58;"),
		'\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '+' | '-' | '.' | '!'
		| '|' | '~' | '=' | '$' => {
			out.push('\\');
			out.push(c);
		}
		_ => out.push(c),
	}
}

fn markers(omitted: usize) -> String {
	format!("\u{2026} ({omitted} more characters omitted)")
}

impl Safe {
	/// A literal written in the program. The `'static` bound is the point: a string built at run
	/// time cannot be passed here.
	pub fn trusted(literal: &'static str) -> Self {
		Self(literal.to_owned())
	}

	pub fn number(n: u64) -> Self {
		Self(n.to_string())
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}

	pub fn len_chars(&self) -> usize {
		self.0.chars().count()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn concat(parts: &[Safe]) -> Self {
		Self(parts.iter().map(|p| p.0.as_str()).collect())
	}

	pub fn join(separator: &Safe, parts: &[Safe]) -> Self {
		Self(
			parts
				.iter()
				.map(|p| p.0.as_str())
				.collect::<Vec<_>>()
				.join(&separator.0),
		)
	}

	/// Inline Markdown text with the default budget.
	pub fn text<S: Source + ?Sized>(source: &S) -> Self {
		Self::text_max(source, DEFAULT_MAX_CHARS)
	}

	/// Inline Markdown text that is at most `max_chars` characters before the truncation marker.
	///
	/// Escaping happens before measuring, so the cut never lands inside an escape sequence.
	pub fn text_max<S: Source + ?Sized>(source: &S, max_chars: usize) -> Self {
		let mut out = String::new();
		let mut written = 0usize;
		let mut omitted = source.omitted();
		let mut prev: Option<char> = None;
		let mut chars = expand(source.raw()).peekable();
		let mut at_start = true;
		while let Some(c) = chars.next() {
			let mut piece = String::new();
			push_markdown(&mut piece, c, prev, chars.peek().copied(), at_start);
			let n = piece.chars().count();
			if written + n > max_chars {
				omitted = omitted.saturating_add(1 + chars.count());
				break;
			}
			written += n;
			out.push_str(&piece);
			at_start = at_start && c == ' ';
			prev = Some(c);
		}
		if omitted > 0 {
			out.push_str(&markers(omitted));
		}
		Self(out)
	}

	/// A fenced code block. The fence is one backtick longer than the longest run in the content,
	/// and at least four, so the content cannot close it. Line breaks are kept.
	pub fn code_block<S: Source + ?Sized>(source: &S, max_chars: usize) -> Self {
		let mut body = String::new();
		let mut written = 0usize;
		let mut omitted = source.omitted();
		let mut chars = expand_keeping_lines(source.raw());
		while let Some(c) = chars.next() {
			if written + 1 > max_chars {
				omitted = omitted.saturating_add(1 + chars.count());
				break;
			}
			written += 1;
			body.push(c);
		}
		if omitted > 0 {
			if !body.is_empty() && !body.ends_with('\n') {
				body.push('\n');
			}
			body.push_str(&markers(omitted));
		}
		let mut longest = 0usize;
		let mut run = 0usize;
		for c in body.chars() {
			if c == '`' {
				run += 1;
				longest = longest.max(run);
			} else {
				run = 0;
			}
		}
		let fence = "`".repeat((longest + 1).max(4));
		Self(format!("{fence}text\n{body}\n{fence}"))
	}

	/// One line of plain text for a workflow log that is not a command. `::` and `##[` cannot
	/// start a command or a runner directive. Line breaks become spaces.
	pub fn log_line<S: Source + ?Sized>(source: &S, max_chars: usize) -> Self {
		Self::plain(source, max_chars, |c, prev, next, out| match c {
			':' if prev == Some(':') || next == Some(':') => out.push_str("%3A"),
			'#' if next == Some('#') || prev == Some('#') => out.push_str("%23"),
			_ => out.push(c),
		})
	}

	/// The message part of a workflow command such as `::error::message`: `%`, CR and LF are
	/// escaped, as the runner expects.
	pub fn workflow_data<S: Source + ?Sized>(source: &S, max_chars: usize) -> Self {
		Self::workflow(source, max_chars, false)
	}

	/// A property of a workflow command such as `file=` in `::error file=…::`: `%`, CR, LF, `:`
	/// and `,` are escaped.
	pub fn workflow_property<S: Source + ?Sized>(source: &S, max_chars: usize) -> Self {
		Self::workflow(source, max_chars, true)
	}

	fn workflow<S: Source + ?Sized>(source: &S, max_chars: usize, property: bool) -> Self {
		let mut out = String::new();
		let mut written = 0usize;
		let mut omitted = source.omitted();
		let mut chars = source.raw().chars();
		while let Some(c) = chars.next() {
			let piece: String = match c {
				'%' => "%25".into(),
				'\r' => "%0D".into(),
				'\n' => "%0A".into(),
				':' if property => "%3A".into(),
				',' if property => "%2C".into(),
				_ => match classify(c) {
					Clean::Drop => continue,
					Clean::Space => " ".into(),
					Clean::Show => shown(c),
					Clean::Keep => c.to_string(),
				},
			};
			let n = piece.chars().count();
			if written + n > max_chars {
				omitted = omitted.saturating_add(1 + chars.count());
				break;
			}
			written += n;
			out.push_str(&piece);
		}
		if omitted > 0 {
			// The marker has to survive the runner's own unescaping, so it carries no `%`.
			out.push_str(&markers(omitted));
		}
		Self(out)
	}

	fn plain<S: Source + ?Sized>(
		source: &S,
		max_chars: usize,
		write: impl Fn(char, Option<char>, Option<char>, &mut String),
	) -> Self {
		let mut out = String::new();
		let mut written = 0usize;
		let mut omitted = source.omitted();
		let mut prev = None;
		let mut chars = expand(source.raw()).peekable();
		while let Some(c) = chars.next() {
			let mut piece = String::new();
			write(c, prev, chars.peek().copied(), &mut piece);
			let n = piece.chars().count();
			if written + n > max_chars {
				omitted = omitted.saturating_add(1 + chars.count());
				break;
			}
			written += n;
			out.push_str(&piece);
			prev = Some(c);
		}
		if omitted > 0 {
			out.push_str(&markers(omitted));
		}
		Self(out)
	}
}

/// The cleaned characters of `s`, one line: controls dropped, breaks as spaces, bidirectional and
/// zero-width characters spelled out.
fn expand(s: &str) -> impl Iterator<Item = char> + '_ {
	s.chars().flat_map(|c| -> Vec<char> {
		match classify(c) {
			Clean::Drop => Vec::new(),
			Clean::Space => vec![' '],
			Clean::Show => shown(c).chars().collect(),
			Clean::Keep => vec![c],
		}
	})
}

/// Like [`expand`], but line breaks and tabs survive (CR and CRLF become LF).
fn expand_keeping_lines(s: &str) -> impl Iterator<Item = char> + '_ {
	let mut prev_cr = false;
	s.chars().flat_map(move |c| -> Vec<char> {
		let after_cr = std::mem::replace(&mut prev_cr, c == '\r');
		match c {
			'\n' if after_cr => Vec::new(),
			'\n' | '\r' => vec!['\n'],
			'\t' => vec!['\t'],
			_ => match classify(c) {
				Clean::Drop => Vec::new(),
				Clean::Space => vec![' '],
				Clean::Show => shown(c).chars().collect(),
				Clean::Keep => vec![c],
			},
		}
	})
}

impl fmt::Display for Safe {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.0)
	}
}

impl fmt::Debug for Safe {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Safe({:?})", self.0)
	}
}
