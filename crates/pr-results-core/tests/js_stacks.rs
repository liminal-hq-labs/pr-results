// JavaScript stack frames as locations: which frame is used, and which are never.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use pr_results_core::{parse_junit_in, Flavour, Limits, RepoPath};

/// The place a vitest-shaped failure with `body` is given, as `path:line` or `-:line` (a line in
/// the case's own file is `own:line`, since the file is not in the report).
fn located(flavour: Flavour, classname: &str, body: &str, prefix: Option<&str>) -> String {
	let escaped = body
		.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;");
	let xml = format!(
		"<testsuites name=\"vitest tests\"><testsuite name=\"{classname}\"><testcase classname=\"{classname}\" name=\"t\"><failure message=\"m\">{escaped}</failure></testcase></testsuite></testsuites>"
	);
	let prefix = prefix.map(|p| RepoPath::parse(p).unwrap());
	let r = parse_junit_in(xml.as_bytes(), flavour, &Limits::default(), prefix.as_ref()).unwrap();
	let c = &r.suites[0].cases[0];
	format!(
		"{}:{}",
		c.file.as_ref().map_or("-", |f| f.as_str()),
		c.line.map_or("-".to_owned(), |l| l.to_string())
	)
}

fn vitest(body: &str) -> String {
	located(Flavour::Vitest, "test/a.test.js", body, None)
}

#[test]
fn the_first_frame_in_the_repository_is_the_location() {
	let table = [
		("❯ src/a.js:4:5\n ❯ test/a.test.js:9:1", "src/a.js:4"),
		("❯ divide src/a.js:4:5", "src/a.js:4"),
		("at fn (src/a.js:4:5)", "src/a.js:4"),
		("at src/a.js:4:5", "src/a.js:4"),
		("at async fn (src/a.js:4:5)", "src/a.js:4"),
		("at new Thing (src/a.js:4:5)", "src/a.js:4"),
		("at Object.<anonymous> (src/a.js:4:5)", "src/a.js:4"),
		("at src/a.js:4", "src/a.js:4"),
		("  at   src/a.js:4:5  ", "src/a.js:4"),
		("at .\\src\\a.js:4:5", "src/a.js:4"),
		("at ./src/a.js:4:5", "src/a.js:4"),
	];
	for (body, want) in table {
		assert_eq!(vitest(body), want, "{body:?}");
	}
}

#[test]
fn node_modules_and_node_internals_are_skipped() {
	let table = [
		(
			"❯ node_modules/vitest/dist/run.js:1:2\n ❯ src/a.js:4:5",
			"src/a.js:4",
		),
		(
			"at x (node_modules/.pnpm/pkg@1.0.0/node_modules/pkg/index.js:3:4)\nat src/a.js:7:8",
			"src/a.js:7",
		),
		("at src/node_modules/b/c.js:1:1", "test/a.test.js:-"),
		(
			"at node:internal/process/task_queues:105:5",
			"test/a.test.js:-",
		),
		("at new Promise (<anonymous>)", "test/a.test.js:-"),
		("at async Promise.all (index 0)", "test/a.test.js:-"),
		("at Array.map (<anonymous>)\nat src/a.js:2:3", "src/a.js:2"),
	];
	for (body, want) in table {
		assert_eq!(vitest(body), want, "{body:?}");
	}
}

#[test]
fn absolute_paths_and_urls_are_dropped_unless_they_name_the_cases_own_file() {
	let table = [
		// Dropped: not the case's file, so the checkout is unknown.
		("at /home/runner/work/r/r/src/a.js:4:5", "test/a.test.js:-"),
		(
			"at fn (/home/runner/work/r/r/src/a.js:4:5)",
			"test/a.test.js:-",
		),
		(
			"at file:///home/runner/work/r/r/src/a.js:4:5",
			"test/a.test.js:-",
		),
		("at C:\\Users\\runner\\src\\a.js:4:5", "test/a.test.js:-"),
		(
			"at file:///C:/Users/runner/src/a.js:4:5",
			"test/a.test.js:-",
		),
		("at file://host/share/src/a.js:4:5", "test/a.test.js:-"),
		("at \\\\server\\share\\a.js:4:5", "test/a.test.js:-"),
		// Kept: the case's own file, seen through the runner's checkout.
		(
			"at fn (/home/runner/work/r/r/test/a.test.js:7:3)",
			"test/a.test.js:7",
		),
		(
			"at file:///home/runner/work/r/r/test/a.test.js:8:3",
			"test/a.test.js:8",
		),
		// Skipped for a later frame.
		(
			"at /home/runner/work/r/r/src/a.js:4:5\nat src/b.js:6:7",
			"src/b.js:6",
		),
		// A longer name that merely ends the same way is not the file.
		(
			"at /home/runner/work/r/r/other-test/a.test.js:7:3",
			"test/a.test.js:-",
		),
	];
	for (body, want) in table {
		assert_eq!(vitest(body), want, "{body:?}");
	}
}

#[test]
fn paths_that_leave_the_repository_are_never_locations() {
	let table = [
		"at ../outside.js:1:1",
		"at ../../etc/passwd:1:1",
		"at src/../../outside.js:1:1",
		"at src/a\u{1}b.js:1:1",
		"at src/a.js:0:5",
		"at src/a.js:99999999999:5",
		"at src/a.js:-3:5",
		"at src/a.js:x:5",
		"at src/a.js",
		"at :4:5",
		"at (:4:5)",
		"at",
		"at ",
		"❯",
		"❯ 4:5",
		"at https://example.invalid/a.js:4:5",
		"at webpack-internal:///./src/a.js:4:5",
	];
	for body in table {
		assert_eq!(vitest(body), "test/a.test.js:-", "{body:?}");
	}
	let long = format!("at {}/a.js:4:5", "d".repeat(2000));
	assert_eq!(
		vitest(&long),
		"test/a.test.js:-",
		"a path over the length cap"
	);
}

