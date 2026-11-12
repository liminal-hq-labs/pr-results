// Tests for the publish flow against the in-memory GitHub: the comment, the check runs, pull
// request verification, events, faults and the request goldens.
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

use std::time::Duration;

use common::{
	check_golden, failing, heavy, passing, paths, section, set_tree, zip, Env, BOT, OLD_SHA, REPO,
	RUN, SHA,
};
use pr_results_core::{Marker, RunId};
use pr_results_github::{
	publish, ApiRequest, CommentAction, Failure, FakePull, Fault, ForgeError, Method, Note,
	PublishReport,
};
use pr_results_render::Conclusion;

fn targets(reqs: &[ApiRequest], method: Method) -> Vec<String> {
	reqs.iter()
		.filter(|r| r.method == method)
		.map(ApiRequest::target)
		.collect()
}

fn marker_run(body: &str) -> u64 {
	Marker::parse(body).unwrap().run.0
}

fn env_with_results() -> Env {
	let env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", &passing(3))]);
	env
}

// ---- the sticky comment --------------------------------------------------------------------

#[test]
fn the_comment_is_created_once_then_updated_in_place() {
	let mut env = env_with_results();
	let first = env.publish();
	assert_eq!(first.comments.len(), 1);
	assert_eq!(first.comments[0].action, CommentAction::Created);
	let comments = env.bot_comments(7);
	assert_eq!(comments.len(), 1);
	assert!(comments[0]
		.body
		.starts_with(&format!("<!-- pr-results:1 key=ci sha={SHA} run={RUN} -->")));
	assert!(
		comments[0].body.contains("All 3 passed"),
		"{}",
		comments[0].body
	);

	// A re-run is a later run for the same commit: the one comment is updated in place.
	env.run.id = RunId(RUN + 1);
	env.fake.add_artifact(
		RUN + 1,
		"rust-test-results",
		zip::build(&[zip::Spec::deflated("junit.xml", passing(3).as_bytes())]),
	);
	let second = env.publish();
	assert_eq!(second.comments[0].action, CommentAction::Updated);
	let after = env.bot_comments(7);
	assert_eq!(after.len(), 1);
	assert_eq!(after[0].id, comments[0].id);
	assert_eq!(marker_run(&after[0].body), RUN + 1);
}

#[test]
fn a_second_identical_publish_only_patches() {
	let env = env_with_results();
	env.publish();
	env.fake.clear_requests();
	env.publish();
	let writes = env.fake.writes();
	assert!(!writes.is_empty());
	assert!(
		writes.iter().all(|w| w.method == Method::Patch),
		"{:?}",
		writes
			.iter()
			.map(|w| (w.method, w.target()))
			.collect::<Vec<_>>()
	);
	assert_eq!(env.bot_comments(7).len(), 1);
	assert_eq!(env.fake.check_runs().len(), 1);
}

#[test]
fn a_forged_marker_from_a_user_and_another_key_are_left_alone() {
	let env = env_with_results();
	let forged = format!("<!-- pr-results:1 key=ci sha={SHA} run=99999 -->\nlooks like ours");
	let user = env.fake.add_comment(7, "mallory", &forged);
	let other_key = format!("<!-- pr-results:1 key=e2e sha={SHA} run=1 -->\nthe e2e comment");
	let e2e = env.fake.add_comment(7, BOT, &other_key);
	let report = env.publish();
	assert_eq!(report.comments[0].action, CommentAction::Created);
	let all = env.fake.comments(7);
	assert_eq!(all.len(), 3);
	assert_eq!(all.iter().find(|c| c.id == user).unwrap().body, forged);
	assert_eq!(all.iter().find(|c| c.id == e2e).unwrap().body, other_key);
	let ours = all.iter().find(|c| c.id != user && c.id != e2e).unwrap();
	assert_eq!(ours.login, BOT);
	assert!(ours.body.contains("All 3 passed"));
}

#[test]
fn the_bots_comment_is_found_among_250_on_many_pages() {
	let env = env_with_results();
	env.fake.set_page_size(3);
	for i in 0..120 {
		env.fake.add_comment(7, "someone", &format!("comment {i}"));
	}
	let marker = format!("<!-- pr-results:1 key=ci sha={SHA} run=1 -->\nold");
	let ours = env.fake.add_comment(7, BOT, &marker);
	for i in 0..129 {
		env.fake.add_comment(7, "someone", &format!("later {i}"));
	}
	assert_eq!(env.fake.comments(7).len(), 250);
	let report = env.publish();
	assert_eq!(report.comments[0].action, CommentAction::Updated);
	assert_eq!(report.comments[0].id, ours);
	assert_eq!(env.fake.comments(7).len(), 250, "nothing was added");
	let reads = targets(&env.fake.requests(), Method::Get)
		.into_iter()
		.filter(|t| t.contains("issues/7/comments"))
		.count();
	assert_eq!(reads, 84, "250 comments, three to a page");
}

