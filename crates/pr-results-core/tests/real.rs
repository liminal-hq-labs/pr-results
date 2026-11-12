// The readers against real reports: output the JavaScript reporters wrote for the sample
// projects under `tests/sample-projects`, parsed and compared with a model written out in text.
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
use std::path::PathBuf;

use common::{all_fixtures, fixture};
use pr_results_core::{
	parse_junit, parse_junit_in, Case, Flavour, Limits, RepoPath, Report, Status, Suite,
};

fn models_dir() -> PathBuf {
	common::fixtures_dir().join("../models")
}

fn blessing() -> bool {
	assert!(
		!(std::env::var_os("CI").is_some() && std::env::var("PR_RESULTS_BLESS").is_ok()),
		"PR_RESULTS_BLESS must not be set when CI is set"
	);
	std::env::var("PR_RESULTS_BLESS").as_deref() == Ok("1")
}

fn status(s: Status) -> &'static str {
	match s {
		Status::Passed => "passed",
		Status::Failed => "failed",
		Status::Errored => "errored",
		Status::Skipped => "skipped",
		Status::Flaky => "flaky",
	}
}

/// One case per line: what the parser made of it, in a form that reads as a table.
fn case_line(c: &Case) -> String {
	let mut out = String::new();
	let place = match (&c.file, c.line) {
		(Some(f), Some(l)) => format!("{}:{l}", f.as_str()),
		(Some(f), None) => f.as_str().to_owned(),
		(None, Some(l)) => format!("?:{l}"),
		(None, None) => "-".to_owned(),
	};
	let _ = write!(
		out,
		"  {:<7} {:>5}ms class={:?} name={:?} at {place}",
		status(c.status),
		c.time_ms,
		c.classname.as_str(),
		c.name.as_str(),
	);
	if !c.attempts.is_empty() || c.attempts_omitted > 0 {
		let _ = write!(out, " attempts={}", c.attempts.len());
	}
	if let Some(m) = &c.message {
		let first = m.as_str().lines().next().unwrap_or_default();
		let _ = write!(out, " message={first:?}");
	}
	out
}

fn suite_lines(s: &Suite) -> String {
	let project = s.project.as_ref().map_or("-", |p| p.as_str());
	let mut out = format!(
		"suite {:?} project={project} flavour={} {}ms\n",
		s.name.as_str(),
		s.flavour.name(),
		s.time_ms
	);
	for c in &s.cases {
		out.push_str(&case_line(c));
		out.push('\n');
	}
	out
}

fn model(report: &Report) -> String {
	report.suites.iter().map(suite_lines).collect()
}

fn real_fixtures() -> Vec<String> {
	all_fixtures()
		.into_iter()
		.map(|(name, _)| name)
		.filter(|n| n.rsplit('/').next().is_some_and(|f| f.starts_with("real-")))
		.collect()
}

/// Compares `got` with the model file for `name`, or rewrites it when blessing.
fn check_model(name: &str, got: &str) {
	let path = models_dir().join(name.replace(".xml", ".txt"));
	if blessing() {
		std::fs::create_dir_all(path.parent().unwrap()).unwrap();
		std::fs::write(&path, got).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&path).unwrap_or_else(|_| {
		panic!(
			"{name}: no model at {}: run with PR_RESULTS_BLESS=1",
			path.display()
		)
	});
	assert!(
		want == got,
		"{name} differs from its model: run with PR_RESULTS_BLESS=1 and review the diff\n--- model\n{want}\n--- parsed\n{got}"
	);
}

#[test]
fn every_real_report_reads_as_its_model() {
	let names = real_fixtures();
	assert!(
		names.len() >= 12,
		"the real fixtures are missing: {names:?}"
	);
	for name in names {
		let report = parse_junit(&fixture(&name), Flavour::Auto, &Limits::default())
			.unwrap_or_else(|e| panic!("{name}: {e}"));
		check_model(&name, &model(&report));
	}
}

#[test]
fn every_real_report_is_detected_as_its_producer() {
	for name in real_fixtures() {
		let want = match name.split('/').next().unwrap() {
			"vitest" => Flavour::Vitest,
			"bun" => Flavour::Bun,
			"playwright" => Flavour::Playwright,
			"jest" if name.ends_with("templates.xml") => Flavour::Generic,
			"jest" => Flavour::Jest,
			other => panic!("{other}"),
		};
		let report = parse_junit(&fixture(&name), Flavour::Auto, &Limits::default()).unwrap();
		assert!(!report.suites.is_empty(), "{name}");
		for s in &report.suites {
			assert_eq!(s.flavour, want, "{name}");
		}
	}
}

