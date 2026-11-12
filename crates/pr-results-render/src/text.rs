// Small helpers for putting `Safe` fragments together without making something new of them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use pr_results_core::{Safe, Source};

/// A literal written in the program.
pub(crate) fn t(literal: &'static str) -> Safe {
	Safe::trusted(literal)
}

/// A count, with no grouping: for small numbers such as "Failure 3 of 12".
pub(crate) fn num(n: usize) -> Safe {
	Safe::number(big(n))
}

/// A count with a thin space between groups of three digits.
pub(crate) fn count(n: u64) -> Safe {
	Safe::grouped(n)
}

pub(crate) fn big(n: usize) -> u64 {
	u64::try_from(n).unwrap_or(u64::MAX)
}

/// The size of `text` against a limit: the larger of its characters and its UTF-16 units.
pub(crate) fn measure(text: &Safe) -> usize {
	text.len_chars().max(text.as_str().encode_utf16().count())
}

/// One of two literals by count: "1 failure", "2 failures".
pub(crate) fn noun(n: u64, one: &'static str, many: &'static str) -> Safe {
	t(if n == 1 { one } else { many })
}

/// Text with its own count of characters cut off, for text a renderer has cut by lines.
pub(crate) struct Piece {
	pub text: String,
	pub omitted: usize,
}

impl Source for Piece {
	fn raw(&self) -> &str {
		&self.text
	}
	fn omitted(&self) -> usize {
		self.omitted
	}
}

/// Where a reference such as `GH-123` or `ABC\-123` starts in `chars`: a letter or digit, an
/// optional backslash, a hyphen and a digit. Returns the position after each one found, so a
/// caller can tell whether it crosses a boundary.
fn reference_ends(chars: &[char]) -> Vec<(usize, usize)> {
	let mut found = Vec::new();
	for (start, c) in chars.iter().enumerate() {
		if !c.is_alphanumeric() {
			continue;
		}
		let mut at = start + 1;
		if chars.get(at) == Some(&'\\') {
			at += 1;
		}
		if chars.get(at) == Some(&'-') && chars.get(at + 1).is_some_and(char::is_ascii_digit) {
			found.push((start, at + 2));
		}
	}
	found
}

/// Whether gluing `left` to `right` would build a reference that neither holds alone.
///
/// `Safe::text` breaks `GH-123` inside one fragment, but it cannot see the fragment next to it:
/// `GH` followed by `-123` is two safe pieces and one issue link. The check looks only at the
/// characters around the join.
fn builds_reference(left: &str, right: &str) -> bool {
	let tail: Vec<char> = {
		let mut v: Vec<char> = left.chars().rev().take(3).collect();
		v.reverse();
		v
	};
	let head: Vec<char> = right.chars().take(3).collect();
	let boundary = tail.len();
	let mut both = tail;
	both.extend(head);
	reference_ends(&both)
		.into_iter()
		.any(|(start, end)| start < boundary && end > boundary)
}

/// Joins fragments. If two neighbours would form a reference between them, a space is put
/// between: visible, and harmless in the places this is used. Every line the renderer writes is
/// built here, never with `Safe::concat` or `Safe::join` directly.
pub(crate) fn cat(parts: &[Safe]) -> Safe {
	let space = t(" ");
	let mut out: Vec<&Safe> = Vec::with_capacity(parts.len() * 2);
	let mut previous: Option<&Safe> = None;
	for part in parts {
		if part.is_empty() {
			continue;
		}
		if let Some(left) = previous {
			if builds_reference(left.as_str(), part.as_str()) {
				out.push(&space);
			}
		}
		out.push(part);
		previous = Some(part);
	}
	let owned: Vec<Safe> = out.into_iter().cloned().collect();
	Safe::concat(&owned)
}

/// `cat` with a separator between the parts.
pub(crate) fn join(separator: &Safe, parts: &[Safe]) -> Safe {
	let mut all: Vec<Safe> = Vec::with_capacity(parts.len() * 2);
	for (i, part) in parts.iter().enumerate() {
		if i > 0 {
			all.push(separator.clone());
		}
		all.push(part.clone());
	}
	cat(&all)
}

