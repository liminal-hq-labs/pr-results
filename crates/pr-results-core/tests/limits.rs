// Tests for the limits that keep hostile input from costing more than a stated amount.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use std::time::{Duration, Instant};

use pr_results_core::{
	parse_junit, Case, Flavour, Limits, ParseError, Report, Status, Suite, Untrusted,
};

fn suite(name: &str, flavour: Flavour, cases: &[&str]) -> Suite {
	Suite {
		name: Untrusted::new(name),
		project: None,
		flavour,
		cases: cases
			.iter()
			.map(|c| Case {
				name: Untrusted::new(*c),
				classname: Untrusted::default(),
				file: None,
				line: None,
				status: Status::Passed,
				time_ms: 1,
				attempts: Vec::new(),
				attempts_omitted: 0,
				message: None,
				detail: None,
				system_out: None,
				system_err: None,
			})
			.collect(),
		time_ms: 10,
	}
}

#[test]
fn merging_keeps_first_seen_order_and_keys_on_name_and_flavour() {
	let mut report = Report {
		suites: vec![
			suite("b", Flavour::Generic, &["1"]),
			suite("a", Flavour::Generic, &["2"]),
			suite("b", Flavour::Nextest, &["3"]),
			suite("b", Flavour::Generic, &["4"]),
			suite("a", Flavour::Generic, &["5"]),
		],
	};
	report.merge_same_named();
	let shape: Vec<(String, Flavour, Vec<String>, u64)> = report
		.suites
		.iter()
		.map(|s| {
			(
				s.name.as_str().to_owned(),
				s.flavour,
				s.cases.iter().map(|c| c.name.as_str().to_owned()).collect(),
				s.time_ms,
			)
		})
		.collect();
	assert_eq!(
		shape,
		vec![
			(
				"b".into(),
				Flavour::Generic,
				vec!["1".into(), "4".into()],
				20
			),
			(
				"a".into(),
				Flavour::Generic,
				vec!["2".into(), "5".into()],
				20
			),
			("b".into(), Flavour::Nextest, vec!["3".into()], 10),
		]
	);
}

#[test]
fn merging_many_distinct_suites_is_fast() {
	let mut report = Report {
		suites: (0..200_000)
			.map(|i| suite(&format!("suite {i}"), Flavour::Generic, &[]))
			.collect(),
	};
	let start = Instant::now();
	report.merge_same_named();
	let took = start.elapsed();
	assert_eq!(report.suites.len(), 200_000);
	// A quadratic merge took 11 s for half this many. The bound is generous on purpose.
	assert!(took < Duration::from_secs(5), "took {took:?}");
}

#[test]
fn merging_many_copies_of_one_suite_is_fast() {
	let mut report = Report {
		suites: (0..200_000)
			.map(|_| suite("same", Flavour::Generic, &["t"]))
			.collect(),
	};
	let start = Instant::now();
	report.merge_same_named();
	assert!(start.elapsed() < Duration::from_secs(5));
	assert_eq!(report.suites.len(), 1);
	assert_eq!(report.suites[0].cases.len(), 200_000);
}

fn suites_doc(n: usize) -> Vec<u8> {
	let mut s = String::from("<testsuites>");
	for i in 0..n {
		s.push_str(&format!(
			"<testsuite name=\"s{i}\"><testcase classname=\"c\" name=\"t\"/></testsuite>"
		));
	}
	s.push_str("</testsuites>");
	s.into_bytes()
}

#[test]
fn the_suite_limit_refuses_with_a_count() {
	let doc = suites_doc(11);
	let at = Limits {
		max_suites: 11,
		..Limits::default()
	};
	assert_eq!(
		parse_junit(&doc, Flavour::Auto, &at).unwrap().suites.len(),
		11
	);
	let under = Limits {
		max_suites: 10,
		..Limits::default()
	};
	let err = parse_junit(&doc, Flavour::Auto, &under).unwrap_err();
	assert_eq!(err, ParseError::TooManySuites { limit: 10 });
	assert_eq!(err.to_string(), "the input has more than 10 suites");
}

#[test]
fn the_default_suite_limit_is_ten_thousand() {
	assert_eq!(Limits::default().max_suites, 10_000);
	let doc = suites_doc(10_001);
	assert_eq!(
		parse_junit(&doc, Flavour::Auto, &Limits::default()).unwrap_err(),
		ParseError::TooManySuites { limit: 10_000 }
	);
}

fn doc_with_suite_file(cases: usize, file: &str) -> Vec<u8> {
	let mut s = format!("<testsuites><testsuite name=\"s\" file=\"{file}\">");
	for i in 0..cases {
		s.push_str(&format!("<testcase classname=\"c\" name=\"t{i}\"/>"));
	}
	s.push_str("</testsuite></testsuites>");
	s.into_bytes()
}

