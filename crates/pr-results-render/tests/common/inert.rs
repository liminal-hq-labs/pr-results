// The inert check: outside code, rendered output holds nothing that GitHub would act on.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use pr_results_core::Marker;
use pr_results_render::Target;

/// The text of `line` with its inline code spans removed. A span opens with a run of backticks
/// and closes at the next run of the same length; a run with no partner is literal.
pub fn strip_code_spans(line: &str) -> String {
	let chars: Vec<char> = line.chars().collect();
	let mut out = String::new();
	let mut i = 0;
	while i < chars.len() {
		if chars[i] != '`' {
			out.push(chars[i]);
			i += 1;
			continue;
		}
		let mut n = 0;
		while chars.get(i + n) == Some(&'`') {
			n += 1;
		}
		let mut j = i + n;
		let mut close = None;
		while j < chars.len() {
			if chars[j] == '`' {
				let mut m = 0;
				while chars.get(j + m) == Some(&'`') {
					m += 1;
				}
				if m == n {
					close = Some(j + m);
					break;
				}
				j += m;
			} else {
				j += 1;
			}
		}
		match close {
			Some(end) => i = end,
			None => {
				out.push_str(&"`".repeat(n));
				i += n;
			}
		}
	}
	out
}

/// Removes every `](destination)`, asserting each destination is a link the tool built.
fn strip_links(name: &str, line: &str, prefix: &str) -> String {
	let mut out = String::new();
	let mut rest = line;
	while let Some(at) = rest.find("](") {
		out.push_str(&rest[..at + 1]);
		let after = &rest[at + 2..];
		assert!(
			after.starts_with(prefix),
			"{name}: a link that is not ours: {line:?}"
		);
		let end = after
			.find(')')
			.unwrap_or_else(|| panic!("{name}: unclosed link: {line:?}"));
		rest = &after[end + 1..];
	}
	out.push_str(rest);
	out
}

/// Removes numeric character references such as `&#58;`, which `Safe::text` writes and whose `#`
/// is not an issue reference.
fn strip_entities(text: &str) -> String {
	let mut out = String::new();
	let mut rest = text;
	while let Some(at) = rest.find("&#") {
		out.push_str(&rest[..at]);
		let after = &rest[at + 2..];
		let digits = after.chars().take_while(char::is_ascii_digit).count();
		if digits > 0 && after[digits..].starts_with(';') {
			rest = &after[digits + 1..];
		} else {
			out.push_str("&#");
			rest = after;
		}
	}
	out.push_str(rest);
	out
}

/// Whether `text` holds a letter or digit, an optional backslash, a hyphen and a digit: the
/// shape of `GH-123` after `Safe` has had its say.
pub fn has_reference(text: &str) -> bool {
	let chars: Vec<char> = text.chars().collect();
	chars.iter().enumerate().any(|(i, c)| {
		c.is_alphanumeric() && {
			let mut at = i + 1;
			if chars.get(at) == Some(&'\\') {
				at += 1;
			}
			chars.get(at) == Some(&'-') && chars.get(at + 1).is_some_and(char::is_ascii_digit)
		}
	})
}

fn fence_length(line: &str) -> usize {
	let trimmed = line.trim_start_matches(' ');
	trimmed.chars().take_while(|c| *c == '`').count()
}

/// Whether a line is one of the few raw HTML lines the renderer writes.
fn trusted_html(line: &str) -> bool {
	if matches!(line, "<details>" | "<details open>" | "</details>") {
		return true;
	}
	match line
		.strip_prefix("<summary>")
		.and_then(|rest| rest.strip_suffix("</summary>"))
	{
		Some(inner) => inner
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | ':' | '(' | ')' | '\u{202f}')),
		None => false,
	}
}

/// Removes the backslash in front of an ASCII punctuation mark, as a Markdown renderer does.
fn unescape(text: &str) -> String {
	let mut out = String::new();
	let mut chars = text.chars().peekable();
	while let Some(c) = chars.next() {
		match chars.peek() {
			Some(next) if c == '\\' && next.is_ascii_punctuation() => {}
			_ => out.push(c),
		}
	}
	out
}