#[test]
fn more_than_a_hundred_pages_is_an_error_not_a_loop() {
	let env = env_with_results();
	env.fake.set_page_size(1);
	for i in 0..120 {
		env.fake.add_comment(7, "someone", &format!("comment {i}"));
	}
	let report = env.publish();
	assert!(report.failed());
	assert_eq!(
		report.failures,
		vec![Failure::Comment {
			pr: 7,
			status: None
		}]
	);
	assert_eq!(
		env.fake.comments(7).len(),
		120,
		"no comment was written blind"
	);
	assert_eq!(
		env.fake.check_runs().len(),
		1,
		"the check run still went out"
	);
}

#[test]
fn an_older_run_never_overwrites_a_later_one_for_the_same_commit() {
	let mut env = env_with_results();
	// Run 6000 wrote the comment; run 5000 (older, re-run late) must leave it.
	env.run.id = RunId(6000);
	env.fake.add_artifact(
		6000,
		"rust-test-results",
		zip::build(&[zip::Spec::deflated("junit.xml", failing(1, "a").as_bytes())]),
	);
	env.publish();
	let written = env.bot_comments(7);
	env.run.id = RunId(RUN);
	env.fake.clear_requests();
	let report = env.publish();
	assert_eq!(report.comments[0].action, CommentAction::SkippedNewer);
	assert!(report.notes.contains(&Note::CommentSkippedNewer { pr: 7 }));
	assert_eq!(env.bot_comments(7), written);
	assert!(
		targets(&env.fake.writes(), Method::Patch)
			.iter()
			.all(|t| !t.contains("issues/comments")),
		"the comment was not touched"
	);
}

#[test]
fn a_run_for_an_older_commit_does_not_touch_the_comment_of_a_newer_one() {
	let mut env = env_with_results();
	env.publish();
	let written = env.bot_comments(7);
	// The pull request moves on to a new commit while this commit's run is re-run.
	env.fake.move_head(7, OLD_SHA);
	env.run.id = RunId(RUN + 10);
	let report = env.publish();
	assert!(report.comments.is_empty());
	assert!(report.notes.contains(&Note::NoPullRequest));
	assert_eq!(env.bot_comments(7), written);
	assert_eq!(
		report.checks.len(),
		1,
		"the old commit still gets its checks"
	);
	assert!(!report.checks[0].created);
}

#[test]
fn concurrent_creates_converge_on_one_comment_that_says_the_later_run() {
	let env = env_with_results();
	let racer = format!("<!-- pr-results:1 key=ci sha={SHA} run=4000 -->\nfrom the earlier run");
	env.fake.race_on_next_comment_create(&racer);
	let report = env.publish();
	let comments = env.bot_comments(7);
	assert_eq!(comments.len(), 1, "one survives");
	assert_eq!(
		marker_run(&comments[0].body),
		RUN,
		"and it says this run, which is later"
	);
	assert_eq!(report.comments[0].action, CommentAction::Created);
	assert!(report
		.notes
		.contains(&Note::DuplicatesRemoved { pr: 7, count: 1 }));
}

#[test]
fn concurrent_creates_keep_the_later_runs_text_when_it_landed_first() {
	let env = env_with_results();
	let racer = format!("<!-- pr-results:1 key=ci sha={SHA} run=9000 -->\nfrom the later run");
	env.fake.race_on_next_comment_create(&racer);
	env.publish();
	let comments = env.bot_comments(7);
	assert_eq!(comments.len(), 1);
	assert_eq!(
		comments[0].body, racer,
		"the later run's comment stands untouched"
	);
}

#[test]
fn leftover_duplicates_are_removed_on_update() {
	let env = env_with_results();
	let old = format!("<!-- pr-results:1 key=ci sha={SHA} run=1 -->\nold");
	let first = env.fake.add_comment(7, BOT, &old);
	env.fake.add_comment(7, BOT, &old);
	env.fake.add_comment(7, BOT, &old);
	let report = env.publish();
	assert_eq!(report.comments[0].id, first, "the oldest is kept");
	assert_eq!(env.bot_comments(7).len(), 1);
}

#[test]
fn a_comment_deleted_between_listing_and_patching_is_made_again() {
	let env = env_with_results();
	let old = format!("<!-- pr-results:1 key=ci sha={SHA} run=1 -->\nold");
	env.fake.add_comment(7, BOT, &old);
	env.fake
		.fail(Method::Patch, "issues/comments", 1, Fault::status(404));
	let report = env.publish();
	assert!(!report.failed());
	assert_eq!(report.comments[0].action, CommentAction::Created);
	assert!(env
		.bot_comments(7)
		.iter()
		.any(|c| marker_run(&c.body) == RUN));
}

