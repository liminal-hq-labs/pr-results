// A vitest-shaped file at the size of a large application's suite: 5 200 cases, some failing
// with stacks and retries. The full size is a measurement, run by hand.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use std::fmt::Write as _;
use std::time::Instant;

use pr_results_core::{parse_junit_in, Flavour, Limits, RepoPath, Status};

fn peak_rss_kib() -> Option<u64> {
	let status = std::fs::read_to_string("/proc/self/status").ok()?;
	let line = status.lines().find_map(|l| l.strip_prefix("VmHWM:"))?;
	line.split_whitespace().next()?.parse().ok()
}

/// `files` test files of `per_file` cases each, as vitest writes them with `addFileAttribute`:
/// every twentieth case fails three times (a test with `retry: 2`) with a stack that starts in a
/// source file, one in thirty is skipped.
fn vitest_report(files: usize, per_file: usize) -> String {
	let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n");
	let _ = writeln!(xml, "<testsuites name=\"vitest tests\">");
	for f in 0..files {
		let file = format!("src/area{}/module{f}.test.ts", f % 12);
		let _ = writeln!(
			xml,
			"    <testsuite name=\"{file}\" timestamp=\"2026-01-01T00:00:00.000Z\" hostname=\"runner\" tests=\"{per_file}\" failures=\"0\" errors=\"0\" skipped=\"0\" time=\"0.123456789\">"
		);
		for c in 0..per_file {
			let name = format!("module {f} &gt; group {} &gt; case {c}", c % 7);
			let _ = writeln!(
				xml,
				"        <testcase classname=\"{file}\" file=\"{file}\" name=\"{name}\" time=\"0.000123456\">"
			);
			if c % 20 == 0 {
				for _ in 0..3 {
					let _ = writeln!(
						xml,
						"            <failure message=\"expected {c} to be {}\" type=\"AssertionError\">\nAssertionError: expected {c} to be {}\n\n - Expected\n + Received\n\n ❯ helper src/area{}/helpers.ts:{}:9\n ❯ {file}:{}:23\n            </failure>",
						c + 1,
						c + 1,
						f % 12,
						c + 5,
						c + 10
					);
				}
			} else if c % 30 == 1 {
				let _ = writeln!(xml, "            <skipped/>");
			}
			let _ = writeln!(xml, "        </testcase>");
		}
		let _ = writeln!(xml, "    </testsuite>");
	}
	let _ = writeln!(xml, "</testsuites>");
	xml
}

fn read(xml: &str) -> pr_results_core::Report {
	let prefix = RepoPath::parse("packages/app/").unwrap();
	parse_junit_in(
		xml.as_bytes(),
		Flavour::Auto,
		&Limits::default(),
		Some(&prefix),
	)
	.unwrap()
}

#[test]
fn the_generated_report_reads_as_a_vitest_report_with_a_prefix() {
	let report = read(&vitest_report(4, 26));
	assert_eq!(report.suites.len(), 4);
	assert!(report.suites.iter().all(|s| s.flavour == Flavour::Vitest));
	let cases: Vec<_> = report.suites.iter().flat_map(|s| &s.cases).collect();
	assert_eq!(cases.len(), 104);
	let failed: Vec<_> = cases
		.iter()
		.filter(|c| c.status == Status::Failed)
		.collect();
	assert_eq!(failed.len(), 8, "cases 0 and 20 of each file");
	for c in failed {
		assert_eq!(c.attempts.len(), 2);
		assert!(c
			.file
			.as_ref()
			.unwrap()
			.as_str()
			.starts_with("packages/app/src/area"));
		assert!(c.file.as_ref().unwrap().as_str().ends_with("helpers.ts"));
	}
}

#[test]
#[ignore = "a measurement, not a check: run with --ignored --nocapture"]
fn five_thousand_two_hundred_cases_parse_quickly_in_little_memory() {
	let xml = vitest_report(130, 40);
	let before = peak_rss_kib();
	let start = Instant::now();
	let mut report = read(&xml);
	let parsed = start.elapsed();
	let after = peak_rss_kib();
	let start = Instant::now();
	report.merge_same_named();
	let merged = start.elapsed();
	let cases: usize = report.suites.iter().map(|s| s.cases.len()).sum();
	println!(
		"input {} KiB, {cases} cases in {} suites; parse {parsed:?}, merge {merged:?}; peak {before:?} KiB before, {after:?} KiB after",
		xml.len() / 1024,
		report.suites.len()
	);
	assert_eq!(cases, 5200);
	assert!(parsed.as_secs() < 5, "parsing took {parsed:?}");
	if let Some(peak) = after {
		assert!(peak < 256 * 1024, "peak {peak} KiB");
	}
}
