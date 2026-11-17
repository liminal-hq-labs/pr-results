// The publisher's use of a baseline: which run is read, how it is compared and what the comment says.
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

use pr_results_core::{Flavour, Glob, InputFormat};
use pr_results_forge::{BaselineGap, FakeForge, FakePull, FakeRun, Fault, Method, Note};
use pr_results_render::{Options, Style};

use common::forgejo::FjEnv;
use common::{check_golden, heavy, passing, section, zip, Env, OLD_SHA, REPO, SHA, WORKFLOW};

const BASELINE_RUN: u64 = 900;
const RELEASE_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// One suite named `suite` holding a passing test for each of `names`.
fn tests(names: &[&str]) -> String {
	named("suite", names)
}

fn named(suite: &str, names: &[&str]) -> String {
	let mut out = format!("<testsuites><testsuite name=\"{suite}\">");
	for n in names {
		let _ = write!(out, "<testcase classname=\"c\" name=\"{n}\" time=\"0.1\"/>");
	}
	out.push_str("</testsuite></testsuites>");
	out
}

fn guided() -> Options {
	Options {
		style: Style::Guided,
		test_delta: true,
		..Options::default()
	}
}

/// An environment that publishes the guided style, with pull request 7 on `main`.
fn env() -> Env {
	let mut env = Env::new();
	env.plan.options = guided();
	env
}

fn run_on(env: &Env, id: u64, sha: &str, branch: &str) {
	env.fake
		.add_workflow_run(FakeRun::push(id, WORKFLOW, REPO, sha, branch, "success"));
}

fn files(fake: &FakeForge, run: u64, artifact: &str, files: &[(&str, &str)]) -> u64 {
	let specs: Vec<zip::Spec> = files
		.iter()
		.map(|(n, c)| zip::Spec::deflated(n, c.as_bytes()))
		.collect();
	fake.add_artifact(run, artifact, zip::build(&specs))
}

fn body(env: &Env, pr: u64) -> String {
	env.bot_comments(pr)[0].body.clone()
}

/// The lookups of a baseline run, as asked of GitHub.
fn lookups(fake: &FakeForge) -> Vec<String> {
	fake.requests()
		.iter()
		.map(|r| r.target())
		.filter(|t| t.contains("/actions/workflows/"))
		.collect()
}

/// Every request that went to a baseline run: the lookups and its artefacts.
fn baseline_requests(fake: &FakeForge) -> Vec<String> {
	fake.requests()
		.iter()
		.map(|r| r.target())
		.filter(|t| t.contains("/actions/workflows/") || t.contains("/actions/runs/900/"))
		.collect()
}

fn sentence(body: &str) -> Option<&str> {
	body.lines().find(|l| l.starts_with("This pull request "))
}

fn silent(body: &str) {
	assert!(
		sentence(body).is_none() && !body.contains("had no results to compare"),
		"no sentence about tests: {body}"
	);
}

// ---- which baseline ------------------------------------------------------------------------

#[test]
fn the_run_on_the_base_commit_is_the_baseline() {
	let env = env();
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b"]))],
	);
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	run_on(
		&env,
		901,
		"cccccccccccccccccccccccccccccccccccccccc",
		"main",
	);
	files(
		&env.fake,
		901,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c", "d", "e"]))],
	);
	env.artifact(
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c"]))],
	);
	let report = env.publish();
	assert!(!report.failed());
	let body = body(&env, 7);
	assert_eq!(
		sentence(&body),
		Some("This pull request adds 1 test against `main`.")
	);
	assert_eq!(
		lookups(&env.fake),
		[format!(
			"/repos/example/app/actions/workflows/{WORKFLOW}/runs?head_sha={OLD_SHA}&event=push&status=completed&per_page=10"
		)],
		"the commit's run is found first, so no branch is asked for"
	);
	assert!(report.notes.contains(&Note::BaselineUsed {
		pr: 7,
		run: BASELINE_RUN,
		compared: 1,
		of: 1
	}));
}