// ---- check runs ----------------------------------------------------------------------------

fn annotated_env(failures: usize) -> Env {
	let env = Env::new();
	env.artifact(
		"rust-test-results",
		&[("junit.xml", &failing(failures, "f"))],
	);
	set_tree(&env.fake, SHA, &paths("f", failures));
	env
}

#[test]
fn annotations_go_fifty_to_a_request() {
	let mut env = annotated_env(120);
	env.plan.options.max_annotations = 500;
	let report = env.publish();
	let check = &env.fake.check_runs()[0];
	assert_eq!(check.batches, vec![50, 50, 20]);
	assert_eq!(check.annotations.len(), 120);
	assert_eq!(
		check.external_id.as_deref(),
		Some("pr-results:1:ci:Example Rust tests")
	);
	assert_eq!(check.conclusion.as_deref(), Some("failure"));
	assert_eq!(report.checks[0].annotations_sent, 120);
	assert_eq!(report.checks[0].annotation_batches, 3);
	let writes = env.fake.writes();
	let check_writes: Vec<&ApiRequest> = writes
		.iter()
		.filter(|w| w.path.as_str().contains("check-runs"))
		.collect();
	assert_eq!(check_writes[0].method, Method::Post);
	assert_eq!(check_writes[1].method, Method::Patch);
	assert_eq!(check_writes[2].method, Method::Patch);
}

#[test]
fn a_cap_of_fifty_says_how_many_were_not_shown() {
	let mut env = annotated_env(120);
	env.plan.options.max_annotations = 50;
	env.publish();
	let check = &env.fake.check_runs()[0];
	assert_eq!(check.batches, vec![50]);
	assert!(check.summary.contains("70 not shown"), "{}", check.summary);
}

#[test]
fn paths_outside_the_tree_are_dropped_and_counted() {
	let mut env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", &failing(10, "f"))]);
	// Only the first six files are in the commit.
	set_tree(&env.fake, SHA, &paths("f", 6));
	env.plan.options.max_annotations = 500;
	let report = env.publish();
	let check = &env.fake.check_runs()[0];
	assert_eq!(check.annotations.len(), 6);
	assert!(
		check.summary.contains("Annotations: 6."),
		"{}",
		check.summary
	);
	assert!(report.notes.contains(&Note::AnnotationsOutsideTree {
		section: 0,
		count: 4
	}));
}

#[test]
fn a_truncated_or_unreadable_tree_checks_nothing_and_says_so() {
	let env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", &failing(2, "f"))]);
	env.fake.set_tree(SHA, &["src/f0.rs"], true);
	let report = env.publish();
	assert!(report.notes.contains(&Note::TreeTruncated));
	assert_eq!(env.fake.check_runs()[0].annotations.len(), 2);

	let env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", &failing(2, "f"))]);
	env.fake.set_reject_unknown_paths(false);
	let report = env.publish();
	assert!(report.notes.contains(&Note::TreeUnavailable));
	assert_eq!(env.fake.check_runs()[0].annotations.len(), 2);
}

#[test]
fn a_refused_annotation_batch_is_left_out_and_noted() {
	// The first batch comes with the create.
	let mut env = annotated_env(120);
	env.plan.options.max_annotations = 500;
	env.fake
		.fail(Method::Post, "check-runs", 1, Fault::status(422));
	let report = env.publish();
	let check = &env.fake.check_runs()[0];
	assert_eq!(
		check.batches,
		vec![50, 20],
		"the first batch was dropped, the rest went"
	);
	assert!(report.notes.contains(&Note::AnnotationsRejected {
		section: 0,
		count: 50
	}));
	assert!(!report.failed());

	// A later batch comes with an update.
	let mut env = annotated_env(120);
	env.plan.options.max_annotations = 500;
	env.fake
		.fail(Method::Patch, "check-runs", 1, Fault::status(422));
	let report = env.publish();
	let check = &env.fake.check_runs()[0];
	assert_eq!(check.batches, vec![50, 20]);
	assert!(report.notes.contains(&Note::AnnotationsRejected {
		section: 0,
		count: 50
	}));
}

#[test]
fn a_check_run_is_found_by_its_external_id_and_updated() {
	let env = annotated_env(2);
	env.publish();
	env.publish();
	let checks = env.fake.check_runs();
	assert_eq!(checks.len(), 1);
	// Annotations append, so the second pass adds the same two again.
	assert_eq!(checks[0].annotations.len(), 4);
	let reads = targets(&env.fake.requests(), Method::Get);
	assert!(reads
		.iter()
		.any(|t| t.contains("check_name=Example%20Rust%20tests") && t.contains("filter=latest")));
}

