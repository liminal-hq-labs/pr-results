// Baseline run lookup: the routes asked, what a listing may supply and what is refused.
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

use pr_results_core::{RepoSlug, RunId, Sha};
use pr_results_forge::{
	BaselineQuery, FakeForge, FakeForgejo, FakeRun, Fault, ForgeError, Forgejo, GitHub, Host,
	Method,
};

use common::{Env, BOT, REPO, SHA, WORKFLOW};

const BASE: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const OLDER: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";

fn world() -> (FakeForge, GitHub<FakeForge>) {
	let fake = FakeForge::new(REPO);
	let gh = GitHub::new(fake.clone(), RepoSlug::parse(REPO).unwrap(), BOT);
	(fake, gh)
}

fn push(id: u64, sha: &str, branch: &str, conclusion: &str) -> FakeRun {
	FakeRun::push(id, WORKFLOW, REPO, sha, branch, conclusion)
}

fn targets(fake: &FakeForge) -> Vec<String> {
	fake.requests().iter().map(|r| r.target()).collect()
}

#[test]
fn the_run_on_an_exact_commit_is_asked_for_with_the_commit_and_nothing_wider() {
	let (fake, gh) = world();
	fake.add_workflow_run(push(1, BASE, "main", "success"));
	fake.add_workflow_run(push(2, OLDER, "main", "success"));
	let sha = Sha::parse(BASE).unwrap();
	let found = gh
		.baseline_run(WORKFLOW, &BaselineQuery::HeadSha(&sha))
		.unwrap()
		.unwrap();
	assert_eq!(found.id, RunId(1));
	assert_eq!(found.head_sha, sha);
	assert_eq!(found.head_branch.as_deref(), Some("main"));
	assert_eq!(found.conclusion, "success");
	assert_eq!(
		targets(&fake),
		[format!(
			"/repos/example/app/actions/workflows/{WORKFLOW}/runs?head_sha={BASE}&event=push&status=completed&per_page=10"
		)]
	);
}

#[test]
fn the_newest_run_on_a_branch_is_asked_for_with_the_branch_percent_encoded() {
	let (fake, gh) = world();
	fake.add_workflow_run(push(1, OLDER, "release/1.x", "success"));
	fake.add_workflow_run(push(2, BASE, "release/1.x", "failure"));
	fake.add_workflow_run(push(3, SHA, "main", "success"));
	let found = gh
		.baseline_run(WORKFLOW, &BaselineQuery::Branch("release/1.x"))
		.unwrap()
		.unwrap();
	assert_eq!(found.id, RunId(2), "a failed base is still the newest");
	assert_eq!(found.conclusion, "failure");
	assert_eq!(
		targets(&fake),
		[format!(
			"/repos/example/app/actions/workflows/{WORKFLOW}/runs?branch=release%2F1.x&event=push&status=completed&per_page=10"
		)]
	);
}

#[test]
fn a_run_that_cannot_be_a_baseline_is_passed_over_for_an_older_one_that_can() {
	let (fake, gh) = world();
	fake.add_workflow_run(push(1, OLDER, "main", "success"));
	fake.add_workflow_run(push(2, BASE, "main", "cancelled"));
	let mut unfinished = push(3, SHA, "main", "success");
	unfinished.status = "in_progress".to_owned();
	unfinished.conclusion = None;
	fake.add_workflow_run(unfinished);
	let found = gh
		.baseline_run(WORKFLOW, &BaselineQuery::Branch("main"))
		.unwrap()
		.unwrap();
	assert_eq!(found.id, RunId(1));
}

#[test]
fn a_fork_with_the_same_branch_name_cannot_supply_a_baseline() {
	let (fake, gh) = world();
	fake.add_workflow_run(push(1, OLDER, "main", "success"));
	let mut forked = push(2, BASE, "main", "success");
	forked.head_repo = Some("forker/app".to_owned());
	fake.add_workflow_run(forked);
	let mut deleted = push(3, SHA, "main", "success");
	deleted.head_repo = None;
	fake.add_workflow_run(deleted);
	let found = gh
		.baseline_run(WORKFLOW, &BaselineQuery::Branch("main"))
		.unwrap()
		.unwrap();
	assert_eq!(found.id, RunId(1), "the newest runs are the fork's");

	let sha = Sha::parse(BASE).unwrap();
	assert_eq!(
		gh.baseline_run(WORKFLOW, &BaselineQuery::HeadSha(&sha))
			.unwrap(),
		None,
		"the base commit's only run is a fork's"
	);
}

