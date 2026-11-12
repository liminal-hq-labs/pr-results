// Tests for the Forgejo host: statuses, the sticky comment and its identity, pull requests, trees, and the goldens.
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
use std::time::Duration;

use common::forgejo::{inputs, FjEnv, NUMBER, SERVER};
use common::{check_golden, failing, passing, REPO, RUN, SHA};
use pr_results_core::{CommentKey, Marker, RepoSlug, RunId, Sha};
use pr_results_forge::{
	ApiRequest, ApiResponse, BlobUrl, CommentAction, CommentSpec, Failure, FakeForgejo, FakePull,
	Fault, Forge, ForgeError, Forgejo, Host, Method, Note, ResultKind, ResultSpec, StatusSpec,
	Tree, ACTIONS_ID, ACTIONS_LOGIN,
};
use pr_results_render::StatusState;

fn marker(run: u64) -> Marker {
	Marker {
		key: CommentKey::parse("ci").unwrap(),
		sha: Sha::parse(SHA).unwrap(),
		run: RunId(run),
	}
}

fn repo() -> RepoSlug {
	RepoSlug::parse(REPO).unwrap()
}

fn targets(reqs: &[ApiRequest], method: Method) -> Vec<String> {
	reqs.iter()
		.filter(|r| r.method == method)
		.map(ApiRequest::target)
		.collect()
}

// ---- statuses ------------------------------------------------------------------------------

#[test]
fn a_section_is_one_status_whose_context_is_its_check_name() {
	let mut env = FjEnv::new(&passing(3));
	env.plan
		.sections
		.push(common::section("E2E", "Example E2E", "e2e-test-results"));
	env.inputs = inputs(&[&passing(3), &failing(1, "f")]);
	let report = env.publish();
	assert!(!report.failed(), "{:?}", report.log_lines());
	assert_eq!(report.results, ResultKind::CommitStatus);

	let rows = env.fake.latest_statuses(SHA);
	assert_eq!(rows.len(), 2);
	let by = |c: &str| rows.iter().find(|r| r.context == c).unwrap();
	assert_eq!(by("Example Rust tests").state, "success");
	assert_eq!(by("Example Rust tests").description, "3 passed");
	assert_eq!(by("Example E2E").state, "failure");
	assert_eq!(by("Example E2E").description, "1 failed");
	for r in &rows {
		assert_eq!(
			(r.creator_login.as_str(), r.creator_id),
			(ACTIONS_LOGIN, ACTIONS_ID)
		);
		assert!(r.description.chars().count() <= 140);
	}
}

#[test]
fn a_status_links_to_the_comment_and_without_one_to_the_run() {
	let env = FjEnv::new(&passing(3));
	env.publish();
	let comment = env.ours();
	assert_eq!(comment.len(), 1);
	assert_eq!(
		env.fake.statuses(SHA)[0].target_url,
		format!("{SERVER}/{REPO}/pulls/7#issuecomment-{}", comment[0].id)
	);

	let mut push = FjEnv::new(&passing(3));
	push.run.event = "push".to_owned();
	let report = push.publish();
	assert!(report.notes.contains(&Note::PushNoComment));
	assert!(push.ours().is_empty());
	assert_eq!(
		push.fake.statuses(SHA)[0].target_url,
		format!("{SERVER}/{REPO}/actions/runs/{NUMBER}"),
		"the run's page is named by its number on Forgejo"
	);
}

#[test]
fn a_neutral_section_is_a_success_status() {
	let mut env = FjEnv::new(&passing(1));
	env.inputs = pr_results_forge::Inputs {
		suites: vec![Vec::new()],
		skipped: vec![0],
	};
	env.publish();
	let row = &env.fake.latest_statuses(SHA)[0];
	assert_eq!(row.state, "success");
	assert_eq!(row.description, "No results were uploaded");
}

