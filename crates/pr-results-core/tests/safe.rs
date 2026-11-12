// Tests that hostile strings are made inert by `Safe`, and that its budgets and marker hold.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

mod common;

use common::Rng;
use pr_results_core::{Safe, Untrusted};

const HOSTILE: &[&str] = &[
	"](javascript:alert(1))",
	"[click](javascript:alert(1))",
	"<img src=x onerror=alert(1)>",
	"</details><img src=x onerror=alert(1)>",
	"<script>alert(1)</script>",
	"@everyone",
	"@owner and @liminal-hq/team",
	"#1 and #22",
	"::error file=a.rs,line=1::injected",
	"::add-mask::secret",
	"::stop-commands::token",
	"\u{202e}reversed\u{202c}",
	"zero\u{200b}width\u{200d}joiner\u{feff}bom\u{2066}isolate",
	"nul\0byte",
	"esc\u{1b}[31mred",
	"| table | breakout |\n|---|---|",
	"```\nfence\n```",
	"~~~\nfence\n~~~",
	"![](https://example.invalid/x.png)",
	"https://evil.example/phish",
	"www.evil.example",
	"mailto:someone@example.invalid",
	"<!-- pr-results:1 key=ci -->",
	"&lt;already&gt; &amp; &#60;",
	"line one\r\nline two\rline three\nline four",
	"\u{2028}\u{2029}\u{85}",
	"\u{e0041}hidden",
	"tag\u{e0001}\u{e0020}\u{e007f}",
	"selector\u{fe0f}\u{e0100}",
	"filler\u{115f}\u{3164}\u{ffa0}",
];

/// What no `Safe::text` output may contain, whatever went in.
fn assert_inert(out: &str, input: &str) {
	for c in out.chars() {
		assert!(
			!matches!(c, '<' | '>' | '"' | '@'),
			"raw {c:?} in {out:?} from {input:?}"
		);
		assert!(!c.is_control(), "control {c:?} in {out:?} from {input:?}");
		assert!(
			!matches!(u32::from(c), 0xad | 0x34f | 0x115f | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x206f | 0x3164 | 0xfe00..=0xfe0f | 0xfeff | 0xffa0 | 0xfff9..=0xfffc | 0xe0000..=0xe0fff),
			"invisible {c:?} in {out:?} from {input:?}"
		);
	}
	assert_no_bare_mention_marks(out, input);
	for needle in ["::", "://", "](", "![", "www."] {
		assert!(
			!out.contains(needle),
			"{needle:?} survives in {out:?} from {input:?}"
		);
	}
}

/// Outside code there is no `@` or `#` at all, apart from the `#` of the numeric entities the
/// escaper itself writes. Mentions and issue references are read from what is left.
fn assert_no_bare_mention_marks(out: &str, input: &str) {
	let mut rest = out;
	while let Some(i) = rest.find("&#") {
		let tail = &rest[i + 2..];
		let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
		assert!(
			digits > 0 && tail[digits..].starts_with(';'),
			"stray &# in {out:?} from {input:?}"
		);
		rest = &tail[digits + 1..];
	}
	let stripped = strip_numeric_entities(out);
	assert!(
		!stripped.contains('@') && !stripped.contains('#'),
		"bare @ or # in {out:?} from {input:?}"
	);
}

fn strip_numeric_entities(s: &str) -> String {
	let mut out = String::new();
	let mut rest = s;
	while let Some(i) = rest.find("&#") {
		out.push_str(&rest[..i]);
		let tail = &rest[i + 2..];
		let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
		rest = tail.get(digits + 1..).unwrap_or_default();
	}
	out.push_str(rest);
	out
}

#[test]
fn hostile_strings_are_inert_as_text() {
	for input in HOSTILE {
		assert_inert(Safe::text(*input).as_str(), input);
	}
}

