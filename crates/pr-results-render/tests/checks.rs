// Tests for the check runs, the annotations, the no-results comment and the job summary.
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

use common::{ctx, load, parse_text, section, synthetic_nextest, Scenario};
use pr_results_core::{parse_junit, Flavour, Limits, Marker};
use pr_results_render::{
	job_summary, no_results, Conclusion, Input, Level, Options, Section, Symbols,
};

fn one(files: &[&str], flavour: Flavour, required: bool) -> Scenario {
	Scenario::new(
		"t",
		"Example checks",
		vec![section("Suite", required, load(files, flavour))],
	)
}

fn out(sc: &Scenario, index: usize) -> pr_results_render::CheckOutput {
	sc.with_ctx(|i| pr_results_render::check(i, index)).unwrap()
}

#[test]
fn a_section_that_is_not_there_has_no_check() {
	let sc = one(&["nextest/pass.xml"], Flavour::Nextest, false);
	assert!(sc.with_ctx(|i| pr_results_render::check(i, 1)).is_none());
}

#[test]
fn conclusions() {
	let table = [
		(
			one(&["nextest/pass.xml"], Flavour::Nextest, false),
			Conclusion::Success,
		),
		(
			one(&["nextest/flaky.xml"], Flavour::Nextest, false),
			Conclusion::Success,
		),
		(
			one(&["nextest/fail.xml"], Flavour::Nextest, false),
			Conclusion::Failure,
		),
		(
			one(&["nextest/retries-exhausted.xml"], Flavour::Nextest, false),
			Conclusion::Failure,
		),
		(one(&[], Flavour::Nextest, false), Conclusion::Neutral),
		(one(&[], Flavour::Nextest, true), Conclusion::Failure),
	];
	for (sc, want) in table {
		assert_eq!(out(&sc, 0).conclusion, want);
	}
}

#[test]
fn titles_read_like_the_spec() {
	let table = [
		(
			one(&["nextest/pass.xml"], Flavour::Nextest, false),
			"3 passed, 1 skipped",
		),
		(
			one(&["nextest/flaky.xml"], Flavour::Nextest, false),
			"1 flaky",
		),
		(
			one(&["vitest/fail-with-stack.xml"], Flavour::Vitest, false),
			"1 failed, 1 passed",
		),
		(
			one(&["nextest/retries-exhausted.xml"], Flavour::Nextest, false),
			"2 failed, 1 skipped",
		),
		(
			one(&[], Flavour::Nextest, false),
			"No results were uploaded",
		),
	];
	for (sc, want) in table {
		assert_eq!(out(&sc, 0).title.as_str(), want);
	}
	let big = Scenario::new(
		"t",
		"x",
		vec![section(
			"S",
			false,
			parse_text(&synthetic_nextest("n", 1, 1, 1977, 1), Flavour::Nextest),
		)],
	);
	assert_eq!(
		out(&big, 0).title.as_str(),
		"1 failed, 1\u{202f}977 passed, 1 flaky"
	);
}

#[test]
fn the_summary_carries_the_row_the_suites_and_the_footer() {
	let sc = one(
		&["steps/groups-split-a.xml", "steps/groups-split-b.xml"],
		Flavour::Steps,
		true,
	);
	let o = out(&sc, 0);
	let s = o.summary.as_str();
	assert!(s.starts_with("| Suite | Result |"));
	assert!(s.contains("| Group | Passed |"));
	assert!(
		s.contains("| `Host tests` | 3 | 0 | 0 | 0 | 7.2 s |"),
		"{s}"
	);
	assert!(s.contains("| `Targets` | 1 | 0 | 0 | 0 | 13.1 s |"), "{s}");
	assert!(s.contains("pr-results 0.1.0"));
}

#[test]
fn the_text_ends_with_the_ten_slowest_tests() {
	let sc = Scenario::new(
		"t",
		"x",
		vec![section(
			"S",
			false,
			parse_text(&synthetic_nextest("n", 2, 3, 30, 0), Flavour::Nextest),
		)],
	);
	let o = out(&sc, 0);
	let text = o.text.as_str();
	let (details, slow) = text.split_once("#### Slowest tests").unwrap();
	assert!(details.contains("##### 1. "));
	assert!(details.contains("##### 2. "));
	assert_eq!(
		slow.lines().filter(|l| l.starts_with("| `")).count(),
		10,
		"{slow}"
	);
	// Longest first.
	let times: Vec<f64> = slow
		.lines()
		.filter(|l| l.starts_with("| `"))
		.map(|l| {
			l.trim_end_matches(" s |")
				.rsplit(' ')
				.next()
				.unwrap()
				.parse()
				.unwrap()
		})
		.collect();
	assert!(times.windows(2).all(|w| w[0] >= w[1]), "{times:?}");
}

