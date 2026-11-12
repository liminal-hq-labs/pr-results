// A small glob matcher for artefact names and file paths: `*`, `**` and `?`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Settings name artefacts and files by pattern. This is the whole language, so the matcher stays
//! small and has no dependency:
//!
//! - `?` matches one character that is not `/`.
//! - `*` matches any run of characters that are not `/`, including none.
//! - `**` as a whole path segment (`**/x`, `a/**/x`, `a/**`) matches any number of segments,
//!   including none. Elsewhere in a segment it means the same as `*`.
//!
//! Everything else matches itself, case-sensitively, so `[` and `{` are not special. Matching
//! takes time proportional to the pattern's length times the text's, and never backtracks.

use std::fmt;

/// The longest pattern accepted, in bytes.
pub const MAX_GLOB_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobError {
	Empty,
	TooLong,
	/// A control character or a backslash, which this language does not give a meaning to.
	BadCharacter,
}

impl fmt::Display for GlobError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(match self {
			Self::Empty => "the pattern is empty",
			Self::TooLong => "the pattern is longer than 256 bytes",
			Self::BadCharacter => "the pattern holds a control character or a backslash",
		})
	}
}

impl std::error::Error for GlobError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glob {
	pattern: String,
	segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
	/// `**` on its own: any number of path segments.
	Any,
	Text(Vec<Token>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
	Literal(char),
	One,
	Run,
}

impl Glob {
	pub fn new(pattern: &str) -> Result<Self, GlobError> {
		if pattern.is_empty() {
			return Err(GlobError::Empty);
		}
		if pattern.len() > MAX_GLOB_BYTES {
			return Err(GlobError::TooLong);
		}
		if pattern.chars().any(|c| c.is_control() || c == '\\') {
			return Err(GlobError::BadCharacter);
		}
		let segments = pattern.split('/').map(segment).collect();
		Ok(Self {
			pattern: pattern.to_owned(),
			segments,
		})
	}

	pub fn pattern(&self) -> &str {
		&self.pattern
	}

	pub fn is_match(&self, text: &str) -> bool {
		let parts: Vec<&str> = text.split('/').collect();
		let (p, s) = (self.segments.len(), parts.len());
		// `reach[i][j]`: the first `i` pattern segments match the first `j` path segments.
		let mut reach = vec![vec![false; s + 1]; p + 1];
		if let Some(row) = reach.first_mut() {
			if let Some(cell) = row.first_mut() {
				*cell = true;
			}
		}
		for (i, seg) in self.segments.iter().enumerate() {
			for j in 0..=s {
				if !reach
					.get(i)
					.and_then(|r| r.get(j))
					.copied()
					.unwrap_or(false)
				{
					continue;
				}
				match seg {
					Segment::Any => {
						for k in j..=s {
							set(&mut reach, i + 1, k);
						}
					}
					Segment::Text(tokens) => {
						if parts
							.get(j)
							.is_some_and(|part| segment_matches(tokens, part))
						{
							set(&mut reach, i + 1, j + 1);
						}
					}
				}
			}
		}
		reach
			.get(p)
			.and_then(|r| r.get(s))
			.copied()
			.unwrap_or(false)
	}
}

impl fmt::Display for Glob {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.pattern)
	}
}

fn set(reach: &mut [Vec<bool>], i: usize, j: usize) {
	if let Some(cell) = reach.get_mut(i).and_then(|r| r.get_mut(j)) {
		*cell = true;
	}
}

fn segment(text: &str) -> Segment {
	if text == "**" {
		return Segment::Any;
	}
	let mut tokens = Vec::new();
	for c in text.chars() {
		match c {
			'?' => tokens.push(Token::One),
			// `**` inside a segment is one run: two in a row add nothing.
			'*' if tokens.last() == Some(&Token::Run) => {}
			'*' => tokens.push(Token::Run),
			other => tokens.push(Token::Literal(other)),
		}
	}
	Segment::Text(tokens)
}

/// Matches one path segment with the usual two-pointer walk: a run remembers where it can resume.
fn segment_matches(tokens: &[Token], text: &str) -> bool {
	let chars: Vec<char> = text.chars().collect();
	let (mut t, mut c) = (0usize, 0usize);
	let mut resume: Option<(usize, usize)> = None;
	while c < chars.len() {
		match tokens.get(t) {
			Some(Token::Run) => {
				resume = Some((t, c));
				t += 1;
			}
			Some(Token::One) => {
				t += 1;
				c += 1;
			}
			Some(Token::Literal(l)) if chars.get(c) == Some(l) => {
				t += 1;
				c += 1;
			}
			_ => match resume {
				Some((rt, rc)) => {
					resume = Some((rt, rc + 1));
					t = rt + 1;
					c = rc + 1;
				}
				None => return false,
			},
		}
	}
	tokens
		.get(t..)
		.is_some_and(|rest| rest.iter().all(|k| *k == Token::Run))
}
