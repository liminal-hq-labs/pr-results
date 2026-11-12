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
			!matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}'),
			"invisible {c:?} in {out:?} from {input:?}"
		);
	}
	// A hash sign is only ever the start of a numeric entity.
	for (i, c) in out.char_indices() {
		assert!(
			c != '#' || out[..i].ends_with('&'),
			"raw # in {out:?} from {input:?}"
		);
	}
	for needle in ["::", "://", "](", "![", "www."] {
		assert!(
			!out.contains(needle),
			"{needle:?} survives in {out:?} from {input:?}"
		);
	}
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
	assert_eq!(Safe::text("@everyone").as_str(), "&#64;everyone");
	assert_eq!(Safe::text("fix #12").as_str(), "fix &#35;12");
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