#[test]
fn a_second_publish_supersedes_the_statuses_and_updates_the_comment_in_place() {
	let mut env = FjEnv::new(&passing(3));
	env.publish();
	let first = env.ours()[0].id;
	env.run.id = RunId(RUN + 1);
	env.inputs = inputs(&[&failing(1, "f")]);
	let report = env.publish();
	assert_eq!(report.comments[0].action, CommentAction::Updated);
	assert_eq!(env.ours().len(), 1);
	assert_eq!(env.ours()[0].id, first);
	assert_eq!(env.fake.statuses(SHA).len(), 2, "each publish adds a row");
	let latest = env.fake.latest_statuses(SHA);
	assert_eq!(latest.len(), 1, "and the later one supersedes the earlier");
	assert_eq!(latest[0].state, "failure");
}

#[test]
fn a_status_post_that_was_applied_is_not_repeated() {
	let env = FjEnv::new(&passing(3));
	env.fake
		.fail(Method::Post, "/statuses/", 1, Fault::AppliedThenStatus(502));
	let report = env.publish();
	assert!(!report.failed(), "{:?}", report.log_lines());
	assert_eq!(env.fake.statuses(SHA).len(), 1);
	let reqs = env.fake.requests();
	assert_eq!(
		targets(&reqs, Method::Post)
			.iter()
			.filter(|t| t.contains("/statuses/"))
			.count(),
		1
	);
	assert!(
		targets(&reqs, Method::Get)
			.iter()
			.any(|t| t.contains(&format!("/commits/{SHA}/statuses"))),
		"it looked before repeating"
	);
}

#[test]
fn a_status_post_that_was_not_applied_is_repeated_after_looking() {
	let env = FjEnv::new(&passing(3));
	env.fake
		.fail(Method::Post, "/statuses/", 1, Fault::status(500));
	let report = env.publish();
	assert!(!report.failed(), "{:?}", report.log_lines());
	assert_eq!(env.fake.statuses(SHA).len(), 1);
	let posts = targets(&env.fake.requests(), Method::Post)
		.into_iter()
		.filter(|t| t.contains("/statuses/"))
		.count();
	assert_eq!(posts, 2);
}

#[test]
fn another_accounts_identical_status_does_not_stand_in_for_ours() {
	let env = FjEnv::new(&passing(3));
	// The same context, state and description, written by an account that is not ours (a
	// `forgejo-actions` with a positive id, and a person).
	env.fake.add_status(
		SHA,
		"Example Rust tests",
		"success",
		"3 passed",
		(ACTIONS_LOGIN, 9),
	);
	env.fake.add_status(
		SHA,
		"Example Rust tests",
		"success",
		"3 passed",
		("mallory", 1001),
	);
	env.fake
		.fail(Method::Post, "/statuses/", 1, Fault::status(500));
	let report = env.publish();
	assert!(!report.failed(), "{:?}", report.log_lines());
	let mine: Vec<_> = env
		.fake
		.statuses(SHA)
		.into_iter()
		.filter(|s| s.creator_id == ACTIONS_ID)
		.collect();
	assert_eq!(
		mine.len(),
		1,
		"ours was written after the others were passed over"
	);
}

#[test]
fn a_status_that_keeps_failing_is_reported_and_exits_one() {
	let env = FjEnv::new(&passing(3));
	env.fake
		.fail(Method::Post, "/statuses/", 10, Fault::status(500));
	let report = env.publish();
	assert!(report.failed());
	assert_eq!(
		report.failures,
		vec![Failure::Check {
			section: 0,
			status: Some(502)
		}]
	);
	assert!(
		report
			.log_lines()
			.iter()
			.any(|l| l.starts_with("The commit status for Rust tests was not written")),
		"{:?}",
		report.log_lines()
	);
	// The comment was still written.
	assert_eq!(env.ours().len(), 1);
}

#[test]
fn a_refused_status_is_a_failure_and_not_a_retry() {
	let env = FjEnv::new(&passing(3));
	env.fake
		.fail(Method::Post, "/statuses/", 1, Fault::status(403));
	let report = env.publish();
	assert_eq!(
		report.failures,
		vec![Failure::Check {
			section: 0,
			status: Some(403)
		}]
	);
	assert_eq!(env.fake.statuses(SHA).len(), 0);
}

// ---- the sticky comment and its identity -----------------------------------------------------