#[test]
fn text_escapes_each_class() {
	assert_eq!(Safe::text("a|b").as_str(), "a\\|b");
	assert_eq!(Safe::text("<b>&").as_str(), "&lt;b&gt;&amp;");
	assert_eq!(Safe::text("@everyone").as_str(), "\u{ff20}everyone");
	assert_eq!(Safe::text("fix #12").as_str(), "fix \u{ff03}12");
	assert_eq!(Safe::text("@org/team").as_str(), "\u{ff20}org/team");
	assert_eq!(Safe::text("org/repo#123").as_str(), "org/repo\u{ff03}123");
	assert_eq!(Safe::text("GH-1 \u{ff20}x").as_str(), "GH\\-1 \u{ff20}x");
	assert_eq!(
		Safe::text("crate::module::test").as_str(),
		"crate:&#58;module:&#58;test"
	);
	assert_eq!(Safe::text("http://x").as_str(), "http&#58;//x");
	assert_eq!(Safe::text("a\nb").as_str(), "a b");
	assert_eq!(Safe::text("a\0b").as_str(), "ab");
	assert_eq!(Safe::text("\u{202e}x").as_str(), "\\\\u\\{202e\\}x");
	assert_eq!(Safe::text("  indented").as_str(), "&#32;&#32;indented");
}

#[test]
fn random_strings_are_inert() {
	let pool: Vec<char> = "ab:/@#<>\"&'`*_[]()|~\\.-+=!$\n\r\t\0\u{1b}\u{7f}\u{85}\u{200b}\u{202e}\u{2066}\u{feff}é漢🙂 wh"
		.chars()
		.collect();
	let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
	for _ in 0..4000 {
		let len = rng.below(40);
		let s: String = (0..len).map(|_| pool[rng.below(pool.len())]).collect();
		assert_inert(Safe::text(s.as_str()).as_str(), &s);
		// The output is stable: the same input gives the same bytes.
		assert_eq!(Safe::text(s.as_str()), Safe::text(s.as_str()));
	}
}

#[test]
fn a_ten_megabyte_name_is_cut_and_says_so() {
	let big = "A".repeat(10 * 1024 * 1024);
	let out = Safe::text(big.as_str());
	assert!(out.len_chars() < 4096 + 64);
	assert!(out.as_str().ends_with("more characters omitted)"));
	assert!(out
		.as_str()
		.contains(&format!("({} more", 10 * 1024 * 1024 - 4096)));

	let block = Safe::code_block(big.as_str(), 1000);
	assert!(block.len_chars() < 1100);
	assert!(block.as_str().contains("more characters omitted"));

	let line = Safe::log_line(big.as_str(), 100);
	assert!(line.len_chars() < 160);
	assert!(Safe::workflow_data(big.as_str(), 100).len_chars() < 160);
}

#[test]
fn truncation_never_splits_an_escape_and_counts_what_it_cut() {
	assert_eq!(
		Safe::text_max("abcdef", 3).as_str(),
		"abc\u{2026} (3 more characters omitted)"
	);
	// Each ampersand is five characters once escaped; seven fits one, not two.
	let out = Safe::text_max("&&&", 7);
	assert_eq!(out.as_str(), "&amp;\u{2026} (2 more characters omitted)");
	assert_eq!(Safe::text_max("abc", 3).as_str(), "abc");
	assert_eq!(
		Safe::text_max("anything", 0).as_str(),
		"\u{2026} (8 more characters omitted)"
	);
}

#[test]
fn text_already_capped_by_the_parser_keeps_its_omitted_count() {
	let capped = Untrusted::capped("abcdef", 4);
	assert_eq!(capped.omitted(), 2);
	assert_eq!(
		Safe::text(&capped).as_str(),
		"abcd\u{2026} (2 more characters omitted)"
	);
	let multibyte = Untrusted::capped("a\u{e9}b", 2);
	assert_eq!(multibyte.as_str(), "a");
	assert_eq!(multibyte.omitted(), 2);
}

#[test]
fn code_blocks_choose_a_fence_the_content_cannot_close() {
	for input in [
		"plain",
		"```",
		"``````````",
		"a ``` b ```` c",
		"~~~",
		"</details>\n```text\n",
	] {
		let block = Safe::code_block(input, 1000);
		let first = block.as_str().lines().next().unwrap();
		let fence: String = first.chars().take_while(|c| *c == '`').collect();
		assert!(fence.len() >= 4, "{block:?}");
		assert_eq!(first, format!("{fence}text"));
		assert!(block.as_str().ends_with(&fence));
		let longest = input.split(|c| c != '`').map(str::len).max().unwrap_or(0);
		assert!(fence.len() > longest, "fence {fence:?} for {input:?}");
		// No line of the content is the fence, so the block cannot end early.
		let body = block
			.as_str()
			.strip_prefix(&format!("{fence}text\n"))
			.unwrap();
		let body = body.strip_suffix(&format!("\n{fence}")).unwrap();
		assert!(body.lines().all(|l| !l.starts_with(&fence)));
	}
	let hostile = Safe::code_block("a\u{202e}b\0c\r\nd", 100);
	assert_eq!(hostile.as_str(), "````text\na\\u{202e}bc\nd\n````");
}