#[test]
fn a_check_with_our_name_but_not_our_external_id_is_left_alone() {
	let env = env_with_results();
	let plain = env.fake.add_check("Example Rust tests", SHA, None);
	let other_key = env.fake.add_check(
		"Example Rust tests",
		SHA,
		Some("pr-results:1:e2e:Example Rust tests"),
	);
	env.publish();
	let checks = env.fake.check_runs();
	assert_eq!(checks.len(), 3);
	for id in [plain, other_key] {
		let c = checks.iter().find(|c| c.id == id).unwrap();
		assert_eq!(c.title, "other", "the other check was not written to");
	}
	assert!(checks
		.iter()
		.any(|c| c.external_id.as_deref() == Some("pr-results:1:ci:Example Rust tests")));
}

#[test]
fn conclusions_follow_the_results_and_the_required_flag() {
	let mut env = Env::new();
	env.plan
		.sections
		.push(section("E2E", "Example E2E", "e2e-test-results"));
	env.plan
		.sections
		.push(section("Docs", "Example Docs", "docs-test-results"));
	env.plan.sections[2].required = true;
	env.artifact("rust-test-results", &[("junit.xml", &passing(2))]);
	let report = env.publish();
	let got: Vec<Conclusion> = report.checks.iter().map(|c| c.conclusion).collect();
	assert_eq!(
		got,
		[
			Conclusion::Success,
			Conclusion::Neutral,
			Conclusion::Failure
		]
	);
}

// ---- finding the pull request --------------------------------------------------------------

#[test]
fn a_fork_pull_request_is_found_by_its_head_when_the_other_routes_miss() {
	let mut env = Env::new();
	env.fake
		.add_pull(FakePull::fork(9, REPO, "forkuser/app", SHA, "patch-1"));
	env.run.pull_requests = Vec::new();
	env.run.head_repo = pr_results_core::RepoSlug::parse("forkuser/app").unwrap();
	env.run.head_branch = "patch-1".to_owned();
	env.artifact("rust-test-results", &[("junit.xml", &passing(1))]);
	let report = env.publish();
	assert_eq!(report.comments.len(), 1);
	assert_eq!(report.comments[0].pr, 9);
	assert!(env.fake.comments(7).is_empty());
	let reads = targets(&env.fake.requests(), Method::Get);
	let commit = reads
		.iter()
		.position(|t| t.contains(&format!("commits/{SHA}/pulls")))
		.unwrap();
	let head = reads
		.iter()
		.position(|t| t.contains("pulls?head=forkuser%3Apatch-1"))
		.unwrap();
	assert!(commit < head, "the commit listing is tried first");
}

#[test]
fn a_pull_request_the_trusted_event_names_is_still_verified() {
	let env = env_with_results();
	// Closed, from another repository, or moved: none of them verifies.
	env.fake.close_pull(7);
	let report = env.publish();
	assert!(report.notes.contains(&Note::NoPullRequest));
	assert!(report.comments.is_empty());
	assert_eq!(
		env.fake.check_runs().len(),
		1,
		"checks do not need a pull request"
	);

	let env = env_with_results();
	env.fake.move_head(7, OLD_SHA);
	let report = env.publish();
	assert!(report.notes.contains(&Note::NoPullRequest));
	assert!(env.fake.comments(7).is_empty());

	let mut env = env_with_results();
	env.run.pull_requests = vec![404];
	let report = env.publish();
	assert!(report.notes.contains(&Note::NoPullRequest));

	// A pull request into another repository with the same head does not verify.
	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	let mut other = FakePull::same_repo(11, "other/repo", SHA, "feature");
	other.in_commit_listing = true;
	env.fake.add_pull(other);
	env.fake.close_pull(7);
	let report = env.publish();
	assert!(report.notes.contains(&Note::NoPullRequest));
}

#[test]
fn repository_names_are_compared_without_regard_to_case() {
	let mut env = env_with_results();
	env.fake.add_pull(FakePull {
		head_repo: Some("Example/App".to_owned()),
		base_repo: "EXAMPLE/app".to_owned(),
		..FakePull::same_repo(12, REPO, SHA, "feature")
	});
	env.run.pull_requests = vec![12];
	let report = env.publish();
	assert_eq!(
		report.comments.iter().map(|c| c.pr).collect::<Vec<_>>(),
		[12]
	);
}

#[test]
fn a_forged_event_file_cannot_pick_someone_elses_pull_request() {
	let env = env_with_results();
	// Pull request 99 is another contributor's, on another commit.
	env.fake
		.add_pull(FakePull::same_repo(99, REPO, OLD_SHA, "theirs"));
	env.event_file(99);
	let report = env.publish();
	assert_eq!(report.comments.len(), 1);
	assert_eq!(report.comments[0].pr, 7);
	assert!(env.fake.comments(99).is_empty());
}

