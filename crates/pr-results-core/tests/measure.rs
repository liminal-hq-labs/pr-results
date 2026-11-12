// Measures peak memory and merge time for the hostile shapes in the limits tests. Run by hand.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use std::time::Instant;

use pr_results_core::{parse_junit, Flavour, Limits, Report, Suite, Untrusted};

fn peak_rss_kib() -> u64 {
	std::fs::read_to_string("/proc/self/status")
		.ok()
		.and_then(|s| {
			s.lines()
				.find_map(|l| l.strip_prefix("VmHWM:").map(str::to_owned))
		})
		.and_then(|l| l.split_whitespace().next().and_then(|n| n.parse().ok()))
		.unwrap_or(0)
}

#[test]
#[ignore = "a measurement, not a check: run with --ignored --nocapture"]
fn measure_file_attribute_amplification() {
	let file = "d/".repeat(512);
	let mut doc = format!("<testsuites><testsuite name=\"s\" file=\"{file}\">");
	for i in 0..200_000 {
		doc.push_str(&format!("<testcase classname=\"c\" name=\"t{i}\"/>"));
	}
	doc.push_str("</testsuite></testsuites>");
	let input = doc.into_bytes();
	let before = peak_rss_kib();
	let report = parse_junit(&input, Flavour::Generic, &Limits::default()).unwrap();
	let after = peak_rss_kib();
	println!(
		"input {} KiB; peak before parse {before} KiB; peak after {after} KiB; cases {}",
		input.len() / 1024,
		report.suites[0].cases.len()
	);
}

#[test]
#[ignore = "a measurement, not a check: run with --ignored --nocapture"]
fn measure_merge_time() {
	let n = 100_000;
	let mut report = Report {
		suites: (0..n)
			.map(|i| Suite {
				name: Untrusted::new(format!("suite {i}")),
				flavour: Flavour::Generic,
				cases: Vec::new(),
				time_ms: 0,
			})
			.collect(),
	};
	let start = Instant::now();
	report.merge_same_named();
	println!("merged {n} suites in {:?}", start.elapsed());
}