#[test]
fn workflow_commands_cannot_be_injected() {
	assert_eq!(
		Safe::workflow_data("100%\r\n::error::x", 100).as_str(),
		"100%25%0D%0A::error::x"
	);
	assert_eq!(
		Safe::workflow_property("a:b,c%\n", 100).as_str(),
		"a%3Ab%2Cc%25%0A"
	);
	for input in HOSTILE {
		for out in [
			Safe::workflow_data(*input, 1000),
			Safe::workflow_property(*input, 1000),
		] {
			assert!(!out.as_str().contains(['\r', '\n', '\0']), "{out:?}");
		}
		let line = Safe::log_line(*input, 1000);
		assert!(!line.as_str().contains("::"), "{line:?}");
		assert!(!line.as_str().contains("##"), "{line:?}");
		assert!(
			!line.as_str().contains(['\r', '\n', '\0', '\u{1b}']),
			"{line:?}"
		);
	}
	assert_eq!(
		Safe::log_line("##[error]boom", 100).as_str(),
		"%23%23[error]boom"
	);
}

#[test]
fn trusted_literals_and_numbers_compose() {
	let row = Safe::concat(&[
		Safe::trusted("| "),
		Safe::text("a|b"),
		Safe::trusted(" | "),
		Safe::number(42),
		Safe::trusted(" |"),
	]);
	assert_eq!(row.as_str(), "| a\\|b | 42 |");
	let joined = Safe::join(&Safe::trusted(", "), &[Safe::number(1), Safe::number(2)]);
	assert_eq!(joined.to_string(), "1, 2");
}

/// Every range that must be spelled out, as (first, last, what it is).
const INVISIBLE: &[(u32, u32, &str)] = &[
	(0xe0000, 0xe007f, "the tag block"),
	(0xfe00, 0xfe0f, "variation selectors 1 to 16"),
	(0xe0100, 0xe01ef, "variation selectors supplement"),
	(0x34f, 0x34f, "combining grapheme joiner"),
	(0xad, 0xad, "soft hyphen"),
	(0x206a, 0x206f, "deprecated format characters"),
	(
		0xfff9,
		0xfffc,
		"interlinear annotation and object replacement",
	),
	(0x115f, 0x1160, "Hangul fillers"),
	(0x3164, 0x3164, "Hangul filler"),
	(0xffa0, 0xffa0, "halfwidth Hangul filler"),
	(0x200b, 0x200f, "zero-width and direction marks"),
	(0x202a, 0x202e, "embeddings and overrides"),
	(
		0x2060,
		0x2069,
		"word joiner, invisible operators and isolates",
	),
	(0x180b, 0x180f, "Mongolian selectors and separator"),
	(0x17b4, 0x17b5, "Khmer inherent vowels"),
	(0x600, 0x605, "Arabic number signs"),
	(0x61c, 0x61c, "Arabic letter mark"),
	(0x6dd, 0x6dd, "Arabic end of ayah"),
	(0x70f, 0x70f, "Syriac abbreviation mark"),
	(0x890, 0x891, "Arabic pound and piastre marks"),
	(0x8e2, 0x8e2, "Arabic disputed end of ayah"),
	(0xfeff, 0xfeff, "byte order mark"),
	(0xfff0, 0xfff8, "unassigned default-ignorables"),
	(0x110bd, 0x110bd, "Kaithi number sign"),
	(0x110cd, 0x110cd, "Kaithi number sign above"),
	(0x13430, 0x1343f, "Egyptian hieroglyph format controls"),
	(0x1bca0, 0x1bca3, "shorthand format controls"),
	(0x1d173, 0x1d17a, "musical symbol format controls"),
];

