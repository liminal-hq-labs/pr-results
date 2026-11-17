// Tests for the guided and enhanced styles: the words, the bar, the commands and the limits.
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

use std::fmt::Write as _;

use common::{
	assert_inert_all, guided_truncation_base, other, parse_text, scenarios, section, Scenario,
};
use pr_results_core::Flavour;
use pr_results_render::{measure, CheckState, Degradation, Style, Symbols, Target};

/// A nextest report with `passed` passing and `failed` failing tests, each 200 ms.
fn report(passed: usize, failed: usize) -> String {
	let mut cases = String::new();
	for i in 0..passed {
		let _ = write!(
			cases,
			"<testcase name=\"ok_{i}\" classname=\"app\" time=\"0.2\"/>"
		);
	}
	for i in 0..failed {
		let _ = write!(
			cases,
			"<testcase name=\"bad_{i}\" classname=\"app\" time=\"0.2\"><failure message=\"boom\"/></testcase>"
		);
	}
	format!("<testsuites><testsuite name=\"app\">{cases}</testsuite></testsuites>")
}

fn run(passed: usize, failed: usize) -> Scenario {
	Scenario::new(
		"t",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			parse_text(&report(passed, failed), Flavour::Nextest),
		)],
	)
	.comment_only()
}

fn body(sc: &Scenario) -> String {
	sc.comment().body.to_string()
}

#[test]
fn the_verb_agrees_with_what_needs_attention() {
	assert!(
		body(&run(5, 1).guided()).contains("\n1 failed test needs attention. 5 tests passed.\n")
	);
	assert!(
		body(&run(5, 2).guided()).contains("\n2 failed tests need attention. 5 tests passed.\n")
	);
	let with_check = run(5, 1)
		.guided()
		.with_other_checks(vec![other("Audit", CheckState::Failed)]);
	assert!(body(&with_check).contains("\n1 failed test and 1 other check need attention."));
	let only_check = run(5, 0)
		.guided()
		.with_other_checks(vec![other("Audit", CheckState::Failed)]);
	assert!(body(&only_check).contains("\n1 other check needs attention. 5 tests passed.\n"));
}

#[test]
fn a_full_bar_is_for_a_run_with_no_failures_and_never_for_one_with() {
	let bar = |passed, failed| {
		body(&run(passed, failed).enhanced())
			.lines()
			.find(|l| l.contains(" tests passed "))
			.unwrap()
			.to_owned()
	};
	assert!(bar(10, 0).ends_with("10 of 10 tests passed \u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}"));
	// 999 of 1 000 is 99.9%, which rounds down to nine tenths: a full bar would hide the failure.
	assert!(bar(999, 1).ends_with(
		"\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b1}"
	));
	assert!(bar(0, 3).ends_with(
		"\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}"
	));
	assert!(bar(1, 1).ends_with(
		"\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}"
	));
}

#[test]
fn the_sentence_comes_before_the_bar_so_a_screen_reader_reads_words_first() {
	let text = body(&run(3, 0).enhanced());
	let sentence = text.find("3 of 3 tests passed").unwrap();
	let bar = text.find('\u{25b0}').unwrap();
	assert!(sentence < bar);
}

#[test]
fn no_run_means_no_bar() {
	let sc = Scenario::new(
		"t",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			parse_text(
				"<testsuites><testsuite name=\"a\"/></testsuites>",
				Flavour::Nextest,
			),
		)],
	)
	.comment_only()
	.enhanced();
	assert!(!body(&sc).contains('\u{25b0}'));
}

#[test]
fn the_extras_do_nothing_in_the_plain_style() {
	let mut sc = run(3, 1);
	sc.options.mascot = true;
	sc.options.progress = true;
	sc.options.alerts = true;
	sc.options.symbols = Symbols::Text;
	assert_eq!(sc.options.style, Style::Plain);
	let text = body(&sc);
	for word in [
		"Jax",
		"[!",
		"\u{25b0}",
		"Everything",
		"need attention",
		"Run it locally",
	] {
		assert!(!text.contains(word), "{word:?} in\n{text}");
	}
}

#[test]
fn an_alert_is_written_only_for_github() {
	for sc in scenarios().iter().filter(|s| s.options.alerts) {
		let text = body(sc);
		assert_eq!(
			text.lines().any(|l| l.starts_with("> [!")),
			sc.target == Target::GitHub,
			"{}",
			sc.name
		);
		assert_eq!(
			text.lines().any(|l| l.starts_with('>')),
			sc.target == Target::GitHub,
			"{}",
			sc.name
		);
	}
}

