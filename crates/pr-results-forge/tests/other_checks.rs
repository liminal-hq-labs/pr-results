// Tests for the table of the commit's other check runs: what is listed, what is left out, and
// what happens when the listing fails.
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

use common::{check_golden, passing, Env, SHA};
use pr_results_core::RunId;
use pr_results_forge::{publish_inputs, Fault, Inputs, Method, Note};

const THIS_RUN: &str = "https://github.com/example/app/actions/runs/5000/job/1";
const PUBLISHER_RUN: &str = "https://github.com/example/app/actions/runs/9001/job/2";
const OTHER_WORKFLOW: &str = "https://github.com/example/app/actions/runs/6000/job/3";

fn env() -> Env {
	let mut env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", &passing(3))]);
	env.plan.other_checks = true;
	env
}

fn body(env: &Env) -> String {
	env.bot_comments(7)[0].body.clone()
}

fn rows(body: &str) -> Vec<String> {
	body.lines()
		.skip_while(|l| *l != "| Check | Result |")
		.skip(2)
		.take_while(|l| l.starts_with('|'))
		.map(str::to_owned)
		.collect()
}

#[test]
fn nothing_is_listed_unless_the_setting_is_on() {
	let mut env = env();
	env.plan.other_checks = false;
	env.fake
		.add_other_check("Format", SHA, "completed", Some("success"), Some(THIS_RUN));
	let report = env.publish();
	assert!(report.notes.is_empty(), "{:?}", report.notes);
	assert!(!body(&env).contains("Other checks"));
	let listed = env
		.fake
		.requests()
		.iter()
		.filter(|r| r.method == Method::Get)
		.map(|r| r.target())
		.any(|t| t.contains("check-runs?filter=latest"));
	assert!(
		!listed,
		"the commit's check runs were read without the setting"
	);
}

#[test]
fn the_commits_other_checks_become_a_table() {
	let env = env();
	env.fake
		.add_other_check("Format", SHA, "completed", Some("success"), Some(THIS_RUN));
	env.fake.add_other_check(
		"Security audit",
		SHA,
		"completed",
		Some("failure"),
		Some(THIS_RUN),
	);
	env.fake
		.add_other_check("Docs build", SHA, "in_progress", None, Some(OTHER_WORKFLOW));
	let report = env.publish();
	assert!(!report.failed(), "{:?}", report.log_lines());
	let body = body(&env);
	assert!(
		body.contains("### Example checks: All 3 passed, 1 other check needs attention"),
		"{body}"
	);
	assert_eq!(
		rows(&body),
		[
			"| `Security audit` | **Failed** |",
			"| `Docs build` | Running |",
			"| `Format` | Passed |",
		]
	);
}

#[test]
fn a_check_on_another_commit_is_not_listed() {
	let env = env();
	env.fake.add_other_check(
		"Format",
		common::OLD_SHA,
		"completed",
		Some("failure"),
		None,
	);
	env.publish();
	let body = body(&env);
	assert!(!body.contains("Other checks"), "{body}");
}

#[test]
fn the_check_runs_this_tool_wrote_are_not_listed_among_the_others() {
	let env = env();
	env.publish();
	let first = env.fake.check_runs();
	assert_eq!(first.len(), 1);
	// A re-run: the earlier run's check run is on the commit, and is ours.
	env.fake
		.add_other_check("Lint", SHA, "completed", Some("success"), None);
	env.publish();
	let body = body(&env);
	assert_eq!(rows(&body), ["| `Lint` | Passed |"], "{body}");
	assert!(!body.contains("`Example Rust tests`"), "{body}");
}

#[test]
fn another_tools_check_that_copies_our_external_id_is_still_listed() {
	let env = env();
	env.fake.add_check_from_app(
		"Impostor",
		SHA,
		Some("pr-results:1:ci:Example Rust tests"),
		"some-other-app",
	);
	env.fake.add_check("Plain", SHA, Some("not-ours"));
	env.publish();
	let body = body(&env);
	assert_eq!(
		rows(&body),
		["| `Impostor` | Passed |", "| `Plain` | Passed |"],
		"{body}"
	);
}

#[test]
fn the_publishers_own_run_is_left_out_but_this_runs_jobs_and_other_workflows_are_not() {
	let mut env = env();
	env.plan.own_run = Some(RunId(9001));
	env.fake.add_other_check(
		"publish / Publish",
		SHA,
		"in_progress",
		None,
		Some(PUBLISHER_RUN),
	);
	env.fake
		.add_other_check("Format", SHA, "completed", Some("success"), Some(THIS_RUN));
	env.fake.add_other_check(
		"Deploy preview",
		SHA,
		"completed",
		Some("success"),
		Some(OTHER_WORKFLOW),
	);
	env.publish();
	let body = body(&env);
	assert_eq!(
		rows(&body),
		["| `Deploy preview` | Passed |", "| `Format` | Passed |"],
		"{body}"
	);
}