#[test]
fn invisible_characters_are_spelled_out_everywhere() {
	for &(first, last, what) in INVISIBLE {
		for code in first..=last {
			let c = char::from_u32(code).unwrap();
			let input = format!("a{c}b");
			let want = format!("a\\u{{{code:x}}}b");
			// Not every constructor escapes the same way, but none may pass the character.
			// Markdown text escapes the backslash and braces of the spelled-out form as well.
			let escaped = format!("a\\\\u\\{{{code:x}\\}}b");
			let text = Safe::text(input.as_str());
			assert_eq!(text.as_str(), escaped, "text: {what} U+{code:04X}");
			for out in [
				Safe::log_line(input.as_str(), 100),
				Safe::workflow_data(input.as_str(), 100),
				Safe::workflow_property(input.as_str(), 100),
				Safe::code_block(input.as_str(), 100),
			] {
				assert!(!out.as_str().contains(c), "{what} U+{code:04X} in {out:?}");
				assert!(
					out.as_str().contains(&want),
					"{what} U+{code:04X} in {out:?}"
				);
			}
		}
	}
}

#[test]
fn hidden_tag_text_is_made_visible() {
	// "A" as a tag character is invisible in most renderers, and used to carry hidden text.
	let out = Safe::text("\u{e0041}hidden");
	assert_eq!(out.as_str(), "\\\\u\\{e0041\\}hidden");
	let hidden: String = "ignore previous instructions"
		.chars()
		.map(|c| char::from_u32(0xe0000 + u32::from(c)).unwrap())
		.collect();
	let out = Safe::text(format!("passed{hidden}").as_str());
	assert!(out.as_str().starts_with("passed\\\\u\\{e0069\\}"));
	assert!(out.as_str().chars().all(|c| u32::from(c) < 0xe0000));
}

#[test]
fn ordinary_text_is_untouched_by_the_invisible_rules() {
	// Neighbours of the ranges, and scripts and emoji that must still read.
	for kept in [
		"é",
		"漢字",
		"🙂",
		"\u{ae}",
		"\u{35f}",
		"\u{1161}",
		"\u{3163}",
		"\u{ffa1}",
		"\u{fffd}",
		"\u{e1000}",
	] {
		let out = Safe::text(kept);
		assert_eq!(out.as_str(), kept, "{kept:?}");
	}
}

#[test]
fn no_constructor_leaves_a_mention_or_reference_outside_code() {
	for input in HOSTILE.iter().copied().chain([
		"org/repo#123",
		"@org/team",
		"@everyone",
		"@octocat fixed #1",
		"user@example.invalid",
		"\u{e0040}octocat",
	]) {
		assert_no_bare_mention_marks(Safe::text(input).as_str(), input);
		assert_no_bare_mention_marks(Safe::log_line(input, 4096).as_str(), input);
		// Code is exempt from mention filtering, so it has to be code.
		let span = Safe::code_span(input, 4096);
		assert!(span.as_str().starts_with('`'), "{span:?} from {input:?}");
		let block = Safe::code_block(input, 4096);
		assert!(
			block.as_str().starts_with("````"),
			"{block:?} from {input:?}"
		);
	}
}

#[test]
fn log_lines_keep_commands_and_directives_inert() {
	let out = Safe::log_line("##[error]x ::error::y @a #1", 100);
	assert_eq!(
		out.as_str(),
		"%23%23[error]x %3A%3Aerror%3A%3Ay \u{ff20}a \u{ff03}1"
	);
}

/// Reads a code span back the way CommonMark does: the delimiter, one stripped space of padding on
/// each side when both are spaces and the content is not all spaces.
fn read_code_span(span: &str) -> String {
	let fence = span.chars().take_while(|&c| c == '`').count();
	let inner = &span[fence..];
	let end = inner.rfind(&"`".repeat(fence)).unwrap();
	assert_eq!(inner[end..].len(), fence, "closing delimiter in {span:?}");
	let body = &inner[..end];
	assert!(
		!body.contains(&"`".repeat(fence)),
		"body closes early in {span:?}"
	);
	if body.starts_with(' ') && body.ends_with(' ') && body.chars().any(|c| c != ' ') {
		body[1..body.len() - 1].to_owned()
	} else {
		body.to_owned()
	}
}

