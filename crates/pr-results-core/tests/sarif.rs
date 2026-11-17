// The SARIF reader: findings as cases, thresholds, locations, hostile input and limits.
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

use common::fixture;
use pr_results_core::{parse_sarif, Limits, ParseError, RepoPath, Report, Severity, Status};

fn parse(path: &str, fail_on: Severity) -> Report {
	parse_sarif(&fixture(path), &Limits::default(), None, fail_on)
		.unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn statuses(r: &Report) -> Vec<(String, Status)> {
	r.suites[0]
		.cases
		.iter()
		.map(|c| (c.name.as_str().to_owned(), c.status))
		.collect()
}

fn want(pairs: &[(&str, Status)]) -> Vec<(String, Status)> {
	pairs.iter().map(|(n, s)| ((*n).to_owned(), *s)).collect()
}

#[test]
fn a_run_is_a_suite_and_a_finding_is_a_case_with_a_file_and_a_line() {
	let r = parse("sarif/clippy.sarif", Severity::Warning);
	assert_eq!(r.suites.len(), 1);
	let suite = &r.suites[0];
	assert_eq!(suite.name.as_str(), "clippy");
	let case = &suite.cases[0];
	assert_eq!(case.name.as_str(), "clippy::too_many_arguments");
	assert_eq!(case.status, Status::Failed);
	assert_eq!(
		case.file.as_ref().unwrap().as_str(),
		"crates/pr-results-forge/src/publish.rs"
	);
	assert_eq!(case.line, Some(454));
	// The file is the group a comment shows between the suite and the rule.
	assert_eq!(
		case.classname.as_str(),
		case.file.as_ref().unwrap().as_str()
	);
	assert_eq!(
		case.message.as_ref().unwrap().as_str(),
		"this function has too many arguments (8/7)"
	);
	assert!(case.detail.is_none());
}

#[test]
fn a_message_of_several_lines_keeps_its_first_as_the_message_and_all_as_the_detail() {
	let r = parse("sarif/clippy.sarif", Severity::Warning);
	let case = &r.suites[0].cases[2];
	assert_eq!(
		case.message.as_ref().unwrap().as_str(),
		"item in documentation is missing backticks"
	);
	assert_eq!(
		case.detail.as_ref().unwrap().as_str(),
		"item in documentation is missing backticks\nhelp: try `FooBar`"
	);
	assert_eq!(case.file.as_ref().unwrap().as_str(), "src/a b.rs");
}

#[test]
fn a_finding_fails_at_or_above_the_threshold_and_is_skipped_below_it() {
	use Status::{Failed, Skipped};
	let at = |level| statuses(&parse("sarif/clippy.sarif", level));
	assert_eq!(
		at(Severity::Error),
		want(&[
			("clippy::too_many_arguments", Failed),
			("clippy::needless_return", Skipped),
			("clippy::doc_markdown", Skipped)
		])
	);
	assert_eq!(
		at(Severity::Warning),
		want(&[
			("clippy::too_many_arguments", Failed),
			("clippy::needless_return", Failed),
			("clippy::doc_markdown", Skipped)
		])
	);
	assert_eq!(
		at(Severity::Note),
		want(&[
			("clippy::too_many_arguments", Failed),
			("clippy::needless_return", Failed),
			("clippy::doc_markdown", Failed)
		])
	);
}

#[test]
fn kinds_levels_and_suppressions_follow_the_standard() {
	use Status::{Failed, Passed, Skipped};
	let r = parse("sarif/levels.sarif", Severity::Warning);
	assert_eq!(
		statuses(&r),
		want(&[
			("passes", Passed),
			("informs", Skipped),
			("not-applicable", Skipped),
			("waived", Skipped),
			("silent", Skipped),
			("default-level", Failed),
			("needs-review", Failed),
			("finding", Failed),
			("markdown-only", Failed),
		])
	);
	let markdown = &r.suites[0].cases[8];
	assert_eq!(markdown.message.as_ref().unwrap().as_str(), "**bold** text");
}

#[test]
fn a_run_with_no_findings_is_one_passing_case_so_a_clean_scan_counts() {
	let r = parse("sarif/clean.sarif", Severity::Warning);
	assert_eq!(r.suites[0].name.as_str(), "semgrep");
	assert_eq!(statuses(&r), want(&[("no findings", Status::Passed)]));
}

#[test]
fn every_run_is_a_suite() {
	let r = parse("sarif/multi-run.sarif", Severity::Warning);
	let names: Vec<&str> = r.suites.iter().map(|s| s.name.as_str()).collect();
	assert_eq!(names, ["semgrep", "trivy"]);
	assert_eq!(r.suites[0].cases[0].status, Status::Failed);
	assert_eq!(r.suites[1].cases[0].status, Status::Passed);
}

#[test]
fn the_path_prefix_goes_in_front_of_every_path() {
	let prefix = RepoPath::parse("apps/web/").unwrap();
	let r = parse_sarif(
		&fixture("sarif/clippy.sarif"),
		&Limits::default(),
		Some(&prefix),
		Severity::Warning,
	)
	.unwrap();
	assert_eq!(
		r.suites[0].cases[1].file.as_ref().unwrap().as_str(),
		"apps/web/src/lib.rs"
	);
}

#[test]
fn a_location_that_is_not_a_path_in_the_repository_is_dropped_and_the_finding_is_kept() {
	let r = parse("sarif/hostile.sarif", Severity::Warning);
	let cases = &r.suites[0].cases;
	assert_eq!(cases.len(), 5);
	for (i, what) in [
		(0, "a path above the root"),
		(1, "an absolute path"),
		(2, "a URL"),
	] {
		assert!(cases[i].file.is_none(), "{what}");
		assert!(cases[i].line.is_none(), "{what}");
		assert_eq!(cases[i].status, Status::Failed, "{what}");
	}
	// Only the first location is used, and a line that is zero or does not fit is no line.
	assert_eq!(cases[3].file.as_ref().unwrap().as_str(), "src/x.rs");
	assert_eq!(cases[3].line, None);
	assert_eq!(cases[4].file.as_ref().unwrap().as_str(), "src/ok.rs");
	assert_eq!(cases[4].line, Some(7));
}

#[test]
fn a_prefix_cannot_be_climbed_out_of_the_repository() {
	let prefix = RepoPath::parse("apps/web/").unwrap();
	let json = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"t"}},"results":[
		{"ruleId":"r","level":"error","message":{"text":"m"},
		 "locations":[{"physicalLocation":{"artifactLocation":{"uri":"../../../x.rs"},"region":{"startLine":1}}}]}]}]}"#;
	let r = parse_sarif(json, &Limits::default(), Some(&prefix), Severity::Warning).unwrap();
	assert!(r.suites[0].cases[0].file.is_none());
}