#[test]
fn the_alert_holds_only_fixed_sentences_and_never_a_name() {
	for sc in scenarios().iter().filter(|s| s.options.alerts) {
		for line in body(sc).lines().filter(|l| l.starts_with('>')) {
			let marker = ["TIP", "NOTE", "WARNING", "CAUTION"]
				.iter()
				.any(|kind| line == format!("> [!{kind}]"));
			if marker || line == ">" {
				continue;
			}
			assert!(line.starts_with("> "), "{}: {line}", sc.name);
			assert!(!line.contains('`'), "{}: code in an alert: {line}", sc.name);
			assert!(
				!line.contains('['),
				"{}: a link in an alert: {line}",
				sc.name
			);
		}
	}
}

#[test]
fn jax_speaks_only_in_a_closed_set_of_sentences() {
	let allowed = [
		"\u{1f9a6} **Jax:** All clear. Every test passed.",
		"\u{1f9a6} **Jax:** Everything passed, though a test needed a retry. Worth a look.",
		"\u{1f9a6} **Jax:** Something broke. The details are below.",
		"\u{1f9a6} **Jax:** The tests are fine, but something else needs a look.",
		"\u{1f9a6} **Jax:** I could not find any test results to show.",
	];
	let mut heard = 0;
	for sc in scenarios() {
		for line in body(&sc).lines().filter(|l| l.contains("Jax")) {
			let said = line.trim_start_matches("> ");
			assert!(allowed.contains(&said), "{}: {line}", sc.name);
			heard += 1;
		}
		if !sc.options.mascot {
			assert!(!body(&sc).contains("Jax"), "{}", sc.name);
		}
	}
	assert!(heard >= 5);
}

#[test]
fn the_guided_words_never_say_a_pull_request_is_ready() {
	// The comment is written by a bot, and a person or a reviewing agent may take it for a verdict.
	for sc in scenarios()
		.iter()
		.filter(|s| s.options.style == Style::Guided)
	{
		let text = body(sc).to_lowercase();
		for word in ["merge", "approve", "lgtm", "ready", "safe to", "ship it"] {
			assert!(!text.contains(word), "{}: {word:?}", sc.name);
		}
	}
}

#[test]
fn a_command_is_built_only_from_names_that_are_safe_in_a_shell() {
	let names = [
		("fine::name_1", true),
		("a_b::c_d::e", true),
		("quote'x", false),
		("dq\"x", false),
		("has space", false),
		("tick`x`", false),
		("dollar$x", false),
		("semi;x", false),
		("pipe|x", false),
		("amp&x", false),
		("paren(x)", false),
		("brace{x}", false),
		("star*x", false),
		("hash#x", false),
		("back\\slash", false),
		("new\nline", false),
		("caf\u{e9}", false),
		("9starts_with_digit", false),
		("-starts_with_dash", false),
	];
	for (name, safe) in names {
		let xml = format!(
			"<testsuites><testsuite name=\"app\"><testcase name=\"{}\" classname=\"app\" time=\"0.2\"><failure message=\"m\"/></testcase></testsuite></testsuites>",
			name.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
		);
		let sc = Scenario::new(
			"t",
			"Example checks",
			vec![section(
				"Rust tests",
				false,
				parse_text(&xml, Flavour::Nextest),
			)],
		)
		.comment_only()
		.guided();
		let text = body(&sc);
		assert_eq!(text.contains("Run it locally"), safe, "{name:?}\n{text}");
		if safe {
			assert!(
				text.contains(&format!("-E 'binary_id(=app) & test(={name})'")),
				"{name:?}"
			);
		}
	}
}

#[test]
fn a_binary_id_is_held_to_the_same_rule() {
	for (binary, safe) in [
		("pkg::bin", true),
		("pkg/bin", true),
		("pk g", false),
		("p'k", false),
	] {
		let xml = format!(
			"<testsuites><testsuite name=\"s\"><testcase name=\"t\" classname=\"{binary}\" time=\"0.2\"><failure message=\"m\"/></testcase></testsuite></testsuites>"
		);
		let sc = Scenario::new(
			"t",
			"Example checks",
			vec![section(
				"Rust tests",
				false,
				parse_text(&xml, Flavour::Nextest),
			)],
		)
		.comment_only()
		.guided();
		assert_eq!(body(&sc).contains("Run it locally"), safe, "{binary:?}");
	}
}