fn first_failure<'a>(report: &'a Report, name: &str) -> &'a Case {
	report
		.suites
		.iter()
		.flat_map(|s| &s.cases)
		.find(|c| c.name.as_str().ends_with(name))
		.unwrap_or_else(|| panic!("no case ending {name:?}"))
}

fn at(case: &Case) -> (String, Option<u32>) {
	(case.file.as_ref().unwrap().as_str().to_owned(), case.line)
}

fn read_with(name: &str, flavour: Flavour, prefix: &str) -> Report {
	let prefix = RepoPath::parse(prefix).unwrap();
	parse_junit_in(&fixture(name), flavour, &Limits::default(), Some(&prefix)).unwrap()
}

#[test]
fn a_prefix_goes_in_front_of_vitests_root_relative_paths() {
	let r = read_with("vitest/real-default.xml", Flavour::Vitest, "apps/web/");
	assert_eq!(
		at(first_failure(&r, "fails with a stack")),
		("apps/web/test/math.test.js".to_owned(), Some(29))
	);
	assert_eq!(
		at(first_failure(&r, "throws from the source")),
		("apps/web/src/math.js".to_owned(), Some(12)),
		"the first frame is the source file that threw"
	);
	assert_eq!(
		at(first_failure(&r, "math > adds")).0,
		"apps/web/test/math.test.js"
	);
}

#[test]
fn a_prefix_is_applied_to_bun_and_to_jest_with_a_file_attribute() {
	let r = read_with("bun/real-default.xml", Flavour::Bun, "packages/server");
	assert_eq!(
		at(first_failure(&r, "fails with a stack")),
		("packages/server/test/math.test.js".to_owned(), Some(29))
	);
	let r = read_with("jest/real-file.xml", Flavour::Jest, "packages/ui/");
	assert_eq!(
		at(first_failure(&r, "fails with a stack")),
		("packages/ui/test/math.test.js".to_owned(), Some(28)),
		"an absolute frame naming the case's own file keeps the line and takes the prefixed path"
	);
}

#[test]
fn playwrights_paths_are_relative_to_its_test_directory_and_may_climb_out_of_it() {
	let r = read_with("playwright/real-default.xml", Flavour::Playwright, "e2e/");
	assert_eq!(
		at(first_failure(&r, "fails with a stack")),
		("e2e/math.spec.js".to_owned(), Some(26))
	);
	assert_eq!(
		at(first_failure(&r, "throws from the source")),
		("src/math.js".to_owned(), Some(12)),
		"`../src/math.js` climbs out of `e2e/` to the repository's src"
	);
	// Without a prefix the same frame cannot be placed, and the next one is used.
	let r = parse_junit(
		&fixture("playwright/real-default.xml"),
		Flavour::Playwright,
		&Limits::default(),
	)
	.unwrap();
	assert_eq!(
		at(first_failure(&r, "throws from the source")),
		("sub/retry.spec.js".to_owned(), Some(19))
	);
	// A prefix with nothing to climb out of keeps the frame out of the repository.
	let r = read_with("playwright/real-default.xml", Flavour::Playwright, "tests");
	assert_eq!(
		at(first_failure(&r, "throws from the source")),
		("src/math.js".to_owned(), Some(12))
	);
}

#[test]
fn playwright_projects_stay_apart_and_shards_of_one_project_merge() {
	let mut both = parse_junit(
		&fixture("playwright/real-shard-1-of-2.xml"),
		Flavour::Auto,
		&Limits::default(),
	)
	.unwrap();
	both.suites.extend(
		parse_junit(
			&fixture("playwright/real-shard-2-of-2.xml"),
			Flavour::Auto,
			&Limits::default(),
		)
		.unwrap()
		.suites,
	);
	both.merge_same_named();
	let keys: Vec<(&str, &str)> = both
		.suites
		.iter()
		.map(|s| (s.name.as_str(), s.project.as_ref().unwrap().as_str()))
		.collect();
	assert_eq!(
		keys,
		[
			("math.spec.js", "chromium"),
			("sub/retry.spec.js", "chromium"),
			("math.spec.js", "firefox"),
			("sub/retry.spec.js", "firefox"),
		],
		"the same spec file in two projects is two suites"
	);
	// The same project from two shards is one suite again.
	let mut twice = parse_junit(
		&fixture("playwright/real-shard-1-of-2.xml"),
		Flavour::Auto,
		&Limits::default(),
	)
	.unwrap();
	let again = twice.clone();
	twice.suites.extend(again.suites);
	twice.merge_same_named();
	assert_eq!(twice.suites.len(), 2);
	assert_eq!(twice.suites[0].cases.len(), 14);
}