#[test]
fn two_pull_requests_on_one_head_follow_the_event_file_or_get_both() {
	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	env.fake
		.add_pull(FakePull::same_repo(8, REPO, SHA, "feature"));
	env.event_file(8);
	let report = env.publish();
	assert_eq!(
		report.comments.iter().map(|c| c.pr).collect::<Vec<_>>(),
		[8]
	);

	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	env.fake
		.add_pull(FakePull::same_repo(8, REPO, SHA, "feature"));
	let report = env.publish();
	assert_eq!(
		report.comments.iter().map(|c| c.pr).collect::<Vec<_>>(),
		[7, 8]
	);
	assert!(report.notes.contains(&Note::SeveralPullRequests {
		found: 2,
		commented: 2
	}));

	// An event file that names neither picks neither.
	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	env.fake
		.add_pull(FakePull::same_repo(8, REPO, SHA, "feature"));
	env.event_file(500);
	let report = env.publish();
	assert_eq!(report.comments.len(), 2);

	// Never more than five.
	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	for n in 8..14 {
		env.fake
			.add_pull(FakePull::same_repo(n, REPO, SHA, "feature"));
	}
	let report = env.publish();
	assert_eq!(report.comments.len(), 5);
	assert!(report.notes.contains(&Note::SeveralPullRequests {
		found: 7,
		commented: 5
	}));
}

#[test]
fn an_event_file_that_is_not_json_or_too_large_is_ignored() {
	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	env.fake
		.add_pull(FakePull::same_repo(8, REPO, SHA, "feature"));
	env.artifact("event-file", &[("event.json", "not json")]);
	assert_eq!(env.publish().comments.len(), 2);

	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	env.fake
		.add_pull(FakePull::same_repo(8, REPO, SHA, "feature"));
	let big = format!(
		"{{\"pull_request\":{{\"number\":8}},\"pad\":\"{}\"}}",
		"x".repeat(1_100_000)
	);
	env.artifact("event-file", &[("event.json", &big)]);
	assert_eq!(
		env.publish().comments.len(),
		2,
		"over a mebibyte is not read"
	);
}

#[test]
fn the_pull_request_is_read_again_before_the_comment_is_written() {
	// The head moves between finding the pull request and writing to it.
	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	env.fake
		.after_request(&format!("commits/{SHA}/pulls"), |f| f.move_head(7, OLD_SHA));
	let report = env.publish();
	assert!(report.notes.contains(&Note::PullMoved { pr: 7 }));
	assert!(env.fake.comments(7).is_empty());
	assert_eq!(
		env.fake.check_runs().len(),
		1,
		"the checks hang on the commit"
	);

	// It is closed in that window.
	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	env.fake
		.after_request(&format!("commits/{SHA}/pulls"), |f| f.close_pull(7));
	let report = env.publish();
	assert!(report.notes.contains(&Note::PullClosed { pr: 7 }));
	assert!(env.fake.comments(7).is_empty());

	// It disappears (a deleted repository, a transferred issue).
	let mut env = env_with_results();
	env.run.pull_requests = Vec::new();
	env.fake.fail(Method::Get, "pulls/7", 1, Fault::status(404));
	let report = env.publish();
	assert!(report.notes.contains(&Note::PullGone { pr: 7 }));
}

// ---- events --------------------------------------------------------------------------------

#[test]
fn a_push_gets_check_runs_and_no_comment() {
	let mut env = env_with_results();
	env.run.event = "push".to_owned();
	env.run.pull_requests = Vec::new();
	let report = env.publish();
	assert_eq!(report.checks.len(), 1);
	assert!(report.comments.is_empty());
	assert!(report.notes.contains(&Note::PushNoComment));
	assert!(targets(&env.fake.requests(), Method::Get)
		.iter()
		.all(|t| !t.contains("pulls") && !t.contains("comments")));
}

#[test]
fn a_cancelled_or_skipped_run_publishes_nothing() {
	for conclusion in ["cancelled", "skipped"] {
		let mut env = env_with_results();
		env.run.conclusion = Some(conclusion.to_owned());
		let report = env.publish();
		assert!(report.nothing_to_do);
		assert!(env.fake.requests().is_empty(), "not even a read");
		assert!(!report.failed());
		assert_eq!(report.log_lines().len(), 1);
	}
}

#[test]
fn comments_can_be_switched_off() {
	let mut env = env_with_results();
	env.plan.post_comment = false;
	let report = env.publish();
	assert!(report.notes.contains(&Note::CommentsOff));
	assert!(env.fake.comments(7).is_empty());
	assert_eq!(env.fake.check_runs().len(), 1);
}

// ---- reading artefacts ---------------------------------------------------------------------