#[test]
fn a_failure_shows_its_earlier_attempts_and_those_that_were_not_kept() {
	let xml = std::fs::read(common::fixtures_dir().join("nextest/retries-exhausted.xml")).unwrap();
	let limits = Limits {
		max_attempts: 1,
		..Limits::default()
	};
	let suites = parse_junit(&xml, Flavour::Nextest, &limits).unwrap().suites;
	assert!(suites
		.iter()
		.flat_map(|s| &s.cases)
		.any(|c| c.attempts_omitted == 1));
	let sc = Scenario::new("t", "x", vec![section("S", false, suites)]);
	let body = sc.comment().body.to_string();
	assert!(
		body.contains("Earlier attempts: 1 shown, 1 more not kept"),
		"{body}"
	);
	let o = out(&sc, 0);
	assert!(o
		.summary
		.as_str()
		.contains("Earlier attempts: 1 shown, 1 more not kept"));
}

#[test]
fn a_flaky_test_says_how_often_it_failed_and_what_was_not_kept() {
	let xml = std::fs::read(common::fixtures_dir().join("nextest/flaky.xml")).unwrap();
	let limits = Limits {
		max_attempts: 1,
		..Limits::default()
	};
	let suites = parse_junit(&xml, Flavour::Nextest, &limits).unwrap().suites;
	let sc = Scenario::new("t", "x", vec![section("S", false, suites)]);
	let body = sc.comment().body.to_string();
	assert!(
		body.contains("failed 2 times, passed on attempt 3 (1 attempts not kept)"),
		"{body}"
	);
}

#[test]
fn annotations_land_on_the_panic_location() {
	let sc = one(&["nextest/fail.xml"], Flavour::Nextest, false);
	let o = out(&sc, 0);
	assert_eq!(o.annotations.len(), 1);
	let a = &o.annotations[0];
	assert_eq!(a.path.as_str(), "crates/pr-results-core/src/junit.rs");
	assert_eq!((a.start_line, a.end_line), (341, 341));
	assert_eq!(a.level, Level::Failure);
	assert_eq!(a.title.as_str(), "junit::sums_suite_time");
	assert!(a
		.message
		.as_str()
		.contains("assertion `left == right` failed"));
	assert_eq!(o.annotations_capped, 0);
}

#[test]
fn annotations_are_plain_text_not_markdown() {
	let xml = "<testsuites><testsuite name=\"s\">\
		<testcase classname=\"c\" name=\"@everyone *bold* #1\" file=\"src/a.rs\" line=\"7\">\
		<failure message=\"x &lt;b&gt; GH-1\">line one\nline two\u{202e}</failure></testcase>\
		<testcase classname=\"c\" name=\"outside\" file=\"../../etc/passwd\" line=\"1\"><failure message=\"x\"/></testcase>\
		<testcase classname=\"c\" name=\"no file\"><failure message=\"x\"/></testcase>\
		</testsuite></testsuites>";
	let sc = Scenario::new(
		"t",
		"x",
		vec![section("S", false, parse_text(xml, Flavour::Generic))],
	);
	let o = out(&sc, 0);
	assert_eq!(
		o.annotations.len(),
		1,
		"only a path inside the repository is annotated"
	);
	let a = &o.annotations[0];
	assert_eq!(a.path.as_str(), "src/a.rs");
	assert_eq!(a.start_line, 7);
	assert_eq!(a.title.as_str(), "@everyone *bold* #1");
	assert_eq!(
		a.message.as_str(),
		"x <b> GH-1\nline one\nline two\\u{202e}"
	);
}

#[test]
fn a_file_without_a_line_starts_at_line_one() {
	let xml = "<testsuites><testsuite name=\"s\">\
		<testcase classname=\"c\" name=\"a\" file=\"src/a.rs\"><failure message=\"x\"/></testcase>\
		<testcase classname=\"c\" name=\"b\" file=\"src/b.rs\" line=\"0\"><failure message=\"x\"/></testcase>\
		</testsuite></testsuites>";
	let sc = Scenario::new(
		"t",
		"x",
		vec![section("S", false, parse_text(xml, Flavour::Generic))],
	);
	let lines: Vec<u32> = out(&sc, 0)
		.annotations
		.iter()
		.map(|a| a.start_line)
		.collect();
	assert_eq!(lines, [1, 1]);
}