#[test]
fn forgejo_actions_with_a_positive_id_is_not_ours() {
	let env = FjEnv::new(&passing(3));
	let body = format!("{}\nnot from the job token", marker(5).render());
	let impostor = env.fake.add_comment_as(7, ACTIONS_LOGIN, 9, &body);
	let report = env.publish();
	assert_eq!(report.comments[0].action, CommentAction::Created);
	assert_ne!(report.comments[0].id, impostor);
	let all = env.fake.comments(7);
	assert_eq!(all.len(), 2);
	assert_eq!(all.iter().find(|c| c.id == impostor).unwrap().body, body);
}

#[test]
fn a_negative_id_under_another_login_is_not_ours_either() {
	let env = FjEnv::new(&passing(3));
	let body = format!("{}\nanother system account", marker(5).render());
	let other = env.fake.add_comment_as(7, "actor", -3, &body);
	let report = env.publish();
	assert_eq!(report.comments[0].action, CommentAction::Created);
	assert_eq!(
		env.fake
			.comments(7)
			.iter()
			.find(|c| c.id == other)
			.unwrap()
			.body,
		body
	);
}

#[test]
fn the_job_token_comment_is_found_among_250_in_one_response() {
	let env = FjEnv::new(&passing(3));
	for i in 0..125 {
		env.fake.add_comment(7, "someone", &format!("c{i}"));
	}
	let mine = env
		.fake
		.add_comment(7, ACTIONS_LOGIN, &format!("{}\nold", marker(1).render()));
	for i in 0..124 {
		env.fake.add_comment(7, "someone", &format!("l{i}"));
	}
	let report = env.publish();
	assert_eq!(report.comments[0].id, mine);
	let lists = targets(&env.fake.requests(), Method::Get)
		.into_iter()
		.filter(|t| t.contains("/issues/7/comments"))
		.collect::<Vec<_>>();
	assert_eq!(lists.len(), 1, "the list is not paged: {lists:?}");
	assert!(!lists[0].contains("page") && !lists[0].contains("limit"));
}

/// A forge that skews `X-Total-Count` on the routes that contain `route`.
struct Skew {
	inner: FakeForgejo,
	route: &'static str,
	delta: i64,
	drop_header: bool,
}

impl Forge for Skew {
	fn send(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		let mut resp = self.inner.send(req)?;
		if req.target().contains(self.route) {
			resp.total_count = if self.drop_header {
				None
			} else {
				resp.total_count
					.map(|t| u64::try_from(i64::try_from(t).unwrap() + self.delta).unwrap())
			};
		}
		Ok(resp)
	}

	fn fetch_blob(&self, url: &BlobUrl, max: u64) -> Result<Vec<u8>, ForgeError> {
		self.inner.fetch_blob(url, max)
	}

	fn sleep(&self, d: Duration) {
		self.inner.sleep(d);
	}

	fn now_epoch(&self) -> u64 {
		self.inner.now_epoch()
	}
}

fn skewed(route: &'static str, delta: i64, drop_header: bool) -> (FakeForgejo, Forgejo<Skew>) {
	let fake = FakeForgejo::new(REPO);
	fake.add_pull(FakePull::same_repo(7, REPO, SHA, "feature"));
	let host = Forgejo::new(
		Skew {
			inner: fake.clone(),
			route,
			delta,
			drop_header,
		},
		repo(),
	);
	(fake, host)
}

#[test]
fn a_comment_list_that_disagrees_with_its_total_count_is_an_error() {
	for delta in [1, -1] {
		let (fake, host) = skewed("/comments", delta, false);
		fake.add_comment(7, "someone", "one");
		fake.add_comment(7, ACTIONS_LOGIN, "two");
		let err = host.comments(7).unwrap_err();
		assert_eq!(
			err,
			ForgeError::Decode("a list does not match its X-Total-Count"),
			"delta {delta}"
		);
		let m = marker(1);
		let body = m.render().as_str().to_owned();
		let r = host.upsert_comment(&CommentSpec {
			pr: 7,
			marker: &m,
			body: &body,
		});
		assert!(
			r.is_err(),
			"nothing is written on a list that cannot be trusted"
		);
		assert!(fake.writes().is_empty());
	}
}

#[test]
fn a_missing_total_count_is_taken_as_the_list_it_is() {
	let (fake, host) = skewed("/comments", 0, true);
	fake.add_comment(7, "someone", "one");
	assert_eq!(host.comments(7).unwrap().len(), 1);
}