/// What Forgejo links outside code and GitHub does not, in text as it renders: a bare commit hash
/// (seven or more hexadecimal digits), `!` and a digit, `$`, and `:word:`.
fn assert_forgejo_inert(at: &str, line: &str, text: &str) {
	let plain = unescape(text);
	assert!(!plain.contains('$'), "{at}: mathematics: {line:?}");
	let chars: Vec<char> = plain.chars().collect();
	assert!(
		!chars
			.windows(2)
			.any(|w| w[0] == '!' && w[1].is_ascii_digit()),
		"{at}: a pull request reference: {line:?}"
	);
	let mut run = 0;
	for c in &chars {
		run = if c.is_ascii_hexdigit() { run + 1 } else { 0 };
		assert!(run < 7, "{at}: a bare commit hash: {line:?}");
	}
	for (i, c) in chars.iter().enumerate() {
		if *c != ':' {
			continue;
		}
		let inner = chars[i + 1..]
			.iter()
			.take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-'))
			.count();
		assert!(
			inner == 0 || chars.get(i + 1 + inner) != Some(&':'),
			"{at}: an emoji code: {line:?}"
		);
	}
}

/// Asserts that `md` is inert: outside code there is no `<img`, no link the tool did not build, no
/// hidden marker after the first line, no `@`, no `#` except in a heading, no URL to autolink, no
/// reference such as `GH-123`, no HTML but the trusted lines, and every `<details>` is closed.
/// Inside code, only that the fences close.
pub fn assert_inert(name: &str, md: &str, link_prefix: &str) {
	assert_inert_for(Target::GitHub, name, md, link_prefix);
}

/// Like [`assert_inert`] for a target. Forgejo adds the rules for what it links outside code.
pub fn assert_inert_for(target: Target, name: &str, md: &str, link_prefix: &str) {
	let mut fence: Option<usize> = None;
	let mut depth = 0i64;
	for (number, line) in md.lines().enumerate() {
		let at = format!("{name}: line {}", number + 1);
		if let Some(open) = fence {
			let backticks = fence_length(line);
			if backticks >= open && line.trim() == "`".repeat(backticks) {
				fence = None;
			}
			continue;
		}
		let backticks = fence_length(line);
		if backticks >= 4 {
			fence = Some(backticks);
			continue;
		}
		if number == 0 && line.starts_with("<!-- pr-results:1") {
			assert!(
				Marker::parse(line).is_some(),
				"{at}: a bad marker: {line:?}"
			);
			continue;
		}
		assert!(
			!line.contains("pr-results:1") && !line.contains("<!--"),
			"{at}: a marker after the first line: {line:?}"
		);
		if trusted_html(line) {
			match line {
				"<details>" | "<details open>" => depth += 1,
				"</details>" => depth -= 1,
				_ => {}
			}
			assert!(depth >= 0, "{at}: </details> with no <details>");
			continue;
		}
		let text = strip_entities(&strip_links(&at, &strip_code_spans(line), link_prefix));
		let lower = text.to_ascii_lowercase();
		assert!(!lower.contains("<img"), "{at}: an image: {line:?}");
		assert!(!text.contains('<'), "{at}: raw HTML: {line:?}");
		assert!(!text.starts_with('>'), "{at}: a quote: {line:?}");
		assert!(!text.contains('@'), "{at}: a mention: {line:?}");
		let body = text.trim_start_matches('#');
		let heading = text.len() - body.len();
		assert!(
			!body.contains('#') && (heading == 0 || (heading <= 6 && body.starts_with(' '))),
			"{at}: an issue reference: {line:?}"
		);
		assert!(
			!text.contains("://") && !lower.contains("www."),
			"{at}: a URL to autolink: {line:?}"
		);
		assert!(!has_reference(&text), "{at}: a reference: {line:?}");
		if target == Target::Forgejo {
			assert_forgejo_inert(&at, line, &text);
		}
	}
	assert!(fence.is_none(), "{name}: a code fence is never closed");
	assert_eq!(depth, 0, "{name}: unbalanced <details>");
}