#[test]
fn the_base_branch_is_asked_for_when_the_commit_has_no_run() {
	let env = env();
	run_on(
		&env,
		BASELINE_RUN,
		"cccccccccccccccccccccccccccccccccccccccc",
		"main",
	);
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	env.publish();
	assert_eq!(
		sentence(&body(&env, 7)),
		Some("This pull request removes 2 tests against `main`.")
	);
	let asked = lookups(&env.fake);
	assert_eq!(asked.len(), 2);
	assert!(asked[0].contains("head_sha=") && asked[1].contains("branch=main&"));
}

#[test]
fn a_base_with_no_run_falls_back_to_the_configured_branch_and_says_so() {
	let mut env = env();
	env.fake.add_pull(FakePull {
		base_ref: "release/1.x".to_owned(),
		base_sha: RELEASE_SHA.to_owned(),
		..FakePull::same_repo(8, REPO, SHA, "feature")
	});
	env.run.pull_requests = vec![8];
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b"]))],
	);
	env.artifact(
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c"]))],
	);
	env.publish();
	assert_eq!(
		sentence(&body(&env, 8)),
		Some("This pull request adds 1 test against `main`, because `release/1.x` had no results to compare with.")
	);
	let asked = lookups(&env.fake);
	assert_eq!(asked.len(), 3, "commit, base branch, then the fallback");
	assert!(asked[1].contains("branch=release%2F1.x&") && asked[2].contains("branch=main&"));
}

#[test]
fn the_fallback_is_not_asked_twice_when_it_is_the_base_branch() {
	let env = env();
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	let report = env.publish();
	assert_eq!(
		lookups(&env.fake).len(),
		2,
		"commit and branch, and no repeat"
	);
	assert!(report.notes.contains(&Note::BaselineNotFound { pr: 7 }));
}

#[test]
fn a_comparison_by_the_fallback_can_use_another_name_for_it() {
	let mut env = env();
	env.plan.baseline_branch = "develop".to_owned();
	run_on(
		&env,
		BASELINE_RUN,
		"cccccccccccccccccccccccccccccccccccccccc",
		"develop",
	);
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a", "b"]))]);
	env.publish();
	assert_eq!(
		sentence(&body(&env, 7)),
		Some("This pull request adds 1 test against `develop`, because `main` had no results to compare with.")
	);
}

#[test]
fn no_baseline_means_no_sentence_no_warning_and_a_note() {
	let env = env();
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	let report = env.publish();
	assert!(!report.failed());
	let comment = body(&env, 7);
	silent(&comment);
	assert!(!comment.to_lowercase().contains("baseline"), "{comment}");
	assert!(report.notes.contains(&Note::BaselineNotFound { pr: 7 }));
	assert!(report
		.log_lines()
		.iter()
		.any(|l| l.starts_with("No baseline run was found for pull request 7")));
	assert_eq!(report.comments.len(), 1);
}

#[test]
fn the_triggering_run_is_never_its_own_baseline() {
	let env = env();
	run_on(&env, common::RUN, OLD_SHA, "main");
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	let report = env.publish();
	silent(&body(&env, 7));
	assert!(report.notes.contains(&Note::BaselineNotFound { pr: 7 }));
	assert_eq!(
		lookups(&env.fake).len(),
		2,
		"the commit and the branch were asked for"
	);
	assert_eq!(
		env.fake.blob_fetches().len(),
		1,
		"only its own artefact was read"
	);
}

#[test]
fn a_lookup_that_fails_leaves_the_comment_as_it_was() {
	let env = env();
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	env.fake
		.fail(Method::Get, "actions/workflows", 1, Fault::status(404));
	let report = env.publish();
	assert!(!report.failed());
	silent(&body(&env, 7));
	assert!(report.notes.contains(&Note::BaselineUnavailable { pr: 7 }));
}

// ---- reading it ----------------------------------------------------------------------------

#[test]
fn an_expired_artefact_leaves_no_baseline() {
	let env = env();
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	let id = files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b"]))],
	);
	env.fake.expire_artifact(id);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	let report = env.publish();
	silent(&body(&env, 7));
	assert!(report.notes.contains(&Note::BaselineUnusable {
		pr: 7,
		why: BaselineGap::Expired
	}));
	assert!(env.fake.blob_fetches().len() == 1, "only the run's own");
}

