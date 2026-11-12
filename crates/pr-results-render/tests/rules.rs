// Rules about the renderer's own source and output: where text may enter, and how it is spelled.
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

use std::path::{Path, PathBuf};

use common::{parse_text, scenarios, section, Scenario};
use pr_results_core::Flavour;

fn src_files() -> Vec<(PathBuf, String)> {
	let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
	let mut files: Vec<(PathBuf, String)> = std::fs::read_dir(dir)
		.unwrap()
		.map(|e| e.unwrap().path())
		.filter(|p| p.extension().is_some_and(|e| e == "rs"))
		.map(|p| {
			let text = std::fs::read_to_string(&p).unwrap();
			(p, text)
		})
		.collect();
	files.sort();
	files
}

fn name(p: &Path) -> String {
	p.file_name().unwrap().to_string_lossy().into_owned()
}

/// Everything before the unit tests of a file.
fn code_of(text: &str) -> &str {
	text.split("#[cfg(test)]").next().unwrap()
}

#[test]
fn lines_are_built_by_cat_so_no_two_pieces_can_form_a_reference() {
	for (path, text) in src_files() {
		if name(&path) == "text.rs" {
			continue;
		}
		let code = code_of(&text);
		assert!(
			!code.contains("Safe::concat("),
			"{} calls Safe::concat",
			name(&path)
		);
		assert!(
			!code.contains("Safe::join("),
			"{} calls Safe::join",
			name(&path)
		);
	}
}

#[test]
fn untrusted_text_enters_only_through_code_or_plain_forms() {
	// `Safe::text` is for the heading's title alone. A name, a message or a path goes through
	// `code_span`, `code_block` or `plain_text`/`plain_block`.
	let mut uses = Vec::new();
	for (path, text) in src_files() {
		let code = code_of(&text);
		for form in ["Safe::text(", "Safe::text_max("] {
			for _ in code.matches(form) {
				uses.push(format!("{}: {form}", name(&path)));
			}
		}
		assert!(!code.contains("Safe::log_line("), "{}", name(&path));
		assert!(!code.contains("workflow_data("), "{}", name(&path));
		assert!(
			!code.contains("as_str()") || name(&path) != "blocks.rs",
			"blocks.rs reads raw text"
		);
	}
	assert_eq!(uses, ["comment.rs: Safe::text_max("], "{uses:?}");
}

#[test]
fn raw_html_is_written_only_as_trusted_literals() {
	// Every `<` in the source that starts a tag sits inside a `t("…")` literal or a `//` comment.
	for (path, text) in src_files() {
		for (n, line) in code_of(&text).lines().enumerate() {
			let code = line.split("//").next().unwrap();
			if code.contains("<details") || code.contains("<summary") || code.contains("</details")
			{
				assert!(
					code.contains("t(") || code.contains("t(\n"),
					"{}:{}: HTML outside a trusted literal: {line}",
					name(&path),
					n + 1
				);
			}
		}
	}
}

/// Banned spellings, and the Canadian words for them.
const BANNED: [&str; 4] = ["color", "canceled", "behavior", "artifact"];

/// Whether `text` holds `word` as prose: not as part of an identifier (`run_artifacts`), a
/// fragment (`#artifacts`) or a longer word.
fn prose_hit(text: &str, word: &str) -> Option<String> {
	let lower = text.to_ascii_lowercase();
	let mut from = 0;
	while let Some(at) = lower[from..].find(word) {
		let start = from + at;
		let before = lower[..start].chars().next_back();
		if !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '#') {
			return Some(text[start.saturating_sub(20)..(start + 20).min(text.len())].to_owned());
		}
		from = start + word.len();
	}
	None
}

fn strip_destinations(text: &str) -> String {
	let mut out = String::new();
	let mut rest = text;
	while let Some(at) = rest.find("](") {
		out.push_str(&rest[..at + 1]);
		let after = &rest[at + 2..];
		rest = after.find(')').map_or("", |end| &after[end + 1..]);
	}
	out.push_str(rest);
	out
}

#[test]
fn the_spelling_is_canadian_in_the_source() {
	for (path, text) in src_files() {
		for word in BANNED {
			if let Some(hit) = prose_hit(&text, word) {
				panic!("{}: {word:?} near {hit:?}", name(&path));
			}
		}
	}
}

#[test]
fn the_spelling_is_canadian_in_every_output() {
	for sc in scenarios() {
		// The hostile scenario's text is the attacker's; every other word is ours.
		if sc.name == "hostile" {
			continue;
		}
		for (file, text) in sc.outputs() {
			let text = strip_destinations(&text);
			for word in BANNED {
				assert_eq!(prose_hit(&text, word), None, "{}/{file}: {word}", sc.name);
			}
		}
	}
}

#[test]
fn a_title_cannot_run_into_the_outcome_to_make_a_reference() {
	// Each of these ends the way a reference starts, or starts the way one ends.
	for title in ["GH", "GH-", "ABC", "x9", "-123", "1", "\\", "GH\u{ff0d}"] {
		let sc = Scenario::new(
			"t",
			title,
			vec![
				section("GH", false, parse_text(
					"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"n\"><failure message=\"m\"/></testcase></testsuite></testsuites>",
					Flavour::Generic,
				)),
				section("-1", false, Vec::new()),
			],
		);
		for (file, text) in sc.outputs() {
			if file.ends_with(".md") {
				common::inert::assert_inert(&format!("{title}/{file}"), &text, common::LINK_PREFIX);
			}
		}
	}
}