#[test]
fn text_that_only_looks_like_a_frame_is_not_one() {
	let table = [
		"expected at src/a.js:4:5 to be defined",
		"look at src/a.js:4:5",
		"- at src/a.js:4:5",
		"Error: at src/a.js:4:5",
		"     3| at src/a.js:4:5",
	];
	for body in table {
		assert_eq!(vitest(body), "test/a.test.js:-", "{body:?}");
	}
}

#[test]
fn the_prefix_is_put_in_front_and_checked_again() {
	let at =
		|body: &str, prefix: &str| located(Flavour::Vitest, "test/a.test.js", body, Some(prefix));
	assert_eq!(at("at src/a.js:4:5", "apps/web/"), "apps/web/src/a.js:4");
	assert_eq!(at("at src/a.js:4:5", "apps\\web"), "apps/web/src/a.js:4");
	assert_eq!(at("at src/a.js:4:5", "./apps/web"), "apps/web/src/a.js:4");
	assert_eq!(
		at("at nothing", "apps/web"),
		"apps/web/test/a.test.js:-",
		"the case's own file takes the prefix too"
	);
	// A leading `..` climbs out of the prefix, never out of the repository.
	assert_eq!(
		at("at ../shared/a.js:4:5", "apps/web"),
		"apps/shared/a.js:4"
	);
	assert_eq!(at("at ../../shared/a.js:4:5", "apps/web"), "shared/a.js:4");
	assert_eq!(
		at("at ../../../shared/a.js:4:5", "apps/web"),
		"apps/web/test/a.test.js:-"
	);
	assert_eq!(
		at("at src/../a.js:4:5", "apps/web"),
		"apps/web/test/a.test.js:-",
		"a `..` after a name is refused"
	);
	assert_eq!(
		at("at node_modules/x/a.js:4:5", "apps/web"),
		"apps/web/test/a.test.js:-"
	);
	assert_eq!(
		at("at /abs/test/a.test.js:4:5", "apps/web"),
		"apps/web/test/a.test.js:4",
		"the own file is matched without the prefix, then shown with it"
	);
}

#[test]
fn playwright_falls_back_to_the_place_the_test_is_declared() {
	let body = "[chromium] › login.spec.ts:12:5 › login › signs in ──────\n\n    Test timeout of 30000ms exceeded.";
	let got = located(Flavour::Playwright, "login.spec.ts", body, Some("e2e/"));
	assert_eq!(got, "e2e/login.spec.ts:12");
	// A frame is preferred to the header.
	let body =
		"[chromium] › login.spec.ts:12:5 › login › signs in\n\n    at /w/e2e/login.spec.ts:20:3";
	assert_eq!(
		located(Flavour::Playwright, "login.spec.ts", body, Some("e2e/")),
		"e2e/login.spec.ts:20"
	);
	// An absolute or climbing header is never used.
	let body = "[chromium] › /etc/passwd:12:5 › login";
	assert_eq!(
		located(Flavour::Playwright, "login.spec.ts", body, None),
		"login.spec.ts:-"
	);
	let body = "[chromium] › ../x.ts:12:5 › login";
	assert_eq!(
		located(Flavour::Playwright, "login.spec.ts", body, None),
		"login.spec.ts:-"
	);
}

#[test]
fn only_the_javascript_reporters_read_stacks() {
	let xml = "<testsuites name=\"x\"><testsuite name=\"s\"><testcase classname=\"c\" name=\"t\"><failure message=\"m\">at src/a.js:4:5</failure></testcase></testsuite></testsuites>";
	let r = parse_junit_in(xml.as_bytes(), Flavour::Generic, &Limits::default(), None).unwrap();
	assert!(r.suites[0].cases[0].file.is_none());
	let r = parse_junit_in(xml.as_bytes(), Flavour::Jest, &Limits::default(), None).unwrap();
	assert_eq!(
		r.suites[0].cases[0].file.as_ref().unwrap().as_str(),
		"src/a.js"
	);
}

#[test]
fn a_prefix_applies_to_a_nextest_panic_and_to_an_attribute_too() {
	let xml = "<testsuites name=\"nextest-run\"><testsuite name=\"s\"><testcase classname=\"c\" name=\"t\"><failure message=\"m\">thread 't' panicked at src/lib.rs:12:5:\nx</failure></testcase><testcase classname=\"c\" name=\"u\" file=\"tests/u.rs\" line=\"3\"/></testsuite></testsuites>";
	let prefix = RepoPath::parse("crates/core").unwrap();
	let r = parse_junit_in(
		xml.as_bytes(),
		Flavour::Auto,
		&Limits::default(),
		Some(&prefix),
	)
	.unwrap();
	let c = &r.suites[0].cases;
	assert_eq!(
		c[0].file.as_ref().unwrap().as_str(),
		"crates/core/src/lib.rs"
	);
	assert_eq!(c[0].line, Some(12));
	assert_eq!(
		c[1].file.as_ref().unwrap().as_str(),
		"crates/core/tests/u.rs"
	);
}