#[test]
fn a_paged_list_shorter_than_its_total_count_is_an_error() {
	let (fake, host) = skewed("/statuses", 1, false);
	fake.add_status(
		SHA,
		"Example Rust tests",
		"success",
		"3 passed",
		(ACTIONS_LOGIN, ACTIONS_ID),
	);
	let sha = Sha::parse(SHA).unwrap();
	// The look-before-retry listing is the only reader of statuses: a failed write whose
	// follow-up list is short is an error, not "nothing there, write again".
	fake.fail(Method::Post, "/statuses/", 1, Fault::status(500));
	let url = format!("{SERVER}/x");
	let r = host.upsert_result(&ResultSpec::Status(StatusSpec {
		context: "Example Rust tests",
		head_sha: &sha,
		state: StatusState::Success,
		description: "3 passed",
		target_url: &url,
	}));
	assert_eq!(
		r.unwrap_err(),
		ForgeError::Decode("a list is shorter than its X-Total-Count")
	);
}

#[test]
fn a_comment_by_a_personal_token_without_a_bot_login_is_removed_and_refused() {
	let env = FjEnv::new(&passing(3));
	// The token is a person's, not the job's: its comments are not by `forgejo-actions`.
	env.fake.set_token_user("ci-bot", 7);
	let report = env.publish();
	assert!(report.failed());
	assert_eq!(
		report.failures[0],
		Failure::Comment {
			pr: 7,
			status: None
		}
	);
	assert!(
		env.fake.comments(7).is_empty(),
		"the stray comment was removed"
	);
}

#[test]
fn a_bot_account_is_known_by_login_and_id_from_get_user() {
	let fake = FakeForgejo::new(REPO);
	fake.add_pull(FakePull::same_repo(7, REPO, SHA, "feature"));
	fake.set_token_user("ci-bot", 7);
	let host = Forgejo::new(fake.clone(), repo()).with_account("CI-Bot");
	assert_eq!(host.identity().unwrap(), ("ci-bot".to_owned(), 7));
	assert_eq!(host.identity().unwrap().1, 7);
	let users = targets(&fake.requests(), Method::Get)
		.iter()
		.filter(|t| *t == "/user")
		.count();
	assert_eq!(users, 1, "asked once");

	let mine = fake.add_comment_as(7, "ci-bot", 7, &format!("{}\nold", marker(1).render()));
	let namesake = fake.add_comment_as(
		7,
		"ci-bot",
		99,
		&format!("{}\nsame login, other id", marker(1).render()),
	);
	let m = marker(5);
	let body = format!("{}\nnew", m.render());
	let r = host
		.upsert_comment(&CommentSpec {
			pr: 7,
			marker: &m,
			body: &body,
		})
		.unwrap();
	assert_eq!(r.id, mine);
	assert_eq!(r.action, CommentAction::Updated);
	assert_eq!(r.duplicates_removed, 0);
	assert!(fake
		.comments(7)
		.iter()
		.any(|c| c.id == namesake && c.body.contains("same login")));
}

#[test]
fn a_token_that_belongs_to_another_account_is_refused() {
	let fake = FakeForgejo::new(REPO);
	fake.set_token_user("someone-else", 12);
	let host = Forgejo::new(fake.clone(), repo()).with_account("ci-bot");
	assert!(matches!(host.identity(), Err(ForgeError::Refused(_))));
	let m = marker(1);
	let body = m.render().as_str().to_owned();
	assert!(host
		.upsert_comment(&CommentSpec {
			pr: 7,
			marker: &m,
			body: &body
		})
		.is_err());
	assert!(fake.writes().is_empty());
}

// ---- pull requests and trees -----------------------------------------------------------------