#[test]
fn a_pull_request_run_on_a_branch_of_the_same_name_is_not_a_baseline() {
	let (fake, gh) = world();
	let mut pr = push(1, BASE, "main", "success");
	pr.event = "pull_request".to_owned();
	fake.add_workflow_run(pr);
	assert_eq!(
		gh.baseline_run(WORKFLOW, &BaselineQuery::Branch("main"))
			.unwrap(),
		None
	);
}

#[test]
fn only_the_workflow_asked_for_is_looked_at() {
	let (fake, gh) = world();
	fake.add_workflow_run(FakeRun::push(1, 999, REPO, BASE, "main", "success"));
	assert_eq!(
		gh.baseline_run(WORKFLOW, &BaselineQuery::Branch("main"))
			.unwrap(),
		None
	);
	assert_eq!(
		gh.baseline_run(999, &BaselineQuery::Branch("main"))
			.unwrap()
			.map(|r| r.id),
		Some(RunId(1))
	);
}

#[test]
fn no_runs_is_none_and_a_workflow_that_cannot_be_read_is_an_error() {
	let (fake, gh) = world();
	assert_eq!(
		gh.baseline_run(WORKFLOW, &BaselineQuery::Branch("main"))
			.unwrap(),
		None
	);
	fake.fail(Method::Get, "actions/workflows", 1, Fault::status(404));
	assert_eq!(
		gh.baseline_run(WORKFLOW, &BaselineQuery::Branch("main")),
		Err(ForgeError::Status {
			status: 404,
			op: "listing workflow runs"
		})
	);
}

#[test]
fn a_branch_that_cannot_be_asked_for_makes_no_request() {
	let (fake, gh) = world();
	for branch in ["", "main\nevil", &"x".repeat(256)] {
		assert_eq!(
			gh.baseline_run(WORKFLOW, &BaselineQuery::Branch(branch))
				.unwrap(),
			None
		);
	}
	assert!(fake.requests().is_empty());
}

#[test]
fn only_one_page_of_ten_runs_is_read() {
	let (fake, gh) = world();
	fake.add_workflow_run(push(1, OLDER, "main", "success"));
	for id in 2..12 {
		fake.add_workflow_run(push(id, BASE, "main", "cancelled"));
	}
	assert_eq!(
		gh.baseline_run(WORKFLOW, &BaselineQuery::Branch("main"))
			.unwrap(),
		None,
		"ten cancelled runs in a row hide the older good one"
	);
	assert_eq!(fake.requests().len(), 1);
}

#[test]
fn a_listing_that_is_not_a_list_is_an_error_and_junk_in_it_is_skipped() {
	let (fake, gh) = world();
	fake.fail(
		Method::Get,
		"actions/workflows",
		1,
		Fault::Status {
			status: 200,
			retry_after_s: None,
			rate_remaining: None,
			rate_reset: None,
			body: "{\"workflow_runs\":{}}".to_owned(),
		},
	);
	assert_eq!(
		gh.baseline_run(WORKFLOW, &BaselineQuery::Branch("main")),
		Err(ForgeError::Decode("a list was expected"))
	);
	fake.fail(
		Method::Get,
		"actions/workflows",
		1,
		Fault::Status {
			status: 200,
			retry_after_s: None,
			rate_remaining: None,
			rate_reset: None,
			body: "{\"workflow_runs\":[null,7,{\"id\":\"x\"}]}".to_owned(),
		},
	);
	assert_eq!(
		gh.baseline_run(WORKFLOW, &BaselineQuery::Branch("main"))
			.unwrap(),
		None
	);
}

#[test]
fn a_host_that_cannot_look_for_a_baseline_has_none_and_asks_nothing() {
	let fake = FakeForgejo::new(REPO);
	let host = Forgejo::new(fake.clone(), RepoSlug::parse(REPO).unwrap());
	assert_eq!(
		host.baseline_run(WORKFLOW, &BaselineQuery::Branch("main"))
			.unwrap(),
		None
	);
	assert!(fake.requests().is_empty());
}

#[test]
fn publishing_does_not_look_for_a_baseline_yet() {
	let env = Env::new();
	env.artifact("rust-test-results", &[("results.xml", &common::passing(3))]);
	let report = env.publish();
	assert!(!report.comments.is_empty());
	assert!(
		!env.fake
			.requests()
			.iter()
			.any(|r| r.target().contains("workflows")),
		"no request names a workflow"
	);
}