#[test]
fn a_run_number_that_only_starts_like_ours_is_not_ours() {
	let mut env = env();
	env.plan.own_run = Some(RunId(9001));
	env.fake.add_other_check(
		"Longer id",
		SHA,
		"completed",
		Some("success"),
		Some("https://github.com/example/app/actions/runs/90011/job/2"),
	);
	env.publish();
	assert_eq!(rows(&body(&env)), ["| `Longer id` | Passed |"]);
}

#[test]
fn every_status_and_conclusion_has_its_words() {
	let env = env();
	for (name, status, conclusion) in [
		("a-success", "completed", Some("success")),
		("b-failure", "completed", Some("failure")),
		("c-timed-out", "completed", Some("timed_out")),
		("d-startup", "completed", Some("startup_failure")),
		("e-cancelled", "completed", Some("cancelled")),
		("f-neutral", "completed", Some("neutral")),
		("g-skipped", "completed", Some("skipped")),
		("h-stale", "completed", Some("stale")),
		("i-action", "completed", Some("action_required")),
		("j-new", "completed", Some("something-new")),
		("k-queued", "queued", None),
		("l-waiting", "waiting", None),
		("m-running", "in_progress", None),
	] {
		env.fake
			.add_other_check(name, SHA, status, conclusion, None);
	}
	env.publish();
	assert_eq!(
		rows(&body(&env)),
		[
			"| `b-failure` | **Failed** |",
			"| `c-timed-out` | **Failed** |",
			"| `d-startup` | **Failed** |",
			"| `i-action` | **Action required** |",
			"| `j-new` | **Action required** |",
			"| `e-cancelled` | Cancelled |",
			"| `k-queued` | Running |",
			"| `l-waiting` | Running |",
			"| `m-running` | Running |",
			"| `f-neutral` | Skipped |",
			"| `g-skipped` | Skipped |",
			"| `h-stale` | Skipped |",
			"| `a-success` | Passed |",
		]
	);
}

#[test]
fn a_listing_that_fails_leaves_the_table_out_and_still_posts() {
	let env = env();
	env.fake
		.add_other_check("Format", SHA, "completed", Some("success"), None);
	env.fake.fail(
		Method::Get,
		"check-runs?filter=latest",
		1,
		Fault::status(403),
	);
	let report = env.publish();
	assert!(!report.failed(), "{:?}", report.log_lines());
	assert!(report.notes.contains(&Note::OtherChecksUnavailable));
	let body = body(&env);
	assert!(body.contains("All 3 passed"), "{body}");
	assert!(!body.contains("Other checks"), "{body}");
	assert_eq!(
		env.fake.check_runs().len(),
		2,
		"the suite's check run was written"
	);
}

#[test]
fn publishing_from_the_job_itself_says_it_lists_nothing() {
	let env = env();
	env.fake
		.add_other_check("Format", SHA, "completed", Some("success"), None);
	let inputs = Inputs {
		suites: vec![Vec::new()],
		skipped: vec![0],
	};
	let report = publish_inputs(&env.gh, &env.run, &env.server, &env.plan, &inputs);
	assert!(report.notes.contains(&Note::OtherChecksInJob));
	assert!(!body(&env).contains("Other checks"));
}

#[test]
fn a_hostile_name_is_inert_and_a_long_one_is_cut() {
	let env = env();
	env.fake.add_other_check(
		"@everyone GH-123 <img src=x onerror=alert(1)> | pipe",
		SHA,
		"completed",
		Some("failure"),
		None,
	);
	env.fake
		.add_other_check(&"w".repeat(5000), SHA, "completed", Some("success"), None);
	env.publish();
	let body = body(&env);
	assert!(
		body.contains("| `@everyone GH-123 <img src=x onerror=alert(1)> \\| pipe` | **Failed** |"),
		"{body}"
	);
	assert!(
		body.len() < 5000,
		"a name of 5 000 characters reached the comment"
	);
}

#[test]
fn the_requests_for_other_checks_match_their_golden() {
	let env = env();
	env.fake.add_other_check(
		"Security audit",
		SHA,
		"completed",
		Some("failure"),
		Some(THIS_RUN),
	);
	env.publish();
	check_golden("other-checks", &env.fake.requests());
}

#[test]
fn a_suite_that_names_its_job_hides_that_jobs_row_while_they_agree() {
	let mut env = env();
	env.plan.sections[0].job = Some("Tests".to_owned());
	env.fake
		.add_other_check("Tests", SHA, "completed", Some("success"), None);
	env.fake
		.add_other_check("Format", SHA, "completed", Some("success"), None);
	env.publish();
	assert_eq!(rows(&body(&env)), ["| `Format` | Passed |"]);
}

#[test]
fn a_named_job_that_failed_with_no_results_is_still_listed() {
	let mut env = Env::new();
	env.plan.other_checks = true;
	env.plan.sections[0].job = Some("Tests".to_owned());
	env.plan.sections[0].required = true;
	env.fake
		.add_other_check("Tests", SHA, "completed", Some("failure"), None);
	env.publish();
	let text = body(&env);
	assert!(text.contains("| `Tests` | **Failed** |"), "{text}");
}