#[test]
fn cases_share_their_suites_file_instead_of_copying_it() {
	let file = "d/".repeat(512);
	let doc = doc_with_suite_file(1000, &file);
	let report = parse_junit(&doc, Flavour::Generic, &Limits::default()).unwrap();
	let cases = &report.suites[0].cases;
	let first = cases[0].file.as_ref().unwrap();
	assert_eq!(first.as_str(), file.trim_end_matches('/'));
	assert!(cases
		.iter()
		.all(|c| c.file.as_ref().unwrap().shares_text_with(first)));
}

#[test]
fn nested_suites_share_the_innermost_file() {
	let doc =
		br#"<testsuites><testsuite name="o" file="outer.rs"><testsuite name="i" file="inner.rs">
		<testcase classname="c" name="a"/><testcase classname="c" name="b"/></testsuite>
		<testcase classname="c" name="c"/></testsuite></testsuites>"#;
	let report = parse_junit(doc, Flavour::Generic, &Limits::default()).unwrap();
	let cases = &report.suites[0].cases;
	let files: Vec<&str> = cases
		.iter()
		.map(|c| c.file.as_ref().unwrap().as_str())
		.collect();
	assert_eq!(files, ["inner.rs", "inner.rs", "outer.rs"]);
	assert!(cases[0]
		.file
		.as_ref()
		.unwrap()
		.shares_text_with(cases[1].file.as_ref().unwrap()));
}

#[test]
fn a_suite_path_taken_for_playwright_and_vitest_cases_is_stored_once() {
	let mut s = String::from("<testsuites><testsuite name=\"tests/a.spec.ts\">");
	for i in 0..100 {
		s.push_str(&format!("<testcase classname=\"\" name=\"t{i}\"/>"));
	}
	s.push_str("</testsuite></testsuites>");
	for flavour in [Flavour::Vitest, Flavour::Playwright] {
		let report = parse_junit(s.as_bytes(), flavour, &Limits::default()).unwrap();
		let cases = &report.suites[0].cases;
		let first = cases[0].file.as_ref().unwrap();
		assert_eq!(first.as_str(), "tests/a.spec.ts");
		assert!(cases
			.iter()
			.all(|c| c.file.as_ref().unwrap().shares_text_with(first)));
	}
}

#[test]
fn the_text_budget_refuses_with_a_count() {
	let name = "n".repeat(100);
	let doc = format!(
		"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"{name}\"/></testsuite></testsuites>"
	);
	// The suite name (1), the class name (1) and the case name (100), 40 bytes of overhead each,
	// and 64 for the case.
	let exact = Limits {
		max_text_bytes: 286,
		..Limits::default()
	};
	assert!(parse_junit(doc.as_bytes(), Flavour::Generic, &exact).is_ok());
	let under = Limits {
		max_text_bytes: 285,
		..Limits::default()
	};
	let err = parse_junit(doc.as_bytes(), Flavour::Generic, &under).unwrap_err();
	assert_eq!(err, ParseError::TooMuchText { limit: 285 });
	assert_eq!(
		err.to_string(),
		"the input holds more than 285 bytes of text to keep"
	);
}

#[test]
fn repeated_bodies_that_are_not_kept_still_count() {
	let body = "b".repeat(1000);
	let mut doc =
		String::from("<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"t\">");
	for _ in 0..50 {
		doc.push_str(&format!("<failure message=\"m\">{body}</failure>"));
	}
	doc.push_str("</testcase></testsuite></testsuites>");
	let tight = Limits {
		max_text_bytes: 10_000,
		..Limits::default()
	};
	assert_eq!(
		parse_junit(doc.as_bytes(), Flavour::Generic, &tight).unwrap_err(),
		ParseError::TooMuchText { limit: 10_000 }
	);
}

#[test]
fn invalid_utf8_counts_at_its_replaced_size() {
	// 300 bad bytes become 300 three-byte replacement characters: 900 bytes of name, plus 40 for
	// it, 82 for the suite and class names and 64 for the case.
	let mut doc = b"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"".to_vec();
	doc.extend(std::iter::repeat_n(0xff, 300));
	doc.extend_from_slice(b"\"/></testsuite></testsuites>");
	let limits = |n| Limits {
		max_text_bytes: n,
		..Limits::default()
	};
	let report = parse_junit(&doc, Flavour::Generic, &limits(1086)).unwrap();
	assert_eq!(report.suites[0].cases[0].name.as_str().len(), 900);
	assert_eq!(
		parse_junit(&doc, Flavour::Generic, &limits(1085)).unwrap_err(),
		ParseError::TooMuchText { limit: 1085 }
	);
	// The same count of valid bytes is a third of the cost.
	let valid = String::from_utf8(
		doc.iter()
			.map(|&b| if b == 0xff { b'x' } else { b })
			.collect(),
	)
	.unwrap();
	assert!(parse_junit(valid.as_bytes(), Flavour::Generic, &limits(486)).is_ok());
}