#[test]
fn the_annotation_cap_is_stated_and_never_above_500() {
	let mut xml = String::from("<testsuites><testsuite name=\"s\">");
	for k in 0..700 {
		xml.push_str(&format!(
			"<testcase classname=\"c\" name=\"t{k}\" file=\"src/a.rs\" line=\"{}\"><failure message=\"x\"/></testcase>",
			k + 1
		));
	}
	xml.push_str("</testsuite></testsuites>");
	let suites = parse_text(&xml, Flavour::Generic);
	let make = |cap| {
		let mut sc = Scenario::new("t", "x", vec![section("S", false, suites.clone())]);
		sc.options.max_annotations = cap;
		sc
	};
	let o = out(&make(50), 0);
	assert_eq!((o.annotations.len(), o.annotations_capped), (50, 650));
	assert!(o
		.summary
		.as_str()
		.contains("Annotations: 50 shown, 650 not shown (the cap is 50)."));
	assert_eq!(o.annotations[0].title.as_str(), "t0", "input order is kept");
	let o = out(&make(1000), 0);
	assert_eq!((o.annotations.len(), o.annotations_capped), (500, 200));
	assert!(o.summary.as_str().contains("(the cap is 500)"));
	let o = out(&make(0), 0);
	assert_eq!((o.annotations.len(), o.annotations_capped), (0, 700));
}

#[test]
fn flaky_cases_are_annotated_as_warnings_only_when_asked() {
	let xml = "<testsuites><testsuite name=\"s\">\
		<testcase classname=\"c\" name=\"bad\" file=\"src/a.rs\" line=\"3\"><failure message=\"x\"/></testcase>\
		<testcase classname=\"c\" name=\"shaky\" file=\"src/b.rs\" line=\"4\"><flakyFailure message=\"x\"/></testcase>\
		</testsuite></testsuites>";
	let suites = parse_text(xml, Flavour::Generic);
	let mut sc = Scenario::new("t", "x", vec![section("S", false, suites)]);
	assert_eq!(out(&sc, 0).annotations.len(), 1);
	sc.options.flaky_annotations = true;
	let o = out(&sc, 0);
	let levels: Vec<Level> = o.annotations.iter().map(|a| a.level).collect();
	assert_eq!(levels, [Level::Failure, Level::Warning]);
	assert_eq!(o.annotations[1].path.as_str(), "src/b.rs");
}

#[test]
fn paths_the_tree_lacks_are_left_out_and_the_summary_counts_what_is_kept() {
	let xml = "<testsuites><testsuite name=\"s\">\
		<testcase classname=\"c\" name=\"a\" file=\"src/a.rs\" line=\"3\"><failure message=\"x\"/></testcase>\
		<testcase classname=\"c\" name=\"b\" file=\"src/gone.rs\" line=\"4\"><failure message=\"x\"/></testcase>\
		</testsuite></testsuites>";
	let sc = Scenario::new(
		"t",
		"x",
		vec![section("S", false, parse_text(xml, Flavour::Generic))],
	);
	let o = sc
		.with_ctx(|i| pr_results_render::check_with_paths(i, 0, &|p| p.as_str() != "src/gone.rs"))
		.unwrap();
	assert_eq!(o.annotations.len(), 1);
	assert_eq!(o.annotations[0].path.as_str(), "src/a.rs");
	assert_eq!(
		o.annotations_capped, 0,
		"a left-out path is not a capped one"
	);
	assert!(o.summary.as_str().contains("Annotations: 1."));
	let all = out(&sc, 0);
	assert_eq!(all.annotations.len(), 2);
}

#[test]
fn check_with_paths_keeps_the_title_and_message_limits() {
	let long_name = "n".repeat(600);
	let long_body = "b".repeat(200_000);
	let xml = format!(
		"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"{long_name}\" file=\"src/a.rs\" line=\"3\"><failure message=\"x\">{long_body}</failure></testcase></testsuite></testsuites>"
	);
	let sc = Scenario::new(
		"t",
		"x",
		vec![section("S", false, parse_text(&xml, Flavour::Generic))],
	);
	let o = sc
		.with_ctx(|i| pr_results_render::check_with_paths(i, 0, &|_| true))
		.unwrap();
	let a = &o.annotations[0];
	assert!(
		a.title.as_str().chars().count() <= 255,
		"{}",
		a.title.as_str().len()
	);
	assert!(a.title.as_str().encode_utf16().count() <= 255);
	assert!(
		a.message.as_str().len() <= 65_535,
		"{}",
		a.message.as_str().len()
	);
	assert_eq!(
		a,
		&out(&sc, 0).annotations[0],
		"the same as the unfiltered check"
	);
}