#[test]
fn text_from_a_hostile_report_is_kept_whole_for_the_escaping_layer() {
	// The reader does not escape: it keeps the text, capped, and `Safe` escapes it at the edge.
	let r = parse("sarif/hostile.sarif", Severity::Warning);
	assert!(r.suites[0].name.as_str().contains("@everyone"));
	assert!(r.suites[0].cases[0].name.as_str().contains("@owner/team"));
	assert!(r.suites[0].cases[0]
		.message
		.as_ref()
		.unwrap()
		.as_str()
		.contains("<script>"));
	assert!(
		!format!("{r:?}").contains("@everyone"),
		"Debug never prints report text"
	);
}

#[test]
fn what_is_kept_is_capped_by_the_same_limits_as_a_junit_report() {
	let limits = Limits {
		max_name_bytes: 8,
		max_message_bytes: 12,
		..Limits::default()
	};
	let r = parse_sarif(
		&fixture("sarif/clippy.sarif"),
		&limits,
		None,
		Severity::Warning,
	)
	.unwrap();
	let case = &r.suites[0].cases[0];
	assert_eq!(case.name.len(), 8);
	assert!(case.name.omitted() > 0);
	assert_eq!(case.message.as_ref().unwrap().len(), 12);
}

#[test]
fn unknown_fields_are_skipped_not_stored() {
	// A document that is mostly something this reader has no use for costs only what it keeps.
	let filler = "x".repeat(4_000_000);
	let json = format!(
		"{{\"version\":\"2.1.0\",\"blob\":\"{filler}\",\"runs\":[{{\"tool\":{{\"driver\":{{\"name\":\"t\",\"notes\":[{}]}}}},\"results\":[]}}]}}",
		vec!["[1,2,3]"; 50_000].join(",")
	);
	let r = parse_sarif(json.as_bytes(), &Limits::default(), None, Severity::Warning).unwrap();
	assert_eq!(r.suites[0].cases.len(), 1);
}

#[test]
fn a_byte_order_mark_is_accepted() {
	let mut bytes = vec![0xef, 0xbb, 0xbf];
	bytes.extend(fixture("sarif/clean.sarif"));
	assert!(parse_sarif(&bytes, &Limits::default(), None, Severity::Warning).is_ok());
}