#[test]
fn no_artefacts_is_a_comment_that_says_so() {
	let env = Env::new();
	let report = env.publish();
	assert!(!report.failed());
	let body = &env.bot_comments(7)[0].body;
	assert!(
		body.contains("No results were uploaded for this run."),
		"{body}"
	);
	assert_eq!(report.checks[0].conclusion, Conclusion::Neutral);
}

#[test]
fn artefacts_are_matched_by_glob_files_by_glob_and_merged_by_suite_name() {
	let mut env = Env::new();
	env.plan.sections[0].artifact = pr_results_core::Glob::new("*-test-results-*").unwrap();
	env.plan.sections[0].files = vec![pr_results_core::Glob::new("**/junit.xml").unwrap()];
	env.artifact(
		"rust-test-results-linux",
		&[("a/junit.xml", &passing(2)), ("skip.xml", &failing(5, "x"))],
	);
	env.artifact("rust-test-results-mac", &[("junit.xml", &passing(3))]);
	env.artifact("other-artifact", &[("junit.xml", &failing(9, "y"))]);
	env.publish();
	let body = &env.bot_comments(7)[0].body;
	assert!(
		body.contains("All 5 passed"),
		"same-named suites merge: {body}"
	);
}

#[test]
fn one_artefact_feeding_two_sections_is_downloaded_once() {
	let mut env = Env::new();
	env.plan
		.sections
		.push(section("Again", "Example Again", "rust-test-results"));
	env.artifact("rust-test-results", &[("junit.xml", &passing(1))]);
	env.publish();
	let downloads = env.fake.blob_fetches();
	assert_eq!(downloads.len(), 1);
}

#[test]
fn an_expired_a_refused_and_an_unreadable_artefact_are_noted_not_fatal() {
	let env = Env::new();
	let id = env.artifact("rust-test-results", &[("junit.xml", &passing(1))]);
	env.fake.expire_artifact(id);
	let report = env.publish();
	assert!(report.notes.contains(&Note::ArtifactExpired { section: 0 }));
	assert!(!report.failed());
	assert!(env.bot_comments(7)[0]
		.body
		.contains("No results were uploaded"));

	let env = Env::new();
	env.fake.add_artifact(
		RUN,
		"rust-test-results",
		zip::build(&[zip::Spec::stored("../escape.xml", b"<a/>")]),
	);
	let report = env.publish();
	assert!(report.notes.contains(&Note::ArtifactRefused {
		section: 0,
		why: pr_results_github::ArchiveError::BadPath
	}));

	let env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", "<testsuites><oops")]);
	let report = env.publish();
	assert!(report.notes.contains(&Note::InputUnreadable { section: 0 }));
}

#[test]
fn an_artefact_over_the_download_limit_is_not_fetched() {
	let mut env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", &passing(50))]);
	env.plan.max_download_bytes = 100;
	let report = env.publish();
	assert!(report
		.notes
		.contains(&Note::ArtifactTooLarge { section: 0 }));
	assert!(env.fake.blob_fetches().is_empty());
}

// ---- faults and retries --------------------------------------------------------------------

/// The waits of a second or more: the retries and the rate limits, not the gap between writes.
fn seconds(env: &Env) -> Vec<u64> {
	env.fake
		.sleeps()
		.iter()
		.filter(|d| **d >= Duration::from_secs(1))
		.map(Duration::as_secs)
		.collect()
}

fn fails_with(env: &Env) -> ForgeError {
	publish(&env.gh, &env.run, &env.server, &env.plan)
		.unwrap_err()
		.source
}

#[test]
fn a_server_error_is_retried_with_backoff_then_succeeds() {
	let env = env_with_results();
	env.fake
		.fail(Method::Get, "/artifacts", 3, Fault::status(502));
	let report = env.publish();
	assert!(!report.failed());
	assert_eq!(&seconds(&env)[..3], [1, 2, 4]);
}

#[test]
fn a_server_error_that_does_not_clear_gives_up_after_four_waits() {
	let env = env_with_results();
	env.fake
		.fail(Method::Get, "/artifacts", 99, Fault::status(502));
	let err = fails_with(&env);
	assert_eq!(seconds(&env), [1, 2, 4, 8]);
	assert!(matches!(err, ForgeError::Status { status: 502, .. }));
}

#[test]
fn a_dropped_connection_on_a_read_is_retried() {
	let env = env_with_results();
	env.fake.fail(
		Method::Get,
		"/artifacts",
		1,
		Fault::Transport("reset".to_owned()),
	);
	assert!(!env.publish().failed());
	assert_eq!(seconds(&env), [1]);
}