#[test]
fn a_baseline_with_no_matching_file_or_no_tests_is_left_out() {
	let env = env();
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("notes.txt", "hello")],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	let report = env.publish();
	silent(&body(&env, 7));
	assert!(report.notes.contains(&Note::BaselineUnusable {
		pr: 7,
		why: BaselineGap::NoResults
	}));
}

#[test]
fn a_baseline_file_that_does_not_parse_leaves_its_section_out() {
	let env = env();
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", "<testsuites><oops")],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	let report = env.publish();
	silent(&body(&env, 7));
	assert!(report.notes.contains(&Note::BaselineUnusable {
		pr: 7,
		why: BaselineGap::Unreadable
	}));
	assert!(
		!report
			.notes
			.iter()
			.any(|n| matches!(n, Note::InputUnreadable { .. })),
		"the baseline's files are not the run's own"
	);
}

#[test]
fn a_baseline_over_the_limit_is_left_out_and_the_runs_results_are_not() {
	let mut env = env();
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	let big: Vec<String> = (0..400).map(|i| format!("test_{i}")).collect();
	let big: Vec<&str> = big.iter().map(String::as_str).collect();
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&big))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	env.plan.archive.max_total_bytes = 4096;
	let report = env.publish();
	silent(&body(&env, 7));
	assert!(report.notes.contains(&Note::BaselineUnusable {
		pr: 7,
		why: BaselineGap::TooLarge
	}));
	assert!(
		body(&env, 7).contains("All 1 passed"),
		"the run's own results are read in full"
	);
}

#[test]
fn the_baseline_has_a_budget_of_its_own() {
	let mut env = env();
	let xml = tests(&["a", "b", "c", "d", "e", "f"]);
	let compressed = zip::build(&[zip::Spec::deflated("junit.xml", xml.as_bytes())]).len() as u64;
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c", "d", "e"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &xml)]);
	// Room for one of the two runs' artefacts and not for both.
	env.plan.archive.max_total_bytes = xml.len() as u64 + 20;
	env.plan.max_download_bytes = compressed + 20;
	let report = env.publish();
	assert_eq!(
		sentence(&body(&env, 7)),
		Some("This pull request adds 1 test against `main`.")
	);
	assert!(!report
		.notes
		.iter()
		.any(|n| matches!(n, Note::BaselineUnusable { .. })));
}

#[test]
fn only_the_sections_that_can_be_compared_are_downloaded() {
	let mut env = env();
	let mut steps = section("Steps", "Example Steps", "steps-results");
	steps.flavour = Flavour::Steps;
	let mut sarif = section("Lint", "Example Lint", "sarif-results");
	sarif.format = InputFormat::Sarif;
	sarif.files = vec![Glob::new("**/*.sarif").unwrap()];
	env.plan.sections.extend([steps, sarif]);
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	let wanted = files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	files(
		&env.fake,
		BASELINE_RUN,
		"steps-results",
		&[("s.xml", &tests(&["s"]))],
	);
	files(
		&env.fake,
		BASELINE_RUN,
		"sarif-results",
		&[("l.sarif", "{}")],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a", "b"]))]);
	env.artifact("steps-results", &[("s.xml", &tests(&["s", "t", "u"]))]);
	let report = env.publish();
	assert_eq!(
		sentence(&body(&env, 7)),
		Some("This pull request adds 1 test against `main`."),
		"the steps section's extra tests are not counted"
	);
	let baseline_blobs: Vec<String> = env
		.fake
		.blob_fetches()
		.into_iter()
		.filter(|b| b.ends_with(&format!("/{wanted}")))
		.collect();
	assert_eq!(baseline_blobs.len(), 1);
	assert_eq!(
		env.fake.blob_fetches().len(),
		3,
		"two of the run's own and one of the baseline's"
	);
	assert!(report.notes.contains(&Note::BaselineUsed {
		pr: 7,
		run: BASELINE_RUN,
		compared: 1,
		of: 1
	}));
}

#[test]
fn a_run_with_nothing_comparable_asks_for_no_baseline() {
	let mut env = Env::new();
	env.plan.options = guided();
	env.plan.sections[0].flavour = Flavour::Steps;
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	env.publish();
	assert!(lookups(&env.fake).is_empty());

	let mut env = Env::new();
	env.plan.options = guided();
	env.plan.sections[0].format = InputFormat::Sarif;
	env.plan.sections[0].files = vec![Glob::new("**/*.sarif").unwrap()];
	env.artifact("rust-test-results", &[("a.sarif", "{}")]);
	env.publish();
	assert!(lookups(&env.fake).is_empty());
}

// ---- what is compared ----------------------------------------------------------------------

#[test]
fn a_section_the_baseline_lacks_is_left_out_on_both_sides() {
	let mut env = env();
	env.plan
		.sections
		.push(section("Web tests", "Example Web", "web-test-results"));
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &named("rust", &["a", "b"]))],
	);
	env.artifact(
		"rust-test-results",
		&[("junit.xml", &named("rust", &["a", "b", "c"]))],
	);
	env.artifact(
		"web-test-results",
		&[("junit.xml", &named("web", &["x", "y", "z"]))],
	);
	let report = env.publish();
	assert_eq!(
		sentence(&body(&env, 7)),
		Some("This pull request adds 1 test against `main`."),
		"the web tests are not all added"
	);
	assert!(report.notes.contains(&Note::BaselineUsed {
		pr: 7,
		run: BASELINE_RUN,
		compared: 1,
		of: 2
	}));
}

