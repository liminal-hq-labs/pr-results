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
