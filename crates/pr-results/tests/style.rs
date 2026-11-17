// Tests for the comment's style settings: the presets, what is set by hand, and the commands.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use pr_results::config::Settings;
use pr_results_render::{Style, Symbols};

fn of(text: &str) -> Settings {
	Settings::parse(text).unwrap()
}

fn err(text: &str) -> String {
	Settings::parse(text).unwrap_err().to_string()
}

#[test]
fn the_style_chooses_the_defaults_and_what_is_set_by_hand_wins() {
	let plain = of("[comment]\nstyle = \"plain\"");
	assert_eq!(plain.style, Style::Plain);
	assert_eq!(plain.symbols, Symbols::Text);

	let guided = of("[comment]\nstyle = \"guided\"");
	assert_eq!(
		(guided.style, guided.symbols),
		(Style::Guided, Symbols::Glyphs)
	);
	assert_eq!(
		(guided.mascot, guided.progress, guided.alerts),
		(false, false, false)
	);

	let enhanced = of("[comment]\nstyle = \"enhanced\"");
	assert_eq!(
		(enhanced.style, enhanced.symbols),
		(Style::Guided, Symbols::Emoji)
	);
	assert_eq!(
		(enhanced.mascot, enhanced.progress, enhanced.alerts),
		(true, true, true)
	);

	let trimmed = of("[comment]\nstyle = \"enhanced\"\nmascot = false\nsymbols = \"glyphs\"");
	assert_eq!(trimmed.symbols, Symbols::Glyphs);
	assert_eq!(
		(trimmed.mascot, trimmed.progress, trimmed.alerts),
		(false, true, true)
	);

	let picked = of("[comment]\nprogress = true\nsymbols = \"emoji\"");
	assert_eq!(
		(picked.style, picked.symbols),
		(Style::Guided, Symbols::Emoji)
	);
	assert_eq!(
		(picked.mascot, picked.progress, picked.alerts),
		(false, true, false)
	);
}

#[test]
fn the_extras_need_a_style_that_has_them() {
	for key in ["mascot", "progress", "alerts"] {
		let text = format!("[comment]\nstyle = \"plain\"\n{key} = true");
		assert!(err(&text).contains("needs `comment.style` of \"guided\" or \"enhanced\""));
		// Switching one off is always allowed, so a file can move between styles.
		of(&format!("[comment]\nstyle = \"plain\"\n{key} = false"));
	}
	assert_eq!(
		err("[comment]\nstyle = \"loud\""),
		"line 2: `comment.style` must be \"plain\", \"guided\" or \"enhanced\""
	);
}

#[test]
fn checks_can_be_ignored_by_pattern_and_given_a_command() {
	let s = of(
		"[comment]\nother_checks = true\nother_checks_ignore = [\"Test (*)\", \"Docs\"]\n\n[comment.check_commands]\n\"Security audit\" = \"cargo audit\"\n\"Format\" = \"cargo fmt --all -- --check\"",
	);
	assert_eq!(s.other_checks_ignore.len(), 2);
	assert!(s.other_checks_ignore[0].is_match("Test (default)"));
	assert!(!s.other_checks_ignore[0].is_match("Clippy (default)"));
	assert_eq!(
		s.check_commands,
		[
			("Format".to_owned(), "cargo fmt --all -- --check".to_owned()),
			("Security audit".to_owned(), "cargo audit".to_owned()),
		]
	);
	assert!(err("[comment]\nother_checks_ignore = \"x\"").contains("must be a list"));
	assert!(err("[comment]\nother_checks_ignore = [3]").contains("must hold strings"));
	assert!(err("[comment]\nother_checks_ignore = [\"\"]").contains("cannot be used"));
	let many = format!(
		"[comment]\nother_checks_ignore = [{}]",
		vec!["\"a\""; 51].join(",")
	);
	assert!(err(&many).contains("at most 50"));
}

#[test]
fn a_command_to_show_is_short_printable_ascii_with_no_backtick_or_backslash() {
	let bad = |command: &str| {
		err(&format!(
			"[comment.check_commands]\n\"Audit\" = {}",
			toml_string(command)
		))
	};
	for command in [
		"",
		"cargo `id`",
		"cargo \\ audit",
		"cargo\naudit",
		"caf\u{e9}",
	] {
		assert!(bad(command).contains("printable ASCII"), "{command:?}");
	}
	assert!(bad(&"a".repeat(201)).contains("1 to 200"));
	assert!(err("[comment.check_commands]\n\"Audit\" = 3").contains("must be a string"));
	of("[comment.check_commands]\n\"Audit\" = \"cargo audit --ignore RUSTSEC\"");
}

fn toml_string(s: &str) -> String {
	let escaped = s
		.replace('\\', "\\\\")
		.replace('"', "\\\"")
		.replace('\n', "\\n");
	format!("\"{escaped}\"")
}

#[test]
fn run_locally_is_for_nextest_suites_and_an_empty_one_turns_the_command_off() {
	let suite = |extra: &str| format!("[[suite]]\nname = \"a\"\nartifact = \"b\"\n{extra}");
	let s = of(&suite("run_locally = \"cargo nextest run --all-features\""));
	assert_eq!(
		s.suites[0].run_locally.as_deref(),
		Some("cargo nextest run --all-features")
	);
	let off = of(&suite("flavour = \"vitest\"\nrun_locally = \"\""));
	assert_eq!(off.suites[0].run_locally.as_deref(), Some(""));
	assert_eq!(of(&suite("")).suites[0].run_locally, None);
	assert!(err(&suite(
		"flavour = \"vitest\"\nrun_locally = \"npx vitest run\""
	))
	.contains("is for nextest suites"));
	assert!(err(&suite("run_locally = \"cargo `id`\"")).contains("printable ASCII"));
}

#[test]
fn a_suite_can_name_the_job_it_comes_from() {
	let suite = |extra: &str| format!("[[suite]]\nname = \"a\"\nartifact = \"b\"\n{extra}");
	assert_eq!(
		of(&suite("job = \"Tests\"")).suites[0].job.as_deref(),
		Some("Tests")
	);
	assert_eq!(of(&suite("")).suites[0].job, None);
	assert!(err(&suite("job = \"\"")).contains("`suite.job`"));
	assert!(err(&suite("job = 3")).contains("must be a string"));
	assert!(err(&suite(&format!("job = \"{}\"", "j".repeat(101)))).contains("1 to 100"));

	let two = "[[suite]]\nname = \"a\"\nartifact = \"x\"\njob = \"Tests\"\n\n[[suite]]\nname = \"b\"\nartifact = \"y\"\njob = \"Tests\"";
	assert!(err(two).contains("two suites name the same `job`"));
	let different = two.replacen("job = \"Tests\"", "job = \"Lint\"", 1);
	of(&different);
}
