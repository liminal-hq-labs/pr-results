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

#[test]
fn a_suite_can_read_sarif_and_say_how_serious_a_finding_must_be_to_fail() {
	use pr_results_core::{InputFormat, Severity};
	let suite = |extra: &str| format!("[[suite]]\nname = \"a\"\nartifact = \"b\"\n{extra}");

	let plain = of(&suite(""));
	assert_eq!(plain.suites[0].format, InputFormat::Junit);
	assert_eq!(plain.suites[0].fail_on, Severity::Warning);

	let sarif = of(&suite("format = \"sarif\""));
	assert_eq!(sarif.suites[0].format, InputFormat::Sarif);
	assert_eq!(sarif.suites[0].fail_on, Severity::Warning);

	for (word, level) in [
		("note", Severity::Note),
		("warning", Severity::Warning),
		("error", Severity::Error),
	] {
		let s = of(&suite(&format!("format = \"sarif\"\nfail_on = \"{word}\"")));
		assert_eq!(s.suites[0].fail_on, level, "{word}");
	}
	assert!(err(&suite("format = \"sarif\"\nfail_on = \"loud\""))
		.contains("must be \"note\", \"warning\" or \"error\""));
	assert!(err(&suite("format = \"sarif\"\nfail_on = 3")).contains("must be a string"));
}

#[test]
fn a_findings_format_reads_sarif_files_unless_told_otherwise() {
	let suite = |extra: &str| format!("[[suite]]\nname = \"a\"\nartifact = \"b\"\n{extra}");
	let files = |s: &pr_results::config::Settings| s.suites[0].files[0].clone();
	assert!(files(&of(&suite("format = \"sarif\""))).is_match("out/clippy.sarif"));
	assert!(!files(&of(&suite("format = \"sarif\""))).is_match("out/junit.xml"));
	assert!(files(&of(&suite(""))).is_match("out/junit.xml"));
	assert!(files(&of(&suite("format = \"sarif\"\nfiles = [\"*.json\"]"))).is_match("a.json"));
}

#[test]
fn a_setting_that_a_format_has_no_use_for_is_refused() {
	let suite = |extra: &str| format!("[[suite]]\nname = \"a\"\nartifact = \"b\"\n{extra}");
	assert!(err(&suite("fail_on = \"error\"")).contains("is for formats that report findings"));
	assert!(err(&suite("format = \"sarif\"\nflavour = \"nextest\"")).contains("is for JUnit files"));
	assert!(err(&suite(
		"format = \"sarif\"\nrun_locally = \"cargo nextest run\""
	))
	.contains("is for nextest suites"));
	assert!(err(&suite("format = \"junit\"\nfail_on = \"note\""))
		.contains("is for formats that report findings"));
	assert!(err(&suite("format = \"steps-json\"")).contains("one of the formats"));
}

#[test]
fn a_suite_can_read_what_cargo_writes() {
	use pr_results_core::InputFormat;
	let suite = |extra: &str| format!("[[suite]]\nname = \"a\"\nartifact = \"b\"\n{extra}");
	for (word, format) in [
		("clippy-json", InputFormat::ClippyJson),
		("cargo-audit-json", InputFormat::CargoAuditJson),
	] {
		let s = of(&suite(&format!("format = \"{word}\"\nfail_on = \"error\"")));
		assert_eq!(s.suites[0].format, format, "{word}");
		assert!(s.suites[0].files[0].is_match("out/result.json"), "{word}");
		assert!(!s.suites[0].files[0].is_match("out/result.xml"), "{word}");
		assert!(err(&suite(&format!(
			"format = \"{word}\"\nflavour = \"nextest\""
		)))
		.contains("is for JUnit files"));
	}
	assert!(err(&suite("format = \"cargo\"")).contains("\"clippy-json\" or \"cargo-audit-json\""));
}

#[test]
fn the_other_checks_table_can_be_kept_open() {
	assert!(!of("").other_checks_open);
	assert!(!of("[comment]\nstyle = \"enhanced\"").other_checks_open);
	for style in ["guided", "enhanced"] {
		let s = of(&format!(
			"[comment]\nstyle = \"{style}\"\nother_checks = true\nother_checks_open = true"
		));
		assert!(s.other_checks_open);
		assert!(s.options().other_checks_open);
	}
	// The style is the default, so no `style` key means guided, which can fold the table.
	assert!(of("[comment]\nother_checks_open = true").other_checks_open);
	assert!(!of("[comment]\nother_checks_open = false").other_checks_open);
}

#[test]
fn keeping_the_table_open_needs_a_style_that_folds_it() {
	let text = "[comment]\nstyle = \"plain\"\nother_checks_open = true";
	assert!(err(text).contains("`comment.other_checks_open` needs `comment.style`"));
	// Switching it off is always allowed, so a file can move between styles.
	of("[comment]\nstyle = \"plain\"\nother_checks_open = false");
	assert!(err("[comment]\nother_checks_open = \"yes\"").contains("other_checks_open"));
}

#[test]
fn the_updated_note_is_on_except_in_the_plain_style() {
	assert!(of("").updated_note);
	assert!(of("").options().updated_note);
	assert!(of("[comment]\nstyle = \"enhanced\"").updated_note);
	assert!(!of("[comment]\nstyle = \"plain\"").updated_note);
	assert!(!of("[comment]\nupdated_note = false").updated_note);
	assert!(!of("[comment]\nstyle = \"enhanced\"\nupdated_note = false").updated_note);
	// Switching it off is always allowed, so a file can move between styles.
	assert!(!of("[comment]\nstyle = \"plain\"\nupdated_note = false").updated_note);
	assert!(err("[comment]\nstyle = \"plain\"\nupdated_note = true")
		.contains("`comment.updated_note` needs `comment.style`"));
	assert!(err("[comment]\nupdated_note = 1").contains("updated_note"));
}

#[test]
fn the_test_delta_is_on_except_in_the_plain_style() {
	assert!(of("").test_delta);
	assert!(of("").options().test_delta);
	assert!(of("[comment]\nstyle = \"enhanced\"").test_delta);
	assert!(!of("[comment]\nstyle = \"plain\"").test_delta);
	assert!(!of("[comment]\ntest_delta = false").test_delta);
	assert!(!of("[comment]\nstyle = \"plain\"\ntest_delta = false").test_delta);
	assert!(err("[comment]\nstyle = \"plain\"\ntest_delta = true")
		.contains("`comment.test_delta` needs `comment.style`"));
	assert!(err("[comment]\ntest_delta = \"yes\"").contains("test_delta"));
}
