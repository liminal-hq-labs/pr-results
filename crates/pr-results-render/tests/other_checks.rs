// Tests for the table of the commit's other checks: its order, its words, and how it gives way.
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

use common::{assert_inert_all, load, other, section, Scenario};
use pr_results_core::Flavour;
use pr_results_render::{measure, CheckState, Degradation, Target};

fn passing() -> Scenario {
	Scenario::new(
		"other",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			load(&["nextest/pass.xml"], Flavour::Nextest),
		)],
	)
	.comment_only()
}

#[test]
fn no_other_checks_means_no_table_and_no_change_to_the_heading() {
	let body = passing().comment().body.to_string();
	assert!(!body.contains("Other checks"), "{body}");
	assert!(!body.contains("need attention"), "{body}");
}

#[test]
fn rows_are_ordered_by_what_needs_attention_then_by_name() {
	let body = passing()
		.with_other_checks(common::other_checks_mix())
		.comment()
		.body
		.to_string();
	let rows: Vec<&str> = body
		.lines()
		.skip_while(|l| *l != "| Check | Result |")
		.skip(2)
		.take_while(|l| l.starts_with('|'))
		.collect();
	let names: Vec<&str> = rows
		.iter()
		.map(|r| r.trim_start_matches("| `").split('`').next().unwrap())
		.collect();
	assert_eq!(
		names,
		[
			"Security audit",
			"Deploy preview",
			"Nightly",
			"Docs build",
			"Release dry run",
			"Clippy (all-features)",
			"Clippy (default)",
			"Format",
		],
		"{body}"
	);
}

#[test]
fn the_same_checks_in_any_order_give_the_same_comment() {
	let mut forward = common::other_checks_mix();
	let a = passing()
		.with_other_checks(forward.clone())
		.comment()
		.body
		.to_string();
	forward.reverse();
	let b = passing()
		.with_other_checks(forward)
		.comment()
		.body
		.to_string();
	assert_eq!(a, b);
}

#[test]
fn a_failed_check_is_said_in_the_heading_even_when_every_test_passed() {
	let body = passing()
		.with_other_checks(vec![
			other("Format", CheckState::Passed),
			other("Security audit", CheckState::Failed),
		])
		.comment()
		.body
		.to_string();
	assert!(
		body.contains("### Example checks: All 3 passed, 1 skipped, 1 other check needs attention"),
		"{body}"
	);
}

#[test]
fn running_and_skipped_checks_do_not_change_the_heading() {
	let body = passing()
		.with_other_checks(vec![
			other("Docs build", CheckState::Running),
			other("Release dry run", CheckState::Skipped),
			other("Nightly", CheckState::Cancelled),
		])
		.comment()
		.body
		.to_string();
	assert!(!body.contains("need attention"), "{body}");
	assert!(body.contains("| `Docs build` | Running |"), "{body}");
}

#[test]
fn names_are_escaped_like_every_other_name() {
	let sc = passing().with_other_checks(vec![other(
		"@everyone GH-123 | <b>x</b>",
		CheckState::Failed,
	)]);
	assert_inert_all(&sc);
	let body = sc.comment().body.to_string();
	assert!(body.contains("`@everyone GH-123 \\| <b>x</b>`"), "{body}");
}

#[test]
fn passing_checks_fold_into_one_row_before_anything_else_is_given_up() {
	let mut checks: Vec<_> = (1..=6)
		.map(|n| other(&format!("Job number {n} of the matrix"), CheckState::Passed))
		.collect();
	checks.push(other("Security audit", CheckState::Failed));
	let full = passing().with_other_checks(checks.clone()).comment();
	assert!(full.steps_taken.is_empty());

	let mut sc = passing().with_other_checks(checks);
	sc.options.comment_limit = measure(&full.body) - 1;
	let c = sc.comment();
	assert_eq!(c.steps_taken, [Degradation::OtherChecksCollapsed]);
	let body = c.body.to_string();
	assert!(body.contains("| 6 passing checks | Passed |"), "{body}");
	assert!(body.contains("| `Security audit` | **Failed** |"), "{body}");
	assert!(!body.contains("Job number"), "{body}");
	assert!(
		body.contains("6 passing checks were combined into one row to fit GitHub's size limit."),
		"{body}"
	);
}

#[test]
fn a_few_passing_checks_are_not_folded() {
	let sc = passing().with_other_checks(vec![
		other("Format", CheckState::Passed),
		other("Lint", CheckState::Passed),
		other("Security audit", CheckState::Failed),
	]);
	let full = sc.comment();
	let mut tight = sc.clone();
	tight.options.comment_limit = measure(&full.body) - 1;
	assert!(!tight
		.comment()
		.steps_taken
		.contains(&Degradation::OtherChecksCollapsed));
}

#[test]
fn a_long_list_is_cut_after_the_rows_that_need_attention() {
	let mut checks: Vec<_> = (0..60)
		.map(|n| other(&format!("Job {n:02}"), CheckState::Passed))
		.collect();
	checks.push(other("Security audit", CheckState::Failed));
	let body = passing()
		.with_other_checks(checks)
		.comment()
		.body
		.to_string();
	assert!(body.contains("| `Security audit` | **Failed** |"), "{body}");
	assert!(body.contains("\u{2026}and 11 more checks."), "{body}");
	assert_eq!(body.matches("| `Job ").count(), 49, "{body}");
}

#[test]
fn the_smallest_comment_still_names_a_failed_check() {
	let mut sc =
		common::truncation_base().with_other_checks(vec![other("Audit", CheckState::Failed)]);
	sc.options.comment_limit = 700;
	let c = sc.comment();
	assert_eq!(c.steps_taken.last(), Some(&Degradation::Minimal));
	let body = c.body.to_string();
	assert!(body.contains("1 other check needs attention"), "{body}");
	assert!(body.contains("Other checks: 1 failing."), "{body}");
	assert!(!body.contains("#### Other checks"), "{body}");
	assert_inert_all(&sc);
}

#[test]
fn a_forgejo_comment_with_other_checks_never_says_check_run_or_github() {
	let mut checks: Vec<_> = (1..=6)
		.map(|n| other(&format!("Job number {n} of the matrix"), CheckState::Passed))
		.collect();
	checks.push(other("Audit", CheckState::Failed));
	let mut sc = passing().with_other_checks(checks).forgejo();
	let full = sc.comment();
	sc.options.comment_limit = measure(&full.body) - 1;
	let body = sc.comment().body.to_string();
	assert!(body.contains("passing checks were combined"), "{body}");
	for word in ["check run", "GitHub"] {
		assert!(!body.contains(word), "{word:?} in\n{body}");
	}
	assert_eq!(sc.target, Target::Forgejo);
}