#[test]
fn a_section_the_run_could_not_read_in_full_is_not_compared() {
	let env = env();
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c"]))],
	);
	env.artifact(
		"rust-test-results",
		&[
			("junit.xml", &tests(&["a"])),
			("bad.xml", "<testsuites><oops"),
		],
	);
	env.publish();
	silent(&body(&env, 7));
	assert!(lookups(&env.fake).is_empty(), "nothing is asked for");

	let env = self::env();
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c"]))],
	);
	env.publish();
	silent(&body(&env, 7));
	assert!(lookups(&env.fake).is_empty(), "the run uploaded nothing");
}

#[test]
fn a_renamed_section_title_does_not_matter() {
	let mut env = env();
	env.plan.sections[0].title = "Renamed".to_owned();
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a", "b"]))]);
	env.publish();
	silent(&body(&env, 7));
}

// ---- several pull requests -----------------------------------------------------------------

fn two_pulls(env: &mut Env, second_base: Option<(&str, &str)>) {
	env.fake
		.add_pull(FakePull::same_repo(8, REPO, SHA, "feature"));
	let mut other = FakePull::same_repo(9, REPO, SHA, "feature");
	if let Some((base_ref, base_sha)) = second_base {
		other.base_ref = base_ref.to_owned();
		other.base_sha = base_sha.to_owned();
	}
	env.fake.add_pull(other);
	env.run.pull_requests = vec![8, 9];
}

#[test]
fn two_pull_requests_on_one_base_cost_one_lookup_and_one_download() {
	let mut env = env();
	two_pulls(&mut env, None);
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	let id = files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a", "b"]))]);
	let report = env.publish();
	assert_eq!(report.comments.len(), 2);
	assert_eq!(body(&env, 8), body(&env, 9));
	assert_eq!(
		sentence(&body(&env, 8)),
		Some("This pull request adds 1 test against `main`.")
	);
	assert_eq!(lookups(&env.fake).len(), 1);
	assert_eq!(
		baseline_requests(&env.fake)
			.iter()
			.filter(|t| t.contains("/artifacts"))
			.count(),
		1
	);
	assert_eq!(
		env.fake
			.blob_fetches()
			.iter()
			.filter(|b| b.ends_with(&format!("/{id}")))
			.count(),
		1
	);
	assert_eq!(
		report
			.notes
			.iter()
			.filter(|n| matches!(n, Note::BaselineUsed { .. }))
			.count(),
		1
	);
}

#[test]
fn two_pull_requests_on_different_bases_get_their_own_sentences() {
	let mut env = env();
	two_pulls(&mut env, Some(("release/1.x", RELEASE_SHA)));
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b"]))],
	);
	run_on(&env, 901, RELEASE_SHA, "release/1.x");
	files(
		&env.fake,
		901,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	env.artifact(
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c"]))],
	);
	let report = env.publish();
	assert_eq!(
		sentence(&body(&env, 8)),
		Some("This pull request adds 1 test against `main`.")
	);
	assert_eq!(
		sentence(&body(&env, 9)),
		Some("This pull request adds 2 tests against `release/1.x`.")
	);
	assert!(report
		.job_summary
		.as_deref()
		.unwrap()
		.contains("adds 1 test against `main`"));
	assert!(!report
		.job_summary
		.as_deref()
		.unwrap()
		.contains("release/1.x"));
}