fn err(bytes: &[u8]) -> ParseError {
	parse_sarif(bytes, &Limits::default(), None, Severity::Warning).unwrap_err()
}

#[test]
fn input_that_is_not_a_sarif_document_is_refused_with_a_reason() {
	assert_eq!(err(b""), ParseError::Empty);
	assert_eq!(err(b"  \n "), ParseError::Empty);
	assert!(matches!(
		err(b"{ not json"),
		ParseError::NotJson { line: 1, .. }
	));
	assert!(matches!(err(b"[]"), ParseError::NotJson { .. }));
	assert!(matches!(
		err(br#"{"version":"2.1.0","runs":"x"}"#),
		ParseError::NotJson { .. }
	));
	assert_eq!(
		err(br#"{"version":"1.0.0","runs":[{}]}"#),
		ParseError::Structure("not a SARIF 2 document")
	);
	assert_eq!(
		err(br#"{"runs":[{}]}"#),
		ParseError::Structure("not a SARIF 2 document")
	);
	assert_eq!(
		err(br#"{"version":"2.1.0","runs":[]}"#),
		ParseError::Structure("the document has no runs")
	);
}

#[test]
fn the_limits_are_enforced() {
	let bytes = fixture("sarif/clippy.sarif");
	let with = |limits: Limits| parse_sarif(&bytes, &limits, None, Severity::Warning).unwrap_err();
	assert!(matches!(
		with(Limits {
			max_bytes: 10,
			..Limits::default()
		}),
		ParseError::TooLarge { .. }
	));
	assert_eq!(
		with(Limits {
			max_cases: 2,
			..Limits::default()
		}),
		ParseError::TooManyCases { limit: 2 }
	);
	assert_eq!(
		with(Limits {
			max_text_bytes: 50,
			..Limits::default()
		}),
		ParseError::TooMuchText { limit: 50 }
	);
	let multi = fixture("sarif/multi-run.sarif");
	let suites = Limits {
		max_suites: 1,
		..Limits::default()
	};
	assert_eq!(
		parse_sarif(&multi, &suites, None, Severity::Warning).unwrap_err(),
		ParseError::TooManySuites { limit: 1 }
	);
}

#[test]
fn nesting_in_a_field_that_is_not_used_costs_nothing_and_cannot_overflow_the_stack() {
	// serde_json skips a value it was not asked for without recursing, so depth is not a limit
	// here, and the fields that are read are a handful of levels deep whatever the input holds.
	let depth = 100_000;
	let json = format!(
		"{{\"version\":\"2.1.0\",\"x\":{}{},\"runs\":[{{\"tool\":{{\"driver\":{{\"name\":\"t\"}}}},\"results\":[]}}]}}",
		"[".repeat(depth),
		"]".repeat(depth)
	);
	assert!(parse_sarif(json.as_bytes(), &Limits::default(), None, Severity::Warning).is_ok());
}

#[test]
fn nesting_in_a_field_that_is_used_is_just_a_document_of_the_wrong_shape() {
	let json = format!(
		"{{\"version\":\"2.1.0\",\"runs\":{}{}}}",
		"[".repeat(5_000),
		"]".repeat(5_000)
	);
	assert!(matches!(err(json.as_bytes()), ParseError::NotJson { .. }));
}

#[test]
fn error_messages_quote_nothing_from_the_input() {
	let secret = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"SECRET-TOKEN-VALUE"}},"results":[{"level": SECRET-TOKEN-VALUE}]}]}"#;
	let message = err(secret).to_string();
	assert!(!message.contains("SECRET"), "{message}");
	assert!(message.contains("line 1"), "{message}");
}

#[test]
fn every_strict_prefix_of_a_document_is_refused() {
	for name in ["clippy", "clean", "multi-run", "levels", "hostile"] {
		let bytes = fixture(&format!("sarif/{name}.sarif"));
		let end = bytes
			.iter()
			.rposition(|b| !b.is_ascii_whitespace())
			.unwrap() + 1;
		assert!(parse_sarif(&bytes[..end], &Limits::default(), None, Severity::Warning).is_ok());
		for cut in 0..end {
			let result = parse_sarif(&bytes[..cut], &Limits::default(), None, Severity::Warning);
			assert!(result.is_err(), "{name} cut at {cut} of {end} was accepted");
		}
	}
}

#[test]
fn input_with_invalid_utf8_is_refused_not_guessed_at() {
	let mut bytes = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":""#.to_vec();
	bytes.extend([0xff, 0xfe]);
	bytes.extend(br#""}},"results":[]}]}"#);
	assert!(matches!(err(&bytes), ParseError::NotJson { .. }));
}