#[test]
fn a_pull_request_is_read_by_number_and_by_branch() {
	let env = FjEnv::new(&passing(1));
	env.fake
		.add_pull(FakePull::fork(9, REPO, "forkuser/app", SHA, "patch-1"));
	let p = env.host.pull(7).unwrap().unwrap();
	assert!(p.open);
	assert_eq!(p.head_sha, SHA);
	assert_eq!(p.head_repo.as_deref(), Some(REPO));
	assert_eq!(p.base_repo.as_deref(), Some(REPO));
	assert!(env.host.pull(404).unwrap().is_none());

	let by_branch = env.host.pulls_by_head("forkuser", "patch-1").unwrap();
	assert_eq!(
		by_branch.iter().map(|p| p.number).collect::<Vec<_>>(),
		vec![9]
	);
	assert_eq!(by_branch[0].head_repo.as_deref(), Some("forkuser/app"));
	// Forgejo filters by branch name alone: another owner's branch of that name is not returned.
	assert!(env
		.host
		.pulls_by_head("someone-else", "patch-1")
		.unwrap()
		.is_empty());
	let targets = targets(&env.fake.requests(), Method::Get);
	assert!(targets
		.iter()
		.any(|t| t.starts_with("/repos/example/app/pulls?")
			&& t.contains("state=open")
			&& t.contains("head=patch-1")
			&& t.contains("limit=50")));
}

#[test]
fn the_commit_route_is_singular_and_a_404_is_none() {
	let env = FjEnv::new(&passing(1));
	let sha = Sha::parse(SHA).unwrap();
	assert!(env.host.pulls_for_commit(&sha).unwrap().is_empty());
	assert_eq!(
		targets(&env.fake.requests(), Method::Get),
		vec![format!("/repos/example/app/commits/{SHA}/pull")]
	);
}

#[test]
fn a_pull_request_found_only_by_branch_still_gets_its_comment() {
	let mut env = FjEnv::new(&passing(3));
	env.run.pull_requests = Vec::new();
	let report = env.publish();
	assert_eq!(report.comments.len(), 1, "{:?}", report.log_lines());
	assert_eq!(report.comments[0].pr, 7);
}

fn tree_of(n: usize) -> Vec<String> {
	(0..n)
		.map(|i| format!("src/dir{}/file{i}.rs", i % 7))
		.collect()
}

#[test]
fn a_tree_of_2500_paths_comes_in_three_pages() {
	let env = FjEnv::new(&passing(1));
	let paths = tree_of(2500);
	let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
	env.fake.set_tree(SHA, &refs, false);
	let tree = env.host.tree_paths(&Sha::parse(SHA).unwrap()).unwrap();
	assert_eq!(tree.paths.len(), 2500);
	assert!(!tree.truncated);
	assert_eq!(tree.paths, paths.iter().cloned().collect::<BTreeSet<_>>());
	let gets = targets(&env.fake.requests(), Method::Get);
	assert_eq!(gets.len(), 3, "{gets:?}");
	for (i, t) in gets.iter().enumerate() {
		assert!(
			t.contains("recursive=true") && t.contains("per_page=1000"),
			"{t}"
		);
		assert!(t.ends_with(&format!("page={}", i + 1)), "{t}");
	}
}

#[test]
fn a_tree_of_two_full_pages_stops_at_its_total_and_a_short_one_takes_a_single_page() {
	let env = FjEnv::new(&passing(1));
	let sha = Sha::parse(SHA).unwrap();
	let paths = tree_of(2000);
	env.fake.set_tree(
		SHA,
		&paths.iter().map(String::as_str).collect::<Vec<_>>(),
		false,
	);
	assert_eq!(env.host.tree_paths(&sha).unwrap().paths.len(), 2000);
	assert_eq!(targets(&env.fake.requests(), Method::Get).len(), 2);

	env.fake.clear_requests();
	let few = tree_of(5);
	env.fake.set_tree(
		SHA,
		&few.iter().map(String::as_str).collect::<Vec<_>>(),
		false,
	);
	assert_eq!(env.host.tree_paths(&sha).unwrap().paths.len(), 5);
	assert_eq!(targets(&env.fake.requests(), Method::Get).len(), 1);
}