#[test]
fn retry_after_is_honoured_up_to_a_minute() {
	let env = env_with_results();
	env.fake
		.fail(Method::Get, "/artifacts", 1, Fault::retry_after(429, 7));
	env.publish();
	assert_eq!(seconds(&env)[0], 7);

	let env = env_with_results();
	env.fake
		.fail(Method::Get, "/artifacts", 1, Fault::retry_after(403, 20));
	env.publish();
	assert_eq!(
		seconds(&env)[0],
		20,
		"a secondary limit comes as a 403 with retry-after"
	);

	let env = env_with_results();
	env.fake
		.fail(Method::Get, "/artifacts", 1, Fault::retry_after(429, 120));
	assert_eq!(fails_with(&env), ForgeError::RateLimited { wait_s: 120 });
	assert!(seconds(&env).is_empty(), "it did not wait");
}

#[test]
fn a_primary_limit_is_waited_out_only_if_it_resets_within_a_minute() {
	let env = env_with_results();
	let now = 1_700_000_000;
	env.fake.set_now(now);
	env.fake
		.fail(Method::Get, "/artifacts", 1, Fault::primary_limit(now + 30));
	env.publish();
	assert_eq!(seconds(&env)[0], 30);

	let env = env_with_results();
	env.fake.set_now(now);
	env.fake.fail(
		Method::Get,
		"/artifacts",
		1,
		Fault::primary_limit(now + 600),
	);
	assert_eq!(fails_with(&env), ForgeError::RateLimited { wait_s: 600 });
}

#[test]
fn a_permission_error_is_not_retried_and_leaves_the_checks_posted() {
	let env = env_with_results();
	env.fake
		.fail(Method::Post, "issues/7/comments", 9, Fault::status(403));
	let report = env.publish();
	assert!(report.failed());
	assert_eq!(
		report.failures,
		vec![Failure::Comment {
			pr: 7,
			status: Some(403)
		}]
	);
	assert_eq!(env.fake.check_runs().len(), 1);
	let posts = targets(&env.fake.requests(), Method::Post)
		.into_iter()
		.filter(|t| t.contains("comments"))
		.count();
	assert_eq!(posts, 1, "a 403 is not retried");
	assert!(report.log_lines().iter().any(|l| l.contains("status 403")));
}

#[test]
fn a_check_run_that_cannot_be_written_is_a_failure_and_the_comment_still_goes() {
	let env = env_with_results();
	env.fake
		.fail(Method::Post, "check-runs", 9, Fault::status(403));
	let report = env.publish();
	assert_eq!(
		report.failures,
		vec![Failure::Check {
			section: 0,
			status: Some(403)
		}]
	);
	assert_eq!(env.bot_comments(7).len(), 1);
}

#[test]
fn a_post_that_may_have_landed_is_not_repeated_blindly() {
	// The comment was created and the answer was lost: the retry finds it instead of posting again.
	let env = env_with_results();
	env.fake.fail(
		Method::Post,
		"issues/7/comments",
		1,
		Fault::AppliedThenStatus(502),
	);
	let report = env.publish();
	assert!(!report.failed());
	assert_eq!(env.bot_comments(7).len(), 1);
	let posts = targets(&env.fake.requests(), Method::Post)
		.into_iter()
		.filter(|t| t.contains("comments"))
		.count();
	assert_eq!(posts, 1);

	// The same for a check run.
	let env = env_with_results();
	env.fake
		.fail(Method::Post, "check-runs", 1, Fault::AppliedThenStatus(502));
	let report = env.publish();
	assert!(!report.failed());
	assert_eq!(env.fake.check_runs().len(), 1);

	// And when the create did not land, it is made again, once.
	let env = env_with_results();
	env.fake
		.fail(Method::Post, "issues/7/comments", 1, Fault::status(502));
	assert!(!env.publish().failed());
	assert_eq!(env.bot_comments(7).len(), 1);
	let posts = targets(&env.fake.requests(), Method::Post)
		.into_iter()
		.filter(|t| t.contains("comments"))
		.count();
	assert_eq!(posts, 2);
}

#[test]
fn a_dropped_connection_on_a_post_is_not_retried_by_the_transport() {
	let env = env_with_results();
	env.fake.fail(
		Method::Post,
		"check-runs",
		1,
		Fault::Transport("reset".to_owned()),
	);
	let report = env.publish();
	assert!(!report.failed(), "the create is confirmed and repeated");
	assert_eq!(env.fake.check_runs().len(), 1);
}

#[test]
fn writes_are_half_a_second_apart() {
	let env = env_with_results();
	env.publish();
	let half = env
		.fake
		.sleeps()
		.iter()
		.filter(|d| **d == Duration::from_millis(500))
		.count();
	// A check run, then a comment: the second write waits.
	assert!(half >= 1, "{:?}", env.fake.sleeps());
}