#[test]
fn pull_requests_with_equal_outcomes_share_one_body() {
	let mut env = env();
	two_pulls(&mut env, Some(("main", RELEASE_SHA)));
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	run_on(&env, 901, RELEASE_SHA, "main");
	files(
		&env.fake,
		901,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a", "b"]))]);
	env.publish();
	assert_eq!(body(&env, 8), body(&env, 9));
	assert_eq!(lookups(&env.fake).len(), 2, "two bases, so two lookups");
}

#[test]
fn a_pull_request_without_a_baseline_gets_the_plain_body_beside_one_that_has_it() {
	let mut env = env();
	two_pulls(&mut env, Some(("release/1.x", RELEASE_SHA)));
	env.plan.baseline_branch = "develop".to_owned();
	run_on(&env, 901, RELEASE_SHA, "release/1.x");
	files(
		&env.fake,
		901,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a", "b"]))]);
	let report = env.publish();
	silent(&body(&env, 8));
	assert_eq!(
		sentence(&body(&env, 9)),
		Some("This pull request adds 1 test against `release/1.x`.")
	);
	assert!(report.notes.contains(&Note::BaselineNotFound { pr: 8 }));
	assert!(
		!report
			.job_summary
			.as_deref()
			.unwrap()
			.contains("adds 1 test"),
		"the summary shows the first pull request's comment"
	);
}

// ---- when it does not run ------------------------------------------------------------------

fn nothing_asked(env: &Env) {
	env.fake.requests().iter().for_each(|r| {
		let t = r.target();
		assert!(
			!t.contains("/actions/workflows/"),
			"a baseline was looked for: {t}"
		);
	});
	assert_eq!(
		env.fake
			.requests()
			.iter()
			.filter(|r| r.target().contains("/actions/runs/"))
			.count(),
		1,
		"only the run's own artefacts were listed"
	);
}

fn with_baseline_waiting(env: &Env) {
	run_on(env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a", "b"]))]);
}

#[test]
fn the_plain_style_asks_for_no_baseline() {
	let mut env = Env::new();
	env.plan.options = Options {
		style: Style::Plain,
		test_delta: true,
		..Options::default()
	};
	with_baseline_waiting(&env);
	env.publish();
	nothing_asked(&env);
}

#[test]
fn a_switched_off_line_asks_for_no_baseline() {
	let mut env = Env::new();
	env.plan.options = Options {
		test_delta: false,
		..guided()
	};
	with_baseline_waiting(&env);
	env.publish();
	nothing_asked(&env);
	silent(&body(&env, 7));
}

#[test]
fn an_empty_baseline_branch_asks_for_no_baseline() {
	let mut env = env();
	env.plan.baseline_branch = String::new();
	with_baseline_waiting(&env);
	env.publish();
	nothing_asked(&env);
	silent(&body(&env, 7));
}

#[test]
fn an_event_without_a_workflow_asks_for_no_baseline() {
	let mut env = env();
	env.run.workflow_id = None;
	with_baseline_waiting(&env);
	env.publish();
	nothing_asked(&env);
}

#[test]
fn a_push_has_no_comment_and_asks_for_no_baseline() {
	let mut env = env();
	env.run.event = "push".to_owned();
	with_baseline_waiting(&env);
	env.publish();
	nothing_asked(&env);
}

#[test]
fn forgejo_asks_for_no_baseline() {
	let mut env = FjEnv::new(&tests(&["a", "b"]));
	env.run.workflow_id = Some(WORKFLOW);
	env.plan.options = guided();
	let report = env.publish();
	assert!(!report.failed());
	let asked: Vec<String> = env.fake.requests().iter().map(|r| r.target()).collect();
	assert!(
		!asked.iter().any(|t| t.contains("/actions/")),
		"no workflow or run was read: {asked:?}"
	);
	assert!(!report
		.notes
		.iter()
		.any(|n| matches!(n, Note::BaselineNotFound { .. })));
}

// ---- untrusted names -----------------------------------------------------------------------

#[test]
fn a_hostile_base_branch_is_shown_only_in_a_code_span() {
	let mut env = env();
	let hostile = "x`](https://evil.example) @octocat #1 <b>hi</b>";
	env.fake.add_pull(FakePull {
		base_ref: hostile.to_owned(),
		base_sha: RELEASE_SHA.to_owned(),
		..FakePull::same_repo(8, REPO, SHA, "feature")
	});
	env.run.pull_requests = vec![8];
	run_on(
		&env,
		BASELINE_RUN,
		"cccccccccccccccccccccccccccccccccccccccc",
		hostile,
	);
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a", "b"]))]);
	env.publish();
	let comment = body(&env, 8);
	let line = sentence(&comment).expect("a sentence");
	assert_eq!(
		line,
		"This pull request adds 1 test against ``x`](https://evil.example) @octocat #1 <b>hi</b>``."
	);
	assert!(
		lookups(&env.fake)
			.iter()
			.any(|t| t.contains("branch=x%60%5D%28https%3A%2F%2Fevil.example%29")),
		"the name is percent-encoded in the request"
	);
}

#[test]
fn a_long_base_branch_name_is_cut_in_the_sentence() {
	let mut env = env();
	let long = "b".repeat(250);
	env.fake.add_pull(FakePull {
		base_ref: long.clone(),
		base_sha: RELEASE_SHA.to_owned(),
		..FakePull::same_repo(8, REPO, SHA, "feature")
	});
	env.run.pull_requests = vec![8];
	run_on(
		&env,
		BASELINE_RUN,
		"cccccccccccccccccccccccccccccccccccccccc",
		&long,
	);
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a"]))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a", "b"]))]);
	env.publish();
	let comment = body(&env, 8);
	let line = sentence(&comment).unwrap();
	assert!(!line.contains(&long), "{line}");
	assert!(line.len() < 200, "{line}");
}

