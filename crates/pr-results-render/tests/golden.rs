// Golden tests: every scenario renders to the files under `tests/golden/`. Set
// `PR_RESULTS_BLESS=1` to rewrite them, then review the diff as Markdown.
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

use std::collections::BTreeSet;

use common::{assert_inert_all, golden_dir, scenarios, Scenario};

/// Whether the goldens are being rewritten. CI must compare, never rewrite, so blessing there
/// is an error.
fn blessing() -> bool {
	blessing_in(
		std::env::var_os("CI").is_some(),
		std::env::var("PR_RESULTS_BLESS").ok().as_deref(),
	)
}

fn blessing_in(ci: bool, bless: Option<&str>) -> bool {
	let asked = bless == Some("1");
	assert!(
		!(ci && asked),
		"PR_RESULTS_BLESS must not be set when CI is set"
	);
	asked
}

#[test]
fn blessing_in_ci_is_refused() {
	assert!(blessing_in(false, Some("1")));
	assert!(!blessing_in(false, None));
	assert!(!blessing_in(true, None));
	assert!(!blessing_in(true, Some("0")));
	let refused = std::panic::catch_unwind(|| blessing_in(true, Some("1")));
	assert!(refused.is_err());
}

fn check_scenario(sc: &Scenario) {
	let dir = golden_dir().join(&sc.name);
	let outputs = sc.outputs();
	if blessing() {
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).unwrap();
		for (file, text) in &outputs {
			std::fs::write(dir.join(file), text).unwrap();
		}
		return;
	}
	let mut expected: BTreeSet<String> = BTreeSet::new();
	for (file, text) in &outputs {
		expected.insert(file.clone());
		let on_disk = std::fs::read_to_string(dir.join(file)).unwrap_or_else(|_| {
			panic!("{}/{file} is missing: run with PR_RESULTS_BLESS=1", sc.name)
		});
		assert!(
			&on_disk == text,
			"{}/{file} differs from the golden: run with PR_RESULTS_BLESS=1 and review the diff",
			sc.name
		);
	}
	let present: BTreeSet<String> = std::fs::read_dir(&dir)
		.unwrap()
		.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
		.collect();
	assert_eq!(
		present, expected,
		"{}: unexpected files in the golden directory",
		sc.name
	);
}

#[test]
fn every_scenario_matches_its_golden() {
	for sc in scenarios() {
		check_scenario(&sc);
	}
}

#[test]
fn there_is_no_golden_without_a_scenario() {
	if blessing() {
		return;
	}
	let known: BTreeSet<String> = scenarios().into_iter().map(|s| s.name).collect();
	let on_disk: BTreeSet<String> = std::fs::read_dir(golden_dir())
		.unwrap()
		.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
		.collect();
	assert_eq!(on_disk, known);
}

#[test]
fn every_golden_is_inert() {
	for sc in scenarios() {
		assert_inert_all(&sc);
	}
}

#[test]
fn rendering_is_deterministic() {
	for sc in scenarios() {
		assert_eq!(sc.outputs(), sc.outputs(), "{}", sc.name);
	}
}

#[test]
fn each_truncation_scenario_ends_at_its_step() {
	let steps: Vec<_> = scenarios()
		.into_iter()
		.filter_map(|s| {
			s.name
				.strip_prefix("truncated-")
				.map(|n| (n.to_owned(), s.comment().steps_taken))
		})
		.collect();
	assert_eq!(steps.len(), 8, "every step of the budget has a scenario");
	for (name, taken) in steps {
		assert_eq!(
			taken.last().map(|d| d.name()),
			Some(name.as_str()),
			"{name}"
		);
	}
}