#[test]
fn a_server_with_a_smaller_page_is_followed_and_a_runaway_tree_is_truncated() {
	let env = FjEnv::new(&passing(1));
	let sha = Sha::parse(SHA).unwrap();
	let paths = tree_of(250);
	let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
	env.fake.set_tree(SHA, &refs, false);
	env.fake.set_tree_page_cap(100);
	assert_eq!(env.host.tree_paths(&sha).unwrap().paths.len(), 250);
	assert_eq!(targets(&env.fake.requests(), Method::Get).len(), 3);

	// More than 100 pages are never read: the tree is said to be truncated.
	env.fake.clear_requests();
	env.fake.set_tree_page_cap(1);
	let tree: Tree = env.host.tree_paths(&sha).unwrap();
	assert!(tree.truncated);
	assert_eq!(tree.paths.len(), 100);
	assert_eq!(targets(&env.fake.requests(), Method::Get).len(), 100);
}

#[test]
fn an_unknown_commit_is_an_error() {
	let env = FjEnv::new(&passing(1));
	assert_eq!(
		env.host
			.tree_paths(&Sha::parse(SHA).unwrap())
			.unwrap_err()
			.status(),
		Some(404)
	);
}

// ---- what a host refuses -----------------------------------------------------------------------

#[test]
fn the_host_refuses_what_belongs_to_the_other_forge() {
	let env = FjEnv::new(&passing(1));
	assert!(matches!(
		env.host.artifacts(RunId(1)),
		Err(ForgeError::Refused(_))
	));
	let sha = Sha::parse(SHA).unwrap();
	let out = pr_results_render::CheckOutput {
		conclusion: pr_results_render::Conclusion::Success,
		title: pr_results_core::Safe::plain_text("t", 10),
		summary: pr_results_core::Safe::plain_text("s", 10),
		text: pr_results_core::Safe::plain_text("x", 10),
		annotations: Vec::new(),
		annotations_capped: 0,
	};
	let check = pr_results_forge::CheckSpec {
		name: "n",
		head_sha: &sha,
		external_id: "e",
		output: &out,
	};
	assert!(matches!(
		env.host.upsert_result(&ResultSpec::Check(check)),
		Err(ForgeError::Refused(_))
	));
	let gh = pr_results_forge::GitHub::new(
		pr_results_forge::FakeForge::new(REPO),
		repo(),
		pr_results_forge::DEFAULT_BOT_LOGIN,
	);
	let status = StatusSpec {
		context: "c",
		head_sha: &sha,
		state: StatusState::Success,
		description: "d",
		target_url: "u",
	};
	assert!(matches!(
		gh.upsert_result(&ResultSpec::Status(status)),
		Err(ForgeError::Refused(_))
	));
}

#[test]
fn nothing_but_the_job_tokens_calls_are_made_and_the_token_has_no_route_here() {
	// Forgejo's publish makes no tree request (there are no annotations) and no artefact request.
	let env = FjEnv::new(&failing(2, "f"));
	env.publish();
	let all: Vec<String> = env.fake.requests().iter().map(ApiRequest::target).collect();
	assert!(
		all.iter()
			.all(|t| !t.contains("git/trees") && !t.contains("artifacts")),
		"{all:?}"
	);
}

// ---- the request goldens ----------------------------------------------------------------------

#[test]
fn golden_requests_for_a_pull_request() {
	let env = FjEnv::new(&failing(2, "f"));
	env.fake.add_comment(7, "alice", "unrelated");
	env.publish();
	check_golden("forgejo-pr", &env.fake.requests());
}

#[test]
fn golden_requests_for_a_second_publish() {
	let mut env = FjEnv::new(&passing(3));
	env.publish();
	env.fake.clear_requests();
	env.run.id = RunId(RUN + 1);
	env.publish();
	check_golden("forgejo-second-publish", &env.fake.requests());
}

#[test]
fn golden_requests_for_a_push() {
	let mut env = FjEnv::new(&passing(3));
	env.run.event = "push".to_owned();
	env.run.pull_requests = Vec::new();
	env.publish();
	check_golden("forgejo-push", &env.fake.requests());
}

#[test]
fn golden_requests_beside_an_impostor() {
	let env = FjEnv::new(&passing(3));
	let body = format!("{}\nforged", marker(99).render());
	env.fake.add_comment_as(7, ACTIONS_LOGIN, 9, &body);
	env.fake.add_comment_as(7, "mallory", 1001, &body);
	env.publish();
	check_golden("forgejo-impostor", &env.fake.requests());
}