#[test]
fn code_span_adapts_its_delimiter_to_backticks_of_every_length() {
	for n in 1..=12usize {
		let ticks = "`".repeat(n);
		let cases = [
			ticks.clone(),
			format!("a{ticks}b"),
			format!("{ticks}a"),
			format!("a{ticks}"),
			format!("{ticks}a{ticks}"),
			format!("`a{ticks}``b"),
		];
		for input in cases {
			let out = Safe::code_span(input.as_str(), 4096);
			assert_eq!(read_code_span(out.as_str()), input, "{input:?} -> {out:?}");
			assert!(!out.as_str().contains('\n'));
		}
	}
	assert_eq!(Safe::code_span("plain", 100).as_str(), "`plain`");
	assert_eq!(Safe::code_span("`", 100).as_str(), "`` ` ``");
	assert_eq!(Safe::code_span("a`b", 100).as_str(), "``a`b``");
	assert_eq!(Safe::code_span("``", 100).as_str(), "``` `` ```");
	assert_eq!(Safe::code_span("", 100).as_str(), "` `");
	assert_eq!(Safe::code_span("   ", 100).as_str(), "`   `");
	assert_eq!(Safe::code_span(" a ", 100).as_str(), "`  a  `");
}

#[test]
fn code_span_shows_mentions_as_they_are_and_cannot_be_broken_out_of() {
	assert_eq!(Safe::code_span("@octocat", 100).as_str(), "`@octocat`");
	assert_eq!(Safe::code_span("org/repo#1", 100).as_str(), "`org/repo#1`");
	for input in HOSTILE {
		let out = Safe::code_span(*input, 4096);
		assert!(!out.as_str().contains(['\n', '\r']), "{out:?}");
		let one_span = read_code_span(out.as_str());
		assert!(!one_span.contains('\n'));
	}
	// A pipe is written for a table cell.
	assert_eq!(Safe::code_span("a|b", 100).as_str(), "`a\\|b`");
}

#[test]
fn code_span_marks_line_breaks_and_drops_controls() {
	assert_eq!(
		Safe::code_span("a\nb\r\nc\rd", 100).as_str(),
		"`a\u{23ce}b\u{23ce}c\u{23ce}d`"
	);
	assert_eq!(
		Safe::code_span("a\u{2028}b\u{85}c", 100).as_str(),
		"`a\u{23ce}b\u{23ce}c`"
	);
	assert_eq!(Safe::code_span("a\tb\0c\u{1b}d", 100).as_str(), "`a bcd`");
}

#[test]
fn code_span_spells_out_invisible_characters() {
	assert_eq!(
		Safe::code_span("\u{e0041}hidden", 100).as_str(),
		"`\\u{e0041}hidden`"
	);
	for &(first, last, what) in INVISIBLE {
		for code in first..=last {
			let c = char::from_u32(code).unwrap();
			let out = Safe::code_span(format!("a{c}b").as_str(), 100);
			assert_eq!(
				out.as_str(),
				format!("`a\\u{{{code:x}}}b`"),
				"{what} U+{code:04X}"
			);
		}
	}
}

#[test]
fn code_span_is_cut_and_says_so() {
	let big = "N".repeat(10 * 1024 * 1024);
	let out = Safe::code_span(big.as_str(), 50);
	assert!(out.len_chars() < 120, "{}", out.len_chars());
	assert!(out.as_str().starts_with(&format!("`{}` ", "N".repeat(50))));
	assert!(out.as_str().ends_with(&format!(
		"({} more characters omitted)",
		10 * 1024 * 1024 - 50
	)));
	// The cut never lands inside an escape.
	assert_eq!(
		Safe::code_span("ab|cd", 3).as_str(),
		"`ab` \u{2026} (3 more characters omitted)"
	);
	assert_eq!(
		Safe::code_span("abc\u{200b}", 5).as_str(),
		"`abc` \u{2026} (1 more characters omitted)"
	);
	// What the parser already cut is counted too.
	let capped = Untrusted::capped("abcdef", 3);
	assert_eq!(
		Safe::code_span(&capped, 100).as_str(),
		"`abc` \u{2026} (3 more characters omitted)"
	);
}