/// Whether the heading may write `title` as ordinary text on Forgejo. The heading puts `: `
/// after the title, so that is checked with it: `x:smile` would become `:smile:`.
pub(crate) fn forgejo_inert(title: &str) -> bool {
	let chars: Vec<char> = title.chars().chain([':', ' ']).collect();
	if chars.contains(&'$') {
		return false;
	}
	// `Safe::text` writes the colon of `://` as an entity, which decodes before Forgejo looks for
	// a link, so a scheme or a `www.` start would autolink.
	let lower: String = chars.iter().collect::<String>().to_ascii_lowercase();
	if lower.contains("://") || lower.contains("www.") {
		return false;
	}
	if chars
		.iter()
		.zip(chars.iter().skip(1))
		.any(|(a, b)| *a == '!' && b.is_ascii_digit())
	{
		return false;
	}
	let mut run = 0usize;
	for c in &chars {
		run = if c.is_ascii_hexdigit() { run + 1 } else { 0 };
		if run >= 7 {
			return false;
		}
	}
	let word = |c: &char| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-');
	for (at, c) in chars.iter().enumerate() {
		if *c != ':' {
			continue;
		}
		let inner = chars.iter().skip(at + 1).take_while(|c| word(c)).count();
		if inner > 0 && chars.get(at + 1 + inner) == Some(&':') {
			return false;
		}
	}
	true
}

/// Lines of Markdown with at most one blank line between blocks.
#[derive(Default)]
pub(crate) struct Doc {
	lines: Vec<Safe>,
}

impl Doc {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn line(&mut self, line: Safe) {
		self.lines.push(line);
	}

	pub fn blank(&mut self) {
		if self.lines.last().is_some_and(|l| !l.is_empty()) {
			self.lines.push(Safe::default());
		}
	}

	pub fn lines(&mut self, lines: Vec<Safe>) {
		self.lines.extend(lines);
	}

	/// A block of text on its own: a blank line before it, then its lines.
	pub fn block(&mut self, line: Safe) {
		self.blank();
		self.lines.push(line);
	}

	pub fn build(mut self) -> Safe {
		while self.lines.last().is_some_and(Safe::is_empty) {
			self.lines.pop();
		}
		self.lines.push(Safe::default());
		join(&t("\n"), &self.lines)
	}
}

#[cfg(test)]
#[allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]
mod tests {
	use super::*;

	fn text(s: &str) -> Safe {
		Safe::text(s)
	}

	#[test]
	fn pieces_that_would_build_a_reference_are_kept_apart() {
		// `Safe::text` is safe on each of these alone and unsafe once they are glued.
		let cases = [("GH", "-123"), ("GH-", "123"), ("ABC", "-1"), ("x9", "-0")];
		for (left, right) in cases {
			let (l, r) = (text(left), text(right));
			let glued = Safe::concat(&[l.clone(), r.clone()]);
			assert!(
				builds_reference(l.as_str(), r.as_str()),
				"{left:?} + {right:?}: {:?}",
				glued.as_str()
			);
			let joined = cat(&[l, r]);
			assert!(
				joined.as_str().contains(' '),
				"{left:?} + {right:?} was glued: {:?}",
				joined.as_str()
			);
		}
	}

	#[test]
	fn pieces_that_are_already_apart_are_left_alone() {
		let cases = [
			("GH", ": 123"),
			("GH ", "-123"),
			("GH-", " 123"),
			("Failure ", "12"),
			("1", "-"),
			("a-b", "1"),
			("", "-1"),
			("a", ""),
		];
		for (left, right) in cases {
			let (l, r) = (text(left), text(right));
			assert!(
				!builds_reference(l.as_str(), r.as_str()),
				"{left:?} {right:?}"
			);
			let joined = cat(&[l.clone(), r.clone()]);
			assert_eq!(
				joined.as_str(),
				format!("{}{}", l.as_str(), r.as_str()),
				"{left:?} {right:?}"
			);
		}
	}

	#[test]
	fn a_reference_inside_one_piece_is_not_the_joiners_to_mend() {
		// Inside one `Safe::text` the hyphen is already fullwidth; the join adds nothing.
		let one = text("GH-123");
		assert_eq!(cat(&[one.clone(), t(" ok")]).as_str(), format!("{one} ok"));
	}

	#[test]
	fn the_guard_works_through_nested_joins() {
		let inner = cat(&[text("G"), text("H")]);
		let outer = cat(&[inner, text("-123")]);
		assert!(outer.as_str().contains(' '), "{:?}", outer.as_str());
	}

	#[test]
	fn doc_collapses_blank_lines_and_ends_with_one_newline() {
		let mut doc = Doc::new();
		doc.line(t("a"));
		doc.blank();
		doc.blank();
		doc.block(t("b"));
		doc.blank();
		assert_eq!(doc.build().as_str(), "a\n\nb\n");
	}

	#[test]
	fn measure_counts_utf16_units() {
		let s = Safe::text("😀😀");
		assert_eq!(s.len_chars(), 2);
		assert_eq!(measure(&s), 4);
	}
}
