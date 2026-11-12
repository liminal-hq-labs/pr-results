// Budget tests: at every limit the output fits, and the steps are taken in the order SPEC gives.
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

use common::{assert_inert_all, parse_text, section, synthetic_nextest, Scenario};
use pr_results_core::Flavour;
use pr_results_render::{measure, Degradation, Options};

/// GitHub's own limit on a comment body, in UTF-16 units.
const GITHUB_COMMENT: usize = 65_536;
/// And on a check run's summary and text.
const GITHUB_CHECK: usize = 65_535;

fn big() -> Scenario {
	let mut sections = vec![section(
		"Rust tests",
		false,
		parse_text(&synthetic_nextest("big", 200, 40, 50, 40), Flavour::Nextest),
	)];
	for name in ["Alpha", "Bravo", "Charlie", "Delta"] {
		sections.push(section(
			name,
			false,
			parse_text(&synthetic_nextest(name, 3, 5, 10, 2), Flavour::Nextest),
		));
	}
	for name in ["Echo", "Foxtrot", "Golf", "Hotel", "India", "Juliet"] {
		sections.push(section(
			name,
			false,
			parse_text(&synthetic_nextest(name, 0, 0, 10, 0), Flavour::Nextest),
		));
	}
	Scenario::new("big", "Example checks", sections)
}

/// Failures whose every line is astral: one character, but two UTF-16 units.
fn astral() -> Scenario {
	let xml = synthetic_nextest("wide", 200, 60, 10, 10).replace("expected", "\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}");
	Scenario::new(
		"astral",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			parse_text(&xml, Flavour::Nextest),
		)],
	)
}

fn with_limits(mut sc: Scenario, limit: usize) -> Scenario {
	sc.options = Options {
		comment_limit: limit,
		summary_limit: limit,
		text_limit: limit,
		max_failures: 50,
		..Options::default()
	};
	sc
}

fn utf16(s: &str) -> usize {
	s.encode_utf16().count()
}

fn assert_fits(sc: &Scenario, limit: usize) {
	let c = sc.comment();
	assert!(
		measure(&c.body) <= limit,
		"{}: comment {} > {limit}",
		sc.name,
		measure(&c.body)
	);
	assert!(utf16(c.body.as_str()) <= GITHUB_COMMENT.min(limit));
	for index in 0..sc.sections.len() {
		let out = sc.with_ctx(|i| pr_results_render::check(i, index)).unwrap();
		for (what, text) in [("summary", &out.summary), ("text", &out.text)] {
			assert!(
				measure(text) <= limit,
				"{} check {index} {what}: {} > {limit}",
				sc.name,
				measure(text)
			);
			assert!(utf16(text.as_str()) <= GITHUB_CHECK.min(limit));
		}
		assert!(out.annotations.len() <= 50);
	}
}

#[test]
fn every_output_fits_at_every_limit() {
	for limit in [2_000, 10_000, 60_000] {
		assert_fits(&with_limits(big(), limit), limit);
		assert_fits(&with_limits(astral(), limit), limit);
	}
}

#[test]
fn the_defaults_stay_under_githubs_limits_even_for_wide_characters() {
	let sc = astral();
	let c = sc.comment();
	assert!(
		utf16(c.body.as_str()) <= GITHUB_COMMENT,
		"{}",
		utf16(c.body.as_str())
	);
	let out = sc.with_ctx(|i| pr_results_render::check(i, 0)).unwrap();
	assert!(utf16(out.summary.as_str()) <= GITHUB_CHECK);
	assert!(utf16(out.text.as_str()) <= GITHUB_CHECK);
}

#[test]
fn the_steps_run_in_spec_order_and_each_once() {
	for limit in [2_000, 10_000, 60_000] {
		let steps = with_limits(big(), limit).comment().steps_taken;
		for pair in steps.windows(2) {
			assert!(pair[0] < pair[1], "{limit}: {steps:?}");
		}
		for step in &steps {
			assert!(Degradation::ALL.contains(step));
		}
	}
}

#[test]
fn a_smaller_limit_takes_the_same_steps_and_then_more() {
	let at = |limit| with_limits(big(), limit).comment().steps_taken;
	let (s60, s10, s2) = (at(60_000), at(10_000), at(2_000));
	assert!(s10.starts_with(&s60), "{s60:?} {s10:?}");
	assert!(s2.starts_with(&s10), "{s10:?} {s2:?}");
	assert!(s2.len() > s60.len());
	assert!(s2.last() >= Some(&Degradation::NoFailures), "{s2:?}");
}

#[test]
fn a_comment_that_fits_takes_no_step() {
	let sc = Scenario::new(
		"small",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			parse_text(&synthetic_nextest("core", 1, 3, 3, 0), Flavour::Nextest),
		)],
	);
	assert!(sc.comment().steps_taken.is_empty());
}

#[test]
fn every_step_says_what_it_left_out() {
	let base = common::truncation_base();
	let limits = common::truncation_limits();
	assert_eq!(limits.len(), 8);
	let mut previous = String::new();
	for (step, limit) in limits {
		let mut sc = base.clone();
		sc.options.comment_limit = limit;
		let body = sc.comment().body.to_string();
		assert_ne!(body, previous, "{step:?} changed nothing");
		let said = match step {
			Degradation::ExtrasToCounts => "The list of 8 flaky tests was left out",
			Degradation::Excerpt8 => "shortened to 8 lines",
			Degradation::Excerpt3 => "shortened to 3 lines",
			Degradation::Failures20 => "more failures in the check runs",
			Degradation::Failures5 => "more failures in the check runs",
			Degradation::NoFailures => "failures are not listed here",
			Degradation::PassingCollapsed => "combined into one row",
			Degradation::Minimal => "Most of this comment was left out",
		};
		assert!(body.contains(said), "{step:?}: {said:?} is not in\n{body}");
		previous = body;
	}
}

#[test]
fn the_smallest_comment_is_inert_and_small() {
	let mut sc = common::truncation_base();
	sc.options.comment_limit = 700;
	let c = sc.comment();
	assert_eq!(c.steps_taken.last(), Some(&Degradation::Minimal));
	assert!(measure(&c.body) <= 700);
	assert_inert_all(&sc);
}