// ---- size ----------------------------------------------------------------------------------

#[test]
fn the_minimal_comment_is_still_posted_when_the_full_one_is_too_large() {
	let mut env = env();
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &passing(1))],
	);
	env.artifact("rust-test-results", &[("junit.xml", &heavy(30, 4000))]);
	env.plan.options.comment_limit = 100_000;
	env.fake.set_limits(3000, 65_535, 50);
	let report = env.publish();
	assert!(!report.failed(), "{:?}", report.failures);
	assert!(report.notes.contains(&Note::CommentMinimal { pr: 7 }));
	let posted = body(&env, 7);
	assert!(posted.chars().count() <= 3000);
}

// ---- the request goldens -------------------------------------------------------------------

#[test]
fn golden_requests_for_a_baseline_found_by_the_base_commit() {
	let env = env();
	run_on(&env, BASELINE_RUN, OLD_SHA, "main");
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b"]))],
	);
	env.artifact(
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c"]))],
	);
	env.publish();
	check_golden("baseline-by-commit", &env.fake.requests());
}

#[test]
fn golden_requests_for_a_baseline_that_falls_back_to_the_configured_branch() {
	let mut env = env();
	env.fake.add_pull(FakePull {
		base_ref: "release/1.x".to_owned(),
		base_sha: RELEASE_SHA.to_owned(),
		..FakePull::same_repo(8, REPO, SHA, "feature")
	});
	env.run.pull_requests = vec![8];
	run_on(
		&env,
		BASELINE_RUN,
		"cccccccccccccccccccccccccccccccccccccccc",
		"main",
	);
	files(
		&env.fake,
		BASELINE_RUN,
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b"]))],
	);
	env.artifact(
		"rust-test-results",
		&[("junit.xml", &tests(&["a", "b", "c"]))],
	);
	env.publish();
	check_golden("baseline-fallback", &env.fake.requests());
}

#[test]
fn golden_requests_for_a_baseline_that_is_not_found() {
	let env = env();
	env.artifact("rust-test-results", &[("junit.xml", &tests(&["a"]))]);
	env.publish();
	check_golden("baseline-not-found", &env.fake.requests());
}
