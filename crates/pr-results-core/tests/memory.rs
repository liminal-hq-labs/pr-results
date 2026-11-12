// A bound on peak memory for the shape that used to multiply it. Alone in its file so the process
// is its own and its high-water mark is only this test's.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use pr_results_core::{parse_junit, Flavour, Limits};

/// The peak resident set of this process in KiB, where `/proc` says.
fn peak_rss_kib() -> Option<u64> {
	let status = std::fs::read_to_string("/proc/self/status").ok()?;
	let line = status.lines().find_map(|l| l.strip_prefix("VmHWM:"))?;
	line.split_whitespace().next()?.parse().ok()
}

#[test]
fn a_long_suite_file_is_not_copied_into_every_case() {
	let file = "d/".repeat(512);
	let mut doc = format!("<testsuites><testsuite name=\"s\" file=\"{file}\">");
	for i in 0..200_000 {
		doc.push_str(&format!("<testcase classname=\"c\" name=\"t{i}\"/>"));
	}
	doc.push_str("</testsuite></testsuites>");
	let input = doc.into_bytes();
	let report = parse_junit(&input, Flavour::Generic, &Limits::default()).unwrap();
	assert_eq!(report.suites[0].cases.len(), 200_000);
	if let Some(peak) = peak_rss_kib() {
		// Copying the path into every case took 273 MiB. Sharing it takes about 64 MiB, almost all
		// of it the cases themselves, so the bound has room and still catches a copy.
		assert!(peak < 128 * 1024, "peak {peak} KiB");
	}
}