#[test]
fn input_that_grows_past_the_size_limit_when_repaired_is_refused() {
	let mut doc = b"<testsuites><testsuite name=\"".to_vec();
	doc.extend(std::iter::repeat_n(0xff, 100));
	doc.extend_from_slice(b"\"/></testsuites>");
	let given = doc.len();
	let limits = Limits {
		max_bytes: given + 10,
		..Limits::default()
	};
	let err = parse_junit(&doc, Flavour::Generic, &limits).unwrap_err();
	assert_eq!(
		err,
		ParseError::TooLarge {
			len: given + 200,
			limit: given + 10
		}
	);
}

fn retries(kind: &str, n: usize) -> Vec<u8> {
	let mut s =
		String::from("<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"t\">");
	for i in 0..n {
		s.push_str(&format!("<{kind} message=\"try {i}\">trace</{kind}>"));
	}
	s.push_str("</testcase></testsuite></testsuites>");
	s.into_bytes()
}

#[test]
fn attempts_past_the_limit_are_counted_not_dropped_silently() {
	let limits = Limits {
		max_attempts: 2,
		..Limits::default()
	};
	let report = parse_junit(&retries("rerunFailure", 5), Flavour::Nextest, &limits).unwrap();
	let case = &report.suites[0].cases[0];
	assert_eq!(case.attempts.len(), 2);
	assert_eq!(case.attempts_omitted, 3);
	assert_eq!(case.attempts[1].message.as_ref().unwrap().as_str(), "try 1");

	// A flaky pass is still flaky when its retries were cut.
	let report = parse_junit(&retries("flakyFailure", 5), Flavour::Nextest, &limits).unwrap();
	let case = &report.suites[0].cases[0];
	assert_eq!(
		(case.status, case.attempts.len(), case.attempts_omitted),
		(Status::Flaky, 2, 3)
	);

	// Nothing is omitted at or under the limit.
	let report = parse_junit(&retries("rerunError", 2), Flavour::Nextest, &limits).unwrap();
	assert_eq!(report.suites[0].cases[0].attempts_omitted, 0);
	assert_eq!(report.suites[0].cases[0].attempts.len(), 2);
}

#[test]
fn every_kept_attempt_is_charged() {
	// Ten attempts at 154 each (64, and 45 for each of the message and the body), on top of the
	// suite (41), class (41), name (41) and case (64).
	let doc = retries("flakyFailure", 10);
	let limits = |n| Limits {
		max_text_bytes: n,
		..Limits::default()
	};
	assert!(parse_junit(&doc, Flavour::Nextest, &limits(1727)).is_ok());
	assert_eq!(
		parse_junit(&doc, Flavour::Nextest, &limits(1726)).unwrap_err(),
		ParseError::TooMuchText { limit: 1726 }
	);
}

#[test]
fn debug_output_shows_a_size_and_never_the_text() {
	let secret = Untrusted::new("@octocat token=hunter2 <script>");
	let shown = format!("{secret:?}");
	assert_eq!(shown, "Untrusted(31 bytes)");
	let cut = Untrusted::capped("0123456789", 4);
	assert_eq!(format!("{cut:?}"), "Untrusted(4 bytes, +6 omitted)");
	// Anything that holds one prints the same way.
	let suite = suite("hunter2", Flavour::Generic, &["@octocat"]);
	let shown = format!("{suite:?}");
	assert!(
		!shown.contains("hunter2") && !shown.contains("octocat"),
		"{shown}"
	);
}

#[test]
fn merging_does_not_join_names_that_were_cut_differently() {
	let mut cut_short = suite("name", Flavour::Generic, &["1"]);
	cut_short.name = Untrusted::capped("name and then some", 4);
	let mut cut_more = suite("name", Flavour::Generic, &["2"]);
	cut_more.name = Untrusted::capped("name and a very long tail", 4);
	let mut report = Report {
		suites: vec![
			cut_short.clone(),
			cut_more,
			suite("name", Flavour::Generic, &["3"]),
			cut_short,
		],
	};
	report.merge_same_named();
	let cases: Vec<usize> = report.suites.iter().map(|s| s.cases.len()).collect();
	assert_eq!(cases, [2, 1, 1]);
}