#[test]
fn an_oversize_comment_is_retried_once_in_its_minimal_form() {
	let mut env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", &heavy(30, 16_000))]);
	env.plan.options.comment_limit = 5_000_000;
	env.plan.options.max_failures = 100;
	env.plan.options.excerpt_lines = 100_000;
	let report = env.publish();
	assert!(!report.failed(), "{:?}", report.failures);
	assert!(report.notes.contains(&Note::CommentMinimal { pr: 7 }));
	let body = &env.bot_comments(7)[0].body;
	assert!(body.contains("Most of this comment was left out"), "{body}");
	let comment_posts = targets(&env.fake.requests(), Method::Post)
		.into_iter()
		.filter(|t| t.contains("comments"))
		.count();
	assert_eq!(
		comment_posts, 2,
		"the large one was refused, then the small one went"
	);
}

#[test]
fn an_oversize_check_run_is_retried_once_in_its_minimal_form() {
	let mut env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", &heavy(30, 16_000))]);
	env.plan.options.summary_limit = 5_000_000;
	env.plan.options.text_limit = 5_000_000;
	env.plan.options.max_failures = 100;
	env.plan.options.excerpt_lines = 100_000;
	env.plan.post_comment = false;
	let report = env.publish();
	assert!(!report.failed(), "{:?}", report.failures);
	assert!(report.notes.contains(&Note::CheckMinimal { section: 0 }));
	let check = &env.fake.check_runs()[0];
	assert!(check.summary.chars().count() < 65_535);
	assert_eq!(check.conclusion.as_deref(), Some("failure"));
}

// ---- logs and dry runs ---------------------------------------------------------------------

#[test]
fn log_lines_cannot_carry_a_workflow_command() {
	let mut env = Env::new();
	env.plan.sections[0].title = "x\n::set-output name=a::b ::stop-commands::z".to_owned();
	env.artifact("rust-test-results", &[("junit.xml", &failing(1, "f"))]);
	let report = env.publish();
	let lines = report.log_lines();
	assert!(!lines.is_empty());
	for line in &lines {
		assert!(!line.contains('\n') && !line.contains('\r'), "{line:?}");
		assert!(!line.starts_with("::"), "{line:?}");
		assert!(
			!line.contains("::set-output") && !line.contains("::stop-commands"),
			"{line:?}"
		);
	}
}

#[test]
fn a_dry_run_writes_nothing_and_keeps_what_it_would_have_sent() {
	let env = env_with_results();
	let dry = pr_results_github::DryRun::new(env.fake.clone());
	let gh = pr_results_github::GitHub::new(dry.clone(), env.run.head_repo.clone(), BOT);
	let report = publish(&gh, &env.run, &env.server, &env.plan).unwrap();
	assert!(!report.failed());
	assert!(
		env.fake.writes().is_empty(),
		"nothing reached the repository"
	);
	let kept = dry.requests();
	assert_eq!(kept.len(), 2, "a check run and a comment");
	assert_eq!(kept[0].method, Method::Post);
	assert!(dry.requests_json().contains("\"method\": \"POST\""));
	assert!(env.fake.comments(7).is_empty());
}

// ---- the request goldens -------------------------------------------------------------------

#[test]
fn golden_requests_for_a_failing_run_with_annotations() {
	let env = Env::new();
	env.artifact("rust-test-results", &[("junit.xml", &failing(3, "f"))]);
	set_tree(&env.fake, SHA, &paths("f", 3));
	env.event_file(7);
	env.fake.add_comment(7, "alice", "unrelated");
	env.publish();
	check_golden("failing-run", &env.fake.requests());
}

#[test]
fn golden_requests_for_a_fork_with_two_sections_and_no_results() {
	let mut env = Env::new();
	env.fake
		.add_pull(FakePull::fork(9, REPO, "forkuser/app", SHA, "patch-1"));
	env.run.pull_requests = Vec::new();
	env.run.head_repo = pr_results_core::RepoSlug::parse("forkuser/app").unwrap();
	env.run.head_branch = "patch-1".to_owned();
	env.plan
		.sections
		.push(section("E2E", "Example E2E", "e2e-test-results"));
	env.plan.sections[1].required = true;
	env.artifact("rust-test-results", &[("junit.xml", &passing(4))]);
	env.publish();
	check_golden("fork-two-sections", &env.fake.requests());
}

#[test]
fn golden_requests_for_a_second_identical_publish() {
	let env = env_with_results();
	env.publish();
	env.fake.clear_requests();
	env.publish();
	check_golden("second-publish", &env.fake.requests());
}

#[test]
fn the_report_counts_what_was_written() {
	let env = annotated_env(3);
	let report: PublishReport = env.publish();
	assert_eq!(report.checks.len(), 1);
	assert!(report.checks[0].created);
	assert_eq!(report.comments.len(), 1);
	let lines = report.log_lines();
	assert!(lines
		.iter()
		.any(|l| l.starts_with("Check run created for Rust tests: 3 annotations")));
	assert!(lines
		.iter()
		.any(|l| l.starts_with("Comment on pull request 7 created")));
}
