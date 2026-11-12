// The settings file: defaults, every key, Desk's two suites, and every way to get one wrong.
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

use common::{root, DESK_TOML};
use pr_results::config::{
	default_check_name, suite_name_from_stem, title_from_repo, CommentMode, Settings,
};
use pr_results_core::Flavour;
use pr_results_render::Symbols;

fn err(text: &str) -> String {
	Settings::parse(text).unwrap_err().to_string()
}

#[test]
fn an_empty_file_is_the_defaults() {
	let s = Settings::parse("").unwrap();
	assert_eq!(s.title, None);
	assert_eq!(s.key.as_str(), "ci");
	assert_eq!(s.mode, CommentMode::Always);
	assert_eq!((s.max_failures, s.excerpt_lines), (20, 20));
	assert_eq!(s.symbols, Symbols::Text);
	assert_eq!((s.max_annotations, s.flaky_annotations), (50, false));
	assert!(s.suites.is_empty());
	assert_eq!(s.limits.max_files, 500);
}

#[test]
fn desks_two_suites_read_as_written() {
	let s = Settings::parse(DESK_TOML).unwrap();
	assert_eq!(s.title.as_deref(), Some("Desk checks"));
	assert_eq!(s.suites.len(), 2);
	let steps = &s.suites[0];
	assert_eq!(steps.name, "Desk steps");
	assert_eq!(steps.check.as_deref(), Some("Desk check steps"));
	assert_eq!(steps.flavour, Flavour::Steps);
	assert!(steps.required);
	assert!(steps.artifact.is_match("results-check"));
	assert!(steps.files[0].is_match("steps.xml"));
	assert!(steps.files[0].is_match("a/b/steps.xml"));
	assert!(!steps.files[0].is_match("nextest-core.xml"));
	let rust = &s.suites[1];
	assert_eq!(rust.flavour, Flavour::Nextest);
	assert!(!rust.required);
	assert!(rust.files[0].is_match("nextest-core.xml"));
}

#[test]
fn every_key_of_the_specification_is_accepted() {
	let s = Settings::parse(
		r#"
schema = 1
title = "Example checks"

[comment]
key = "e2e"
mode = "off"
max_failures = 5
excerpt_lines = 8
symbols = "glyphs"
other_checks = false

[checks]
per_suite = true
combined = ""

[annotations]
max_per_suite = 500
flaky = true

[baseline]
branch = "main"
workflow = "CI"
slower_percent = 25
slower_min_seconds = 5

[limits]
max_artifact_bytes = 1_000_000
max_files = 10
max_file_bytes = 0x10000
max_cases = 1000

[[suite]]
name = "Web"
artifact = "web-*"
files = ["a.xml", "**/b.xml"]
format = "junit"
flavour = "vitest"
required = true
path_prefix = ""
"#,
	)
	.unwrap();
	assert_eq!(s.key.as_str(), "e2e");
	assert_eq!(s.mode, CommentMode::Off);
	assert_eq!((s.max_failures, s.excerpt_lines), (5, 8));
	assert_eq!(s.symbols, Symbols::Glyphs);
	assert_eq!((s.max_annotations, s.flaky_annotations), (500, true));
	assert_eq!(s.limits.max_artifact_bytes, 1_000_000);
	assert_eq!(s.limits.max_file_bytes, 0x10000);
	assert_eq!(s.suites[0].files.len(), 2);
	assert_eq!(s.suites[0].flavour, Flavour::Vitest);
}

#[test]
fn an_unknown_key_names_the_key_and_the_line() {
	assert_eq!(
		err("title = \"x\"\ntitel = \"y\"\n"),
		"line 2: unknown key `titel`"
	);
	assert_eq!(
		err("[comment]\nkey = \"ci\"\nmoed = \"off\"\n"),
		"line 3: unknown key `comment.moed`"
	);
	assert_eq!(
		err("[[suite]]\nname = \"a\"\nartifact = \"b\"\nflavor = \"nextest\"\n"),
		"line 4: unknown key `suite.flavor`"
	);
	assert_eq!(
		err("[limits]\nmax_file = 3\n"),
		"line 2: unknown key `limits.max_file`"
	);
}

