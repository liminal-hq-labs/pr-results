// Tests for the comparison of two runs' tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use pr_results_core::{parse_junit, test_delta, Flavour, Limits, Suite, TestDelta, MAX_LISTED};

fn suites(xml: &str) -> Vec<Suite> {
	let text = format!("<?xml version=\"1.0\"?>\n{xml}");
	parse_junit(text.as_bytes(), Flavour::Generic, &Limits::default())
		.unwrap()
		.suites
}

fn run(suite: &str, cases: &[(&str, &str)]) -> Vec<Suite> {
	let body: String = cases
		.iter()
		.map(|(class, name)| format!("<testcase classname=\"{class}\" name=\"{name}\"/>"))
		.collect();
	suites(&format!(
		"<testsuites><testsuite name=\"{suite}\">{body}</testsuite></testsuites>"
	))
}

fn names(ids: &[pr_results_core::TestId]) -> Vec<&str> {
	ids.iter().map(|i| i.name.as_str()).collect()
}

#[test]
fn the_same_tests_are_no_difference() {
	let a = run("app", &[("c", "one"), ("c", "two")]);
	let delta = test_delta(&a, &a);
	assert!(delta.is_empty());
	assert_eq!(delta, TestDelta::default());
}

#[test]
fn added_and_removed_tests_are_found_in_name_order() {
	let base = run("app", &[("c", "keep"), ("c", "gone_b"), ("c", "gone_a")]);
	let now = run("app", &[("c", "keep"), ("c", "new_b"), ("c", "new_a")]);
	let delta = test_delta(&now, &base);
	assert_eq!((delta.added_total, delta.removed_total), (2, 2));
	assert_eq!(names(&delta.added), ["new_a", "new_b"]);
	assert_eq!(names(&delta.removed), ["gone_a", "gone_b"]);
}

#[test]
fn a_renamed_test_is_one_removed_and_one_added() {
	let delta = test_delta(
		&run("app", &[("c", "after")]),
		&run("app", &[("c", "before")]),
	);
	assert_eq!((delta.added_total, delta.removed_total), (1, 1));
}

#[test]
fn whitespace_is_not_a_difference_but_the_class_and_the_suite_are() {
	let base = run("app", &[("c", "a  test")]);
	assert!(test_delta(&run("app", &[("c", " a test ")]), &base).is_empty());
	assert_eq!(
		test_delta(&run("app", &[("d", "a test")]), &base).added_total,
		1
	);
	assert_eq!(
		test_delta(&run("lib", &[("c", "a test")]), &base).added_total,
		1
	);
}

#[test]
fn a_test_in_several_sections_is_one_test() {
	// The same tests run under three feature sets: one is added, not three.
	let one = run("app", &[("c", "t")]);
	let mut base = one.clone();
	base.extend(one.clone());
	base.extend(one.clone());
	let mut now = base.clone();
	now.extend(run("app", &[("c", "new")]));
	now.extend(run("app", &[("c", "new")]));
	let delta = test_delta(&now, &base);
	assert_eq!((delta.added_total, delta.removed_total), (1, 0));
	assert_eq!(names(&delta.added), ["new"]);
	let back = test_delta(&base, &now);
	assert_eq!((back.added_total, back.removed_total), (0, 1));
}

#[test]
fn a_test_listed_twice_in_a_run_is_one_test() {
	let one = run("app", &[("c", "t")]);
	let two = run("app", &[("c", "t"), ("c", "t")]);
	assert!(test_delta(&two, &one).is_empty());
	assert!(test_delta(&one, &two).is_empty());
}

#[test]
fn a_skipped_test_is_still_the_same_test() {
	let skipped = suites(
		"<testsuites><testsuite name=\"app\"><testcase classname=\"c\" name=\"t\"><skipped/></testcase></testsuite></testsuites>",
	);
	assert!(test_delta(&skipped, &run("app", &[("c", "t")])).is_empty());
}

#[test]
fn nothing_to_compare_with_means_every_test_is_new_and_the_caller_decides_what_to_say() {
	let now = run("app", &[("c", "a"), ("c", "b")]);
	let delta = test_delta(&now, &[]);
	assert_eq!((delta.added_total, delta.removed_total), (2, 0));
}

#[test]
fn the_totals_are_exact_while_the_names_are_capped() {
	let many: Vec<(String, String)> = (0..MAX_LISTED + 25)
		.map(|i| ("c".to_owned(), format!("t{i:04}")))
		.collect();
	let pairs: Vec<(&str, &str)> = many.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
	let delta = test_delta(&run("app", &pairs), &[]);
	assert_eq!(delta.added_total, MAX_LISTED + 25);
	assert_eq!(delta.added.len(), MAX_LISTED);
	assert_eq!(delta.added[0].name, "t0000");
}

#[test]
fn a_project_keeps_two_suites_of_one_name_apart() {
	let playwright = |project: &str| {
		let text = format!(
			"<?xml version=\"1.0\"?>\n<testsuites><testsuite name=\"spec\" hostname=\"{project}\"><testcase classname=\"c\" name=\"t\"/></testsuite></testsuites>"
		);
		parse_junit(text.as_bytes(), Flavour::Playwright, &Limits::default())
			.unwrap()
			.suites
	};
	let delta = test_delta(&playwright("chromium"), &playwright("firefox"));
	assert_eq!((delta.added_total, delta.removed_total), (1, 1));
	assert_eq!(delta.added[0].project, "chromium");
	assert!(test_delta(&playwright("chromium"), &playwright("chromium")).is_empty());
}