#[test]
fn a_suite_can_name_its_own_prefix_and_turn_the_command_off() {
	let mut sc = run(0, 1).guided();
	sc.sections[0].run_locally = Some("cargo nextest run --all-features".to_owned());
	assert!(
		body(&sc).contains("cargo nextest run --all-features -E 'binary_id(=app) & test(=bad_0)'")
	);
	sc.sections[0].run_locally = Some(String::new());
	assert!(!body(&sc).contains("Run it locally"));
	// A prefix that is not printable ASCII, or holds a quote, is not used, even if a settings file
	// let it through.
	for bad in ["cargo 'x'", "cargo `x`", "caf\u{e9}", "a\nb"] {
		sc.sections[0].run_locally = Some(bad.to_owned());
		assert!(!body(&sc).contains("Run it locally"), "{bad:?}");
	}
	// Another flavour has no command, whatever the settings say.
	let mut other_flavour = Scenario::new(
		"t",
		"Example checks",
		vec![section(
			"Web",
			false,
			parse_text(
				"<testsuites><testsuite name=\"s\"><testcase name=\"t\" classname=\"c\"><failure message=\"m\"/></testcase></testsuite></testsuites>",
				Flavour::Generic,
			),
		)],
	)
	.comment_only()
	.guided();
	other_flavour.sections[0].run_locally = Some("cargo nextest run".to_owned());
	assert!(!body(&other_flavour).contains("Run it locally"));
}

#[test]
fn only_tests_that_took_a_moment_are_listed_as_slow() {
	let quick = "<testsuites><testsuite name=\"s\"><testcase name=\"t\" classname=\"c\" time=\"0.05\"/></testsuite></testsuites>";
	let sc = Scenario::new(
		"t",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			parse_text(quick, Flavour::Nextest),
		)],
	)
	.comment_only()
	.guided();
	assert!(!body(&sc).contains("Slowest tests"));
	assert!(body(&run(3, 0).guided()).contains("<summary>Slowest tests (3)</summary>"));
}

#[test]
fn the_heading_has_a_mark_only_with_emoji() {
	assert!(body(&run(3, 0).guided()).contains("### Example checks:"));
	assert!(body(&run(3, 0).enhanced()).contains("### \u{2705} Example checks:"));
	assert!(body(&run(3, 1).enhanced()).contains("### \u{274c} Example checks:"));
}

#[test]
fn the_slowest_list_is_the_first_thing_the_budget_gives_up_with_the_flaky_list() {
	let base = guided_truncation_base();
	let full = body(&base);
	assert!(
		full.contains("Slowest tests"),
		"the base has slow tests to list"
	);
	let steps: Vec<Degradation> = common::limits_of(&base)
		.into_iter()
		.map(|(s, _)| s)
		.collect();
	assert_eq!(
		steps.first(),
		Some(&Degradation::OtherChecksCollapsed),
		"{steps:?}"
	);
	assert!(steps.contains(&Degradation::ExtrasToCounts));
	let (_, limit) = common::limits_of(&base)
		.into_iter()
		.find(|(s, _)| *s == Degradation::ExtrasToCounts)
		.unwrap();
	let mut sc = base.clone();
	sc.options.comment_limit = limit;
	let text = body(&sc);
	assert!(!text.contains("<summary>Slowest tests"), "{text}");
	assert!(text.contains("The list of slowest tests was left out to fit GitHub's size limit."));
}

#[test]
fn every_guided_and_enhanced_comment_fits_at_every_limit_and_stays_inert() {
	for base in scenarios()
		.into_iter()
		.filter(|s| s.options.style == Style::Guided && !s.checks)
	{
		for limit in [400, 700, 1_000, 2_000, 4_000, 10_000, 60_000] {
			let mut sc = base.clone();
			sc.options.comment_limit = limit;
			let c = sc.comment();
			if limit >= 1_000 {
				assert!(
					measure(&c.body) <= limit,
					"{} at {limit}: {}",
					sc.name,
					measure(&c.body)
				);
			}
			assert_inert_all(&sc);
		}
	}
}

#[test]
fn the_smallest_guided_comment_keeps_the_heading_the_totals_and_the_other_checks_line() {
	let mut sc = guided_truncation_base();
	sc.options.comment_limit = 700;
	let c = sc.comment();
	assert_eq!(c.steps_taken.last(), Some(&Degradation::Minimal));
	let text = c.body.to_string();
	assert!(text.contains("1 other check needs attention"), "{text}");
	assert!(
		text.contains("Other checks: 1 failing, 6 passed."),
		"{text}"
	);
	for dropped in ["Run it locally", "Slowest tests", "need attention."] {
		assert!(!text.contains(dropped), "{dropped}\n{text}");
	}
}
