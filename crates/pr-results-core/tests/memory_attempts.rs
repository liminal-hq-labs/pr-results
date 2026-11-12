// A bound on peak memory for many cases that each carry many retry attempts. Alone in its file so
// the process is its own and its high-water mark is only this test's.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use pr_results_core::{parse_junit, Flavour, Limits, ParseError};

/// The peak resident set of this process in KiB, where `/proc` says.
fn peak_rss_kib() -> Option<u64> {
	let status = std::fs::read_to_string("/proc/self/status").ok()?;
	let line = status.lines().find_map(|l| l.strip_prefix("VmHWM:"))?;
	line.split_whitespace().next()?.parse().ok()
}

#[test]
fn a_flood_of_empty_attempts_is_refused_before_it_costs_much() {
	// Reserved up front so growing the input does not add a copy to the peak.
	let mut doc = String::with_capacity(50 * 1024 * 1024);
	doc.push_str("<testsuites><testsuite name=\"s\">");
	for i in 0..32_027 {
		doc.push_str(&format!("<testcase classname=\"c\" name=\"t{i}\">"));
		for _ in 0..100 {
			doc.push_str("<flakyFailure/>");
		}
		doc.push_str("</testcase>");
	}
	doc.push_str("</testsuite></testsuites>");
	assert!(doc.len() > 49_000_000 && doc.len() < 50 * 1024 * 1024);
	let before = peak_rss_kib();
	let err = parse_junit(doc.as_bytes(), Flavour::Nextest, &Limits::default()).unwrap_err();
	assert!(matches!(err, ParseError::TooMuchText { .. }));
	if let Some(peak) = peak_rss_kib() {
		println!("peak before parse {before:?} KiB, after {peak} KiB");
		// Without a charge per attempt this parsed and peaked at 285 MiB.
		assert!(peak < 128 * 1024, "peak {peak} KiB");
	}
}