#[test]
fn only_playwrights_hostname_is_a_project() {
	for name in [
		"vitest/real-default.xml",
		"bun/real-default.xml",
		"jest/real-default.xml",
	] {
		let r = parse_junit(&fixture(name), Flavour::Auto, &Limits::default()).unwrap();
		assert!(r.suites.iter().all(|s| s.project.is_none()), "{name}");
	}
}

#[test]
fn what_each_reporter_records_about_retries() {
	let attempts = |name: &str, case: &str| {
		let r = parse_junit(&fixture(name), Flavour::Auto, &Limits::default()).unwrap();
		let c = first_failure(&r, case);
		(c.status, c.attempts.len())
	};
	// vitest writes one `failure` element for every attempt of a test that kept failing.
	assert_eq!(
		attempts("vitest/real-default.xml", "never passes"),
		(Status::Failed, 2)
	);
	// A test that failed and then passed leaves no trace in any of the four.
	for (name, case) in [
		(
			"vitest/real-default.xml",
			"retries > passes on the second try",
		),
		("bun/real-retry.xml", "passes on the second try"),
		("jest/real-default.xml", "retries passes on the second try"),
		("playwright/real-default.xml", "passes on the second try"),
	] {
		assert_eq!(attempts(name, case), (Status::Passed, 0), "{name}");
	}
	// Neither bun nor Playwright nor jest-junit writes the retries of a test that kept failing.
	// Playwright reports a thrown error as an `error`, the others as a `failure`.
	for (name, case, want) in [
		("bun/real-retry.xml", "never passes", Status::Failed),
		(
			"jest/real-default.xml",
			"retries never passes",
			Status::Failed,
		),
		(
			"playwright/real-default.xml",
			"never passes",
			Status::Errored,
		),
	] {
		assert_eq!(attempts(name, case), (want, 0), "{name}");
	}
}

#[test]
fn bun_todo_and_skip_and_vitest_todo_are_skipped_but_jest_todo_passes() {
	let bun = parse_junit(
		&fixture("bun/real-default.xml"),
		Flavour::Auto,
		&Limits::default(),
	)
	.unwrap();
	assert_eq!(
		first_failure(&bun, "is still to do").status,
		Status::Skipped
	);
	assert_eq!(
		first_failure(&bun, "is still to do")
			.message
			.as_ref()
			.unwrap(),
		&"TODO"
	);
	let vitest = parse_junit(
		&fixture("vitest/real-default.xml"),
		Flavour::Auto,
		&Limits::default(),
	)
	.unwrap();
	assert_eq!(
		first_failure(&vitest, "is still to do").status,
		Status::Skipped
	);
	let jest = parse_junit(
		&fixture("jest/real-default.xml"),
		Flavour::Auto,
		&Limits::default(),
	)
	.unwrap();
	assert_eq!(first_failure(&jest, "is skipped").status, Status::Skipped);
	assert_eq!(
		first_failure(&jest, "is still to do").status,
		Status::Passed,
		"jest-junit writes a todo test as a plain case"
	);
}

#[test]
fn a_failure_with_no_message_takes_its_first_line_of_text() {
	let bun = parse_junit(
		&fixture("bun/real-default.xml"),
		Flavour::Auto,
		&Limits::default(),
	)
	.unwrap();
	let case = first_failure(&bun, "fails with an empty message");
	assert_eq!(case.status, Status::Failed);
	assert_eq!(case.message.as_ref().unwrap(), &"Error");
}

#[test]
fn bun_rerun_each_repeats_cases_and_they_stay_separate() {
	let r = parse_junit(
		&fixture("bun/real-rerun-each.xml"),
		Flavour::Auto,
		&Limits::default(),
	)
	.unwrap();
	let adds = r
		.suites
		.iter()
		.flat_map(|s| &s.cases)
		.filter(|c| c.name.as_str() == "adds")
		.count();
	assert_eq!(adds, 2);
}

#[test]
fn bun_describe_blocks_read_outermost_first() {
	let r = parse_junit(
		&fixture("bun/real-default.xml"),
		Flavour::Auto,
		&Limits::default(),
	)
	.unwrap();
	assert_eq!(
		first_failure(&r, "rounds down").classname,
		"math > division > rounding"
	);
	assert_eq!(first_failure(&r, "adds").classname, "math");
}