#[test]
fn a_value_of_the_wrong_kind_or_size_is_named() {
	assert_eq!(err("title = 3"), "line 1: `title` must be a string");
	assert_eq!(
		err("[comment]\nmax_failures = \"many\""),
		"line 2: `comment.max_failures` must be a whole number"
	);
	assert_eq!(
		err("[comment]\nmax_failures = -1"),
		"line 2: `comment.max_failures` must be a whole number from 0 to 1000"
	);
	assert_eq!(
		err("[annotations]\nmax_per_suite = 501"),
		"line 2: `annotations.max_per_suite` must be a whole number from 0 to 500"
	);
	assert_eq!(
		err("[annotations]\nflaky = \"yes\""),
		"line 2: `annotations.flaky` must be true or false"
	);
	assert_eq!(err("schema = 2"), "line 1: `schema` must be 1");
	assert_eq!(err("comment = 3"), "line 1: `comment` must be a table");
	assert_eq!(
		err("suite = 3"),
		"line 1: `suite` must be an array of tables, `[[suite]]`"
	);
	assert_eq!(
		err("[comment]\nsymbols = \"emoji\""),
		"line 2: `comment.symbols` must be \"text\" or \"glyphs\""
	);
	assert!(err("[comment]\nkey = \"Bad Key\"").starts_with("line 2: `comment.key` must be"));
	assert!(err("[limits]\nmax_files = 0").contains("from 1 to 10000"));
}

#[test]
fn a_file_that_is_not_toml_says_where() {
	let e = Settings::parse("title = \n").unwrap_err();
	assert_eq!(e.line, Some(1));
	let e = Settings::parse("a = 1\na = 2\n").unwrap_err();
	assert_eq!(e.line, Some(2));
}

#[test]
fn names_are_one_to_a_hundred_printable_characters() {
	let long = "x".repeat(101);
	assert!(err(&format!("title = \"{long}\"")).contains("1 to 100 printable"));
	assert!(err("title = \"\"").contains("1 to 100 printable"));
	assert!(err("title = \"a\\u0007b\"").contains("1 to 100 printable"));
	let ok = "x".repeat(100);
	assert!(Settings::parse(&format!("title = \"{ok}\"")).is_ok());
	assert!(err(&format!(
		"[[suite]]\nname = \"a\"\nartifact = \"b\"\ncheck = \"{long}\""
	))
	.contains("`suite.check` must be 1 to 100 printable"));
	assert!(
		err(&format!("[[suite]]\nname = \"{long}\"\nartifact = \"b\"")).contains("`suite.name`")
	);
}

#[test]
fn a_suite_needs_a_name_and_an_artefact_and_a_usable_pattern() {
	assert_eq!(
		err("[[suite]]\nartifact = \"a\""),
		"line 1: a `[[suite]]` needs a `name`"
	);
	assert_eq!(
		err("[[suite]]\nname = \"a\""),
		"line 1: a `[[suite]]` needs an `artifact`"
	);
	assert!(err("[[suite]]\nname = \"a\"\nartifact = \"\"").contains("not a usable pattern"));
	assert!(err("[[suite]]\nname = \"a\"\nartifact = \"b\"\nfiles = []").contains("1 to 32"));
	assert!(
		err("[[suite]]\nname = \"a\"\nartifact = \"b\"\nfiles = [3]").contains("must hold strings")
	);
	assert!(
		err("[[suite]]\nname = \"a\"\nartifact = \"b\"\nfiles = [\"a\\\\b\"]")
			.contains("cannot be used")
	);
}