#[test]
fn no_results_is_a_short_comment_with_a_marker() {
	let c = no_results(&ctx(), "Example checks");
	let body = c.body.as_str();
	assert!(body.starts_with("<!-- pr-results:1 key=ci sha="));
	assert!(Marker::parse(body).is_some());
	assert!(body.contains("### Example checks: No results\n"));
	assert!(body.contains("\nNo results were uploaded for this run.\n"));
	assert!(c.steps_taken.is_empty());
	// An optional section with nothing in it says the same.
	let sc = Scenario::new("t", "Example checks", vec![section("S", false, Vec::new())]);
	assert_eq!(sc.comment(), c);
	assert_eq!(
		Scenario::new("t", "Example checks", Vec::new()).comment(),
		c
	);
}

#[test]
fn a_required_section_with_nothing_is_a_failure_in_the_comment_too() {
	let sc = Scenario::new(
		"t",
		"Example checks",
		vec![section("Desk steps", true, Vec::new())],
	);
	let body = sc.comment().body.to_string();
	assert!(body.contains("### Example checks: No results from 1 required suite\n"));
	assert!(body.contains("No results were uploaded for `Desk steps`."));
}

#[test]
fn the_job_summary_is_the_comment_without_its_marker() {
	let sc = one(&["nextest/fail.xml"], Flavour::Nextest, false);
	let c = sc.comment();
	let summary = job_summary(&c);
	assert!(c.body.as_str().starts_with("<!--"));
	assert!(!summary.as_str().contains("<!--"));
	assert_eq!(
		format!("{}\n{}", c.body.as_str().lines().next().unwrap(), summary),
		c.body.to_string()
	);
}

#[test]
fn a_long_title_is_cut_visibly() {
	let long = "t".repeat(500);
	let sc = Scenario::new(
		"t",
		&long,
		vec![section(
			"S",
			false,
			load(&["nextest/pass.xml"], Flavour::Nextest),
		)],
	);
	let body = sc.comment().body.to_string();
	let heading = body.lines().nth(1).unwrap();
	assert!(
		heading.contains("(380 more characters omitted)"),
		"{heading}"
	);
	let empty = Scenario::new(
		"t",
		"  ",
		vec![section(
			"S",
			false,
			load(&["nextest/pass.xml"], Flavour::Nextest),
		)],
	);
	assert!(empty
		.comment()
		.body
		.as_str()
		.contains("### Test results: All 3 passed"));
}

#[test]
fn with_more_than_one_section_a_failure_names_its_section() {
	let sc = Scenario::new(
		"t",
		"x",
		vec![
			section(
				"Rust tests",
				false,
				load(&["nextest/fail.xml"], Flavour::Nextest),
			),
			section(
				"Frontend",
				false,
				load(&["vitest/fail-with-stack.xml"], Flavour::Vitest),
			),
		],
	);
	let body = sc.comment().body.to_string();
	assert!(body.contains("`Rust tests` › `pr-results-core` › `junit::sums_suite_time`"));
	assert!(body.contains("`Frontend` › `src/lib/format.test.ts` › `formatDuration > rounds up`"));
	// And with one, it does not.
	let single = one(&["nextest/fail.xml"], Flavour::Nextest, false);
	assert!(single
		.comment()
		.body
		.as_str()
		.contains("\n`pr-results-core` › `junit::sums_suite_time`\n"));
}

#[test]
fn the_group_of_a_step_is_what_follows_the_last_colon_or_full_stop() {
	let sc = one(&["steps/desk-job.xml"], Flavour::Steps, true);
	let body = sc.comment().body.to_string();
	assert!(
		body.contains("`check (host)` › `Host tests` › `shell crates, host tests`"),
		"{body}"
	);
	let sc = one(&["steps/step-fail.xml"], Flavour::Steps, true);
	let body = sc.comment().body.to_string();
	// The group repeats the suite's name, so it is shown once.
	assert!(
		body.contains("\n`Host tests` › `shell crates, host tests`\n"),
		"{body}"
	);
	assert!(body.contains("Last 7 of 7 lines") || body.contains("test gfx::fills_rect ... FAILED"));
}

