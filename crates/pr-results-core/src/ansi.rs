// Terminal escape sequences, removed from text that was written for a terminal.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Removes CSI sequences (colours, cursor moves), OSC sequences (titles, links) and lone escapes,
/// and keeps the text between them.
pub fn strip_ansi(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	let mut chars = s.chars().peekable();
	while let Some(c) = chars.next() {
		if c != '\u{1b}' {
			out.push(c);
			continue;
		}
		match chars.peek() {
			Some('[') => {
				chars.next();
				for n in chars.by_ref() {
					if ('@'..='~').contains(&n) {
						break;
					}
				}
			}
			Some(']') => {
				chars.next();
				while let Some(n) = chars.next() {
					if n == '\u{7}' {
						break;
					}
					if n == '\u{1b}' {
						if chars.peek() == Some(&'\\') {
							chars.next();
						}
						break;
					}
				}
			}
			Some(_) => {
				chars.next();
			}
			None => {}
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn colours_titles_and_links_are_removed_and_text_is_kept() {
		assert_eq!(strip_ansi("\u{1b}[31merror\u{1b}[0m: bad"), "error: bad");
		assert_eq!(strip_ansi("\u{1b}[1;38;5;196mx\u{1b}[K"), "x");
		assert_eq!(strip_ansi("a\u{1b}]0;title\u{7}b"), "ab");
		assert_eq!(
			strip_ansi("a\u{1b}]8;;https://x\u{1b}\\link\u{1b}]8;;\u{1b}\\"),
			"alink"
		);
		assert_eq!(strip_ansi("a\u{1b}Mb"), "ab");
		assert_eq!(strip_ansi("trailing\u{1b}"), "trailing");
		assert_eq!(strip_ansi("unterminated\u{1b}[31"), "unterminated");
		assert_eq!(strip_ansi("plain"), "plain");
	}
}