#[test]
fn only_junit_is_a_format_and_only_known_flavours_exist() {
	let suite = |extra: &str| format!("[[suite]]\nname = \"a\"\nartifact = \"b\"\n{extra}\n");
	assert!(Settings::parse(&suite("format = \"junit\"")).is_ok());
	assert!(err(&suite("format = \"steps-times\"")).contains("not supported"));
	assert!(err(&suite("format = \"steps-json\"")).contains("not supported"));
	assert!(err(&suite("format = \"tap\"")).contains("must be \"junit\""));
	assert!(err(&suite("flavour = \"mocha\"")).contains("`suite.flavour` must be"));
	for f in [
		"auto",
		"nextest",
		"vitest",
		"bun",
		"jest",
		"playwright",
		"steps",
		"generic",
	] {
		assert!(
			Settings::parse(&suite(&format!("flavour = \"{f}\""))).is_ok(),
			"{f}"
		);
	}
}

#[test]
fn settings_for_later_versions_are_refused_not_ignored() {
	assert!(err("[comment]\nmode = \"changes\"").contains("arrives in v0.3"));
	assert!(err("[comment]\nmode = \"failures\"").contains("arrives in v0.3"));
	assert!(err("[comment]\nmode = \"loud\"").contains("must be"));
	assert!(err("[comment]\nother_checks = true").contains("arrives in v0.3"));
	assert!(err("[checks]\nper_suite = false").contains("arrives with the combined check"));
	assert!(err("[checks]\ncombined = \"All\"").contains("arrives in v0.3"));
	assert!(
		err("[[suite]]\nname = \"a\"\nartifact = \"b\"\npath_prefix = \"web/\"")
			.contains("arrives in v0.2")
	);
}

#[test]
fn two_suites_may_not_share_a_check_name() {
	let two =
		"[[suite]]\nname = \"a\"\nartifact = \"x\"\n[[suite]]\nname = \"a\"\nartifact = \"y\"\n";
	assert!(err(two).contains("same check name"));
	let named = "[[suite]]\nname = \"a\"\nartifact = \"x\"\ncheck = \"One\"\n[[suite]]\nname = \"b\"\nartifact = \"y\"\ncheck = \"One\"\n";
	assert!(err(named).contains("same check name"));
	let fine =
		"[[suite]]\nname = \"a\"\nartifact = \"x\"\n[[suite]]\nname = \"b\"\nartifact = \"y\"\n";
	assert!(Settings::parse(fine).is_ok());
}

#[test]
fn names_come_from_the_repository_and_the_artefact() {
	assert_eq!(title_from_repo("desk"), "Desk");
	assert_eq!(title_from_repo("pr-results"), "Pr Results");
	assert_eq!(title_from_repo("my_app.rs"), "My App Rs");
	assert_eq!(title_from_repo("---"), "Test results");
	assert_eq!(suite_name_from_stem("rust"), "Rust tests");
	assert_eq!(suite_name_from_stem("frontend"), "Frontend tests");
	assert_eq!(suite_name_from_stem("e2e-web"), "E2e web tests");
	assert_eq!(default_check_name("Desk", "Rust tests"), "Desk Rust tests");
	assert_eq!(default_check_name("", "Rust tests"), "Rust tests");
}

#[test]
fn every_settings_example_in_the_documents_is_valid() {
	let mut checked = 0;
	for file in ["SPEC.md", "docs/adopting.md"] {
		let text = std::fs::read_to_string(root().join(file)).unwrap();
		let mut lines = text.lines();
		while let Some(line) = lines.next() {
			if line.trim_end() != "```toml" {
				continue;
			}
			let block: Vec<&str> = lines
				.by_ref()
				.take_while(|l| !l.starts_with("```"))
				.collect();
			let source = block.join("\n");
			if let Err(e) = Settings::parse(&source) {
				panic!("{file}: a toml block is not valid settings: {e}\n{source}");
			}
			checked += 1;
		}
	}
	// SPEC.md has one, and the adoption guide has three: section 3, Playwright and Desk.
	assert!(checked >= 4, "found only {checked} toml blocks");
}