#[test]
fn a_step_failure_shows_the_end_of_its_log_and_a_test_the_start() {
	let sc = one(&["steps/desk-job.xml"], Flavour::Steps, true);
	let body = sc.comment().body.to_string();
	assert!(body.contains("Last 20 of 31 lines:"), "{body}");
	assert!(body.contains("could not compile `desk-gfx`"));
	assert!(!body.contains("[1/24]"));
	let mut sc = Scenario::new(
		"t",
		"x",
		vec![section(
			"S",
			false,
			parse_text(&synthetic_nextest("n", 1, 40, 0, 0), Flavour::Nextest),
		)],
	);
	sc.options.excerpt_lines = 5;
	let body = sc.comment().body.to_string();
	assert!(body.contains("First 5 of 41 lines:"), "{body}");
	assert!(body.contains("panicked at crates/n/src/m0.rs"));
}

#[test]
fn glyphs_precede_the_status_words() {
	let sc = common::scenarios()
		.into_iter()
		.find(|s| s.name == "glyphs")
		.unwrap();
	let body = sc.comment().body.to_string();
	assert!(body.contains("\u{2717} **Failed**"));
	assert!(body.contains("\u{2713} Passed"));
	assert!(body.contains("\u{2013} No results"));
	assert!(body.contains("\u{21bb} "));
	let text = Scenario {
		options: Options {
			symbols: Symbols::Text,
			..sc.options.clone()
		},
		..sc
	};
	let plain = text.comment().body.to_string();
	assert!(
		!plain.contains('\u{2717}') && !plain.contains('\u{2713}') && !plain.contains('\u{21bb}')
	);
}

#[test]
fn inputs_that_were_skipped_are_always_said() {
	let suites = load(&["nextest/pass.xml"], Flavour::Nextest);
	let mut s = section("S", false, suites);
	s.inputs_skipped = 1;
	let sc = Scenario::new("t", "x", vec![s]);
	assert!(sc
		.comment()
		.body
		.as_str()
		.contains("1 input file was skipped because it could not be read"));
	assert!(out(&sc, 0)
		.summary
		.as_str()
		.contains("1 input file was skipped"));
}

#[test]
fn input_borrows_only_what_it_needs() {
	// The public API takes plain borrowed parts, so a caller needs no clone of the model.
	let ctx = ctx();
	let suites = load(&["nextest/pass.xml"], Flavour::Nextest);
	let sections = [Section {
		title: "S",
		check_name: "Example S",
		required: false,
		suites: &suites,
		inputs_skipped: 0,
	}];
	let options = Options::default();
	let input = Input {
		ctx: &ctx,
		title: "T",
		sections: &sections,
		options: &options,
	};
	assert!(pr_results_render::comment(&input).steps_taken.is_empty());
}

fn annotated(name: &str, message: &str) -> pr_results_render::CheckOutput {
	let xml = format!(
		"<testsuites><testsuite name=\"s\">\
		<testcase classname=\"c\" name=\"{name}\" file=\"src/a.rs\" line=\"7\">\
		<failure message=\"{message}\"/></testcase></testsuite></testsuites>"
	);
	let sc = Scenario::new(
		"t",
		"x",
		vec![section("S", false, parse_text(&xml, Flavour::Generic))],
	);
	out(&sc, 0)
}

fn utf16(s: &str) -> usize {
	s.encode_utf16().count()
}

#[test]
fn an_annotation_title_never_passes_255_even_with_its_marker() {
	for name in [
		"n".repeat(1_000),
		"\u{1f600}".repeat(1_000),
		"\u{202e}".repeat(1_000),
		"ab\u{1f600}".repeat(400),
		"n".repeat(255),
		"n".repeat(256),
	] {
		let o = annotated(&name, "x");
		let title = o.annotations[0].title.as_str();
		assert!(
			title.chars().count() <= 255,
			"{} chars",
			title.chars().count()
		);
		assert!(utf16(title) <= 255, "{} units", utf16(title));
		if name.chars().count() > 255 {
			assert!(title.contains("more characters omitted"), "{title}");
		}
	}
	let short = annotated(&"n".repeat(255), "x");
	assert_eq!(short.annotations[0].title.as_str(), "n".repeat(255));
}

#[test]
fn an_annotation_message_stays_under_64_kib_even_when_every_character_is_wide() {
	for message in [
		"\u{1f600}".repeat(100_000),
		"m".repeat(100_000),
		"\u{202e}".repeat(100_000),
	] {
		let o = annotated("t", &message);
		let m = o.annotations[0].message.as_str();
		assert!(m.len() <= 65_535, "{} bytes", m.len());
		assert!(m.contains("more characters omitted"));
	}
}

#[test]
fn a_check_run_title_stays_under_255() {
	let o = annotated("t", "x");
	assert!(o.title.len_chars() <= 255);
}
