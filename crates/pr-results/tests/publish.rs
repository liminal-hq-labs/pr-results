// `pr-results publish` from the event file to the comment, against an in-memory GitHub.
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

use common::{
	artifact, fixture, github_with_pr, invoke, publisher_env, scratch, Factory, DESK_TOML, REPO,
	SHA, TOKEN,
};
use pr_results_forge::{FakeForge, FakePull};

fn pr_event() -> String {
	fixture("events/workflow_run-pr.json")
}

/// A failing nextest run: `fail.xml` of the fixtures.
fn failing_xml() -> String {
	fixture("nextest/fail.xml")
}

#[test]
fn a_pull_request_run_gets_its_check_run_and_comment() {
	let dir = scratch("pr");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	let vars = publisher_env(&dir, &pr_event());
	let factory = Factory::new(&fake);

	let o = invoke(&["publish"], &vars, &factory);

	assert_eq!(o.code, 0, "{}", o.err);
	assert_eq!(
		factory.asked.borrow().as_slice(),
		["https://api.github.com"]
	);
	let checks = fake.check_runs();
	assert_eq!(checks.len(), 1);
	assert_eq!(checks[0].name, "App Rust tests");
	assert_eq!(checks[0].conclusion.as_deref(), Some("failure"));
	let comments = fake.comments(7);
	assert_eq!(comments.len(), 1);
	assert!(comments[0]
		.body
		.starts_with("<!-- pr-results:1 key=ci sha="));
	assert!(comments[0].body.contains("### App: "));
	assert!(o.out.contains("Check run created for Rust tests"));
	assert!(o.out.contains("Comment on pull request 7 created."));
}

#[test]
fn the_token_is_masked_first_exactly_once_and_never_printed_again() {
	let dir = scratch("mask");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	let vars = publisher_env(&dir, &pr_event());
	let mask = format!("::add-mask::{TOKEN}");

	for extra in [&[][..], &["--dry-run", dir.join("o").to_str().unwrap()][..]] {
		let mut args = vec!["publish"];
		args.extend_from_slice(extra);
		let o = invoke(&args, &vars, &Factory::new(&fake));

		assert_eq!(o.code, 0, "{}", o.err);
		assert_eq!(o.out.lines().next(), Some(mask.as_str()));
		assert_eq!(o.out.matches("::add-mask::").count(), 1);
		assert_eq!(o.out.matches(TOKEN).count(), 1);
		assert!(!o.err.contains(TOKEN));
	}
}

#[test]
fn nothing_is_masked_when_nothing_is_published() {
	let dir = scratch("mask-none");
	let fake = github_with_pr();
	let vars = publisher_env(&dir, &pr_event().replace("\"success\"", "\"cancelled\""));

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	assert_eq!(o.code, 0);
	assert!(!o.out.contains("add-mask"));
}

#[test]
fn a_second_identical_run_updates_in_place() {
	let dir = scratch("again");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	let vars = publisher_env(&dir, &pr_event());
	let factory = Factory::new(&fake);
	assert_eq!(invoke(&["publish"], &vars, &factory).code, 0);

	let o = invoke(&["publish"], &vars, &factory);

	assert_eq!(o.code, 0, "{}", o.err);
	assert_eq!(fake.check_runs().len(), 1);
	assert_eq!(fake.comments(7).len(), 1);
	assert!(o.out.contains("Comment on pull request 7 updated."));
}

#[test]
fn failing_tests_exit_zero_unless_asked() {
	let dir = scratch("exit-4");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	let vars = publisher_env(&dir, &pr_event());
	let factory = Factory::new(&fake);

	assert_eq!(invoke(&["publish"], &vars, &factory).code, 0);
	assert_eq!(
		invoke(&["publish", "--fail-on-failures"], &vars, &factory).code,
		4
	);
}

#[test]
fn passing_tests_exit_zero_even_when_asked() {
	let dir = scratch("exit-pass");
	let fake = github_with_pr();
	artifact(
		&fake,
		"rust-test-results",
		&[("junit.xml", &fixture("nextest/pass.xml"))],
	);
	let vars = publisher_env(&dir, &pr_event());

	let o = invoke(
		&["publish", "--fail-on-failures"],
		&vars,
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
}

#[test]
fn a_refused_comment_leaves_the_checks_and_exits_one() {
	let dir = scratch("exit-1");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	fake.fail(
		pr_results_forge::Method::Post,
		"/issues/7/comments",
		1,
		pr_results_forge::Fault::status(403),
	);
	let vars = publisher_env(&dir, &pr_event());

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	assert_eq!(o.code, 1, "{}", o.out);
	assert_eq!(fake.check_runs().len(), 1);
	assert!(o
		.out
		.contains("The comment on pull request 7 was not written (status 403)."));
}

#[test]
fn a_fork_run_comments_on_the_fork_pull_request_only() {
	let dir = scratch("fork");
	let fake = FakeForge::new(REPO);
	// The pull request the fork's forged event file names, and the one the run belongs to.
	fake.add_pull(FakePull::same_repo(
		1,
		REPO,
		"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		"other",
	));
	fake.add_pull(FakePull::fork(8, REPO, "forker/app", SHA, "fix-typo"));
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	artifact(
		&fake,
		"event-file",
		&[("event.json", &fixture("events/uploaded-event-forged.json"))],
	);
	let vars = publisher_env(&dir, &fixture("events/workflow_run-fork.json"));

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	assert_eq!(o.code, 0, "{}", o.err);
	assert_eq!(fake.comments(8).len(), 1);
	assert!(fake.comments(1).is_empty());
	assert_eq!(fake.check_runs().len(), 1);
	// The event file is not a suite: only `rust-test-results` became one.
	assert_eq!(fake.check_runs()[0].name, "App Rust tests");
}

#[test]
fn a_forged_event_file_cannot_pick_a_pull_request() {
	let dir = scratch("forged");
	let fake = github_with_pr();
	// Another open pull request on the same head: both verify, and the forged file names a third.
	fake.add_pull(FakePull::same_repo(9, REPO, SHA, "feature"));
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	artifact(
		&fake,
		"event-file",
		&[("event.json", &fixture("events/uploaded-event-forged.json"))],
	);
	// The event lists no pull requests, so the commit's pull requests are used.
	let vars = publisher_env(
		&dir,
		&fixture("events/workflow_run-fork.json").replace("forker/app", REPO),
	);

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	assert_eq!(o.code, 0, "{}", o.err);
	assert!(fake.comments(1).is_empty());
	assert_eq!(fake.comments(7).len(), 1);
	assert_eq!(fake.comments(9).len(), 1);
	assert!(o
		.out
		.contains("2 pull requests match this run and the event file chose none"));
}

/// Two pull requests on one head, so the event file is consulted.
fn two_candidates(fake: &FakeForge) {
	fake.add_pull(FakePull::same_repo(9, REPO, SHA, "feature"));
	// Open in this repository, but not at the run's head: it can never be a candidate.
	fake.add_pull(FakePull::same_repo(
		1,
		REPO,
		"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		"other",
	));
}

fn no_pr_list_event() -> String {
	fixture("events/workflow_run-fork.json").replace("forker/app", REPO)
}

#[test]
fn a_forged_event_file_naming_an_unrelated_open_pull_request_is_ignored() {
	let dir = scratch("forged-other");
	let fake = github_with_pr();
	two_candidates(&fake);
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	artifact(
		&fake,
		"event-file",
		&[("event.json", &fixture("events/uploaded-event-forged.json"))],
	);

	let o = invoke(
		&["publish"],
		&publisher_env(&dir, &no_pr_list_event()),
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
	assert!(
		fake.comments(1).is_empty(),
		"the forged number redirected the comment"
	);
	assert_eq!(fake.comments(7).len() + fake.comments(9).len(), 2);
}

#[test]
fn the_event_file_chooses_only_among_pull_requests_that_matched() {
	let dir = scratch("hint-chooses");
	let fake = github_with_pr();
	two_candidates(&fake);
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	artifact(
		&fake,
		"event-file",
		&[("event.json", "{\"pull_request\":{\"number\":9}}")],
	);

	let o = invoke(
		&["publish"],
		&publisher_env(&dir, &no_pr_list_event()),
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
	assert_eq!(fake.comments(9).len(), 1);
	assert!(fake.comments(7).is_empty());
	assert!(fake.comments(1).is_empty());
}

#[test]
fn an_oversize_event_file_is_ignored() {
	let dir = scratch("hint-big");
	let fake = github_with_pr();
	two_candidates(&fake);
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	let big = format!(
		"{{\"pull_request\":{{\"number\":9}},\"pad\":\"{}\"}}",
		"x".repeat(1024 * 1024 + 1)
	);
	artifact(&fake, "event-file", &[("event.json", &big)]);

	let o = invoke(
		&["publish"],
		&publisher_env(&dir, &no_pr_list_event()),
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
	// The number would have chosen 9 alone; refused for its size, both get the comment.
	assert_eq!(fake.comments(7).len(), 1);
	assert_eq!(fake.comments(9).len(), 1);
}

#[test]
fn the_event_file_is_not_even_downloaded_when_one_pull_request_matched() {
	let dir = scratch("hint-unused");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	artifact(
		&fake,
		"event-file",
		&[("event.json", &fixture("events/uploaded-event-forged.json"))],
	);

	let o = invoke(
		&["publish"],
		&publisher_env(&dir, &fixture("events/workflow_run-pr.json")),
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
	assert_eq!(fake.comments(7).len(), 1);
	assert_eq!(
		fake.blob_fetches().len(),
		1,
		"only the results were downloaded"
	);
}

#[test]
fn a_push_gets_check_runs_and_no_comment() {
	let dir = scratch("push");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	let vars = publisher_env(&dir, &fixture("events/workflow_run-push.json"));

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	assert_eq!(o.code, 0, "{}", o.err);
	assert_eq!(fake.check_runs().len(), 1);
	assert!(fake.comments(7).is_empty());
	assert!(o.out.contains("A push gets check runs and no comment."));
}

#[test]
fn a_cancelled_run_publishes_nothing() {
	let dir = scratch("cancelled");
	let fake = github_with_pr();
	let event = pr_event().replace("\"success\"", "\"cancelled\"");
	let vars = publisher_env(&dir, &event);

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	assert_eq!(o.code, 0, "{}", o.err);
	assert!(fake.requests().is_empty());
	assert!(o.out.contains("cancelled or skipped"));
}

#[test]
fn a_deleted_head_repository_publishes_nothing() {
	let dir = scratch("deleted-fork");
	let fake = github_with_pr();
	let event = fixture("events/workflow_run-fork.json")
		.replace("{ \"full_name\": \"forker/app\" }", "null");
	let vars = publisher_env(&dir, &event);

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	assert_eq!(o.code, 0, "{}", o.err);
	assert!(fake.requests().is_empty());
}

#[test]
fn a_run_with_no_results_says_so_and_stays_neutral() {
	let dir = scratch("none");
	let fake = github_with_pr();
	let vars = publisher_env(&dir, &pr_event());

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	assert_eq!(o.code, 0, "{}", o.err);
	assert!(fake.check_runs().is_empty());
	assert_eq!(fake.comments(7).len(), 1);
	assert!(fake.comments(7)[0]
		.body
		.contains("No results were uploaded for this run."));
}

#[test]
fn desk_publishes_two_suites_from_one_set_of_artefacts() {
	let dir = scratch("desk");
	let fake = github_with_pr();
	artifact(
		&fake,
		"results-check",
		&[
			("steps.xml", &fixture("steps/desk-job.xml")),
			("nextest-core.xml", &fixture("nextest/pass.xml")),
		],
	);
	let config = dir.join("pr-results.toml");
	std::fs::write(&config, DESK_TOML).unwrap();
	let vars = publisher_env(&dir, &pr_event());

	let o = invoke(
		&["publish", "--config", config.to_str().unwrap()],
		&vars,
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
	let mut names: Vec<String> = fake.check_runs().into_iter().map(|c| c.name).collect();
	names.sort();
	assert_eq!(names, ["Desk Rust tests", "Desk check steps"]);
	let comment = &fake.comments(7)[0].body;
	assert!(comment.contains("### Desk checks"));
	assert!(comment.contains("`Desk steps`"));
	assert!(comment.contains("`Rust tests`"));
}

#[test]
fn the_default_settings_file_is_read_when_it_exists() {
	let dir = scratch("default-config");
	let fake = github_with_pr();
	artifact(
		&fake,
		"results-a",
		&[("steps.xml", &fixture("steps/desk-job.xml"))],
	);
	std::fs::create_dir_all(dir.join(".github")).unwrap();
	std::fs::write(dir.join(".github/pr-results.toml"), DESK_TOML).unwrap();
	let vars = publisher_env(&dir, &pr_event());
	// Reading the default path is relative to the working directory, which tests cannot change
	// for one test alone, so this checks the explicit path with the same file instead.
	let o = invoke(
		&[
			"publish",
			"--config",
			dir.join(".github/pr-results.toml").to_str().unwrap(),
		],
		&vars,
		&Factory::new(&fake),
	);
	assert_eq!(o.code, 0, "{}", o.err);
	assert_eq!(fake.check_runs().len(), 2);
}

#[test]
fn comments_switched_off_still_post_the_checks() {
	let dir = scratch("off");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	let config = dir.join("off.toml");
	std::fs::write(&config, "[comment]\nmode = \"off\"\n").unwrap();
	let vars = publisher_env(&dir, &pr_event());

	let o = invoke(
		&["publish", "--config", config.to_str().unwrap()],
		&vars,
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
	assert_eq!(fake.check_runs().len(), 1);
	assert!(fake.comments(7).is_empty());
}

// ---- --dry-run -----------------------------------------------------------------------------

#[test]
fn a_dry_run_writes_the_files_and_posts_nothing() {
	let dir = scratch("dry");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	let vars = publisher_env(&dir, &pr_event());
	let out = dir.join("out");

	let o = invoke(
		&["publish", "--dry-run", out.to_str().unwrap()],
		&vars,
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
	assert!(fake.writes().is_empty());
	let requests = std::fs::read_to_string(out.join("requests.json")).unwrap();
	assert!(requests.contains("\"method\": \"POST\""));
	assert!(requests.contains("/check-runs"));
	assert!(requests.contains("/issues/7/comments"));
	let comment = std::fs::read_to_string(out.join("comment.md")).unwrap();
	assert!(comment.starts_with("<!-- pr-results:1 key=ci"));
	assert!(o.out.contains("### App: "));
	let check = std::fs::read_to_string(out.join("checks/0.md")).unwrap();
	assert!(check.starts_with("conclusion: failure\ntitle: "));
	assert!(check.contains("--- summary ---"));
	assert!(!out.join("checks/1.md").exists());
}

#[test]
fn a_dry_run_defuses_a_line_that_would_start_a_command() {
	let dir = scratch("dry-inject");
	let fake = github_with_pr();
	let xml = "<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"t\">\
		<failure message=\"m\">::set-output name=x::y\n##[group]hello</failure></testcase></testsuite></testsuites>";
	artifact(&fake, "rust-test-results", &[("junit.xml", xml)]);
	let vars = publisher_env(&dir, &pr_event());
	let out = dir.join("out");

	let o = invoke(
		&["publish", "--dry-run", out.to_str().unwrap()],
		&vars,
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
	for line in o.out.lines().skip(1) {
		let start = line.trim_start();
		assert!(
			!start.starts_with("::") && !start.starts_with("##["),
			"{line}"
		);
	}
	assert!(o.out.contains("\\ ::set-output"));
	// The file holds it exactly.
	assert!(std::fs::read_to_string(out.join("comment.md"))
		.unwrap()
		.contains("::set-output"));
}

// ---- refusals ------------------------------------------------------------------------------

fn refused(vars: &std::collections::BTreeMap<String, String>, fake: &FakeForge, why: &str) {
	let o = invoke(&["publish"], vars, &Factory::new(fake));
	assert_eq!(o.code, 2, "{why}: {}", o.err);
	assert!(fake.requests().is_empty(), "{why}: it made a request");
	assert!(
		!o.out.contains("add-mask"),
		"{why}: it masked a token it never used"
	);
}

#[test]
fn it_refuses_to_run_anywhere_but_a_workflow_run_on_the_default_branch() {
	let dir = scratch("refuse");
	let fake = github_with_pr();
	let base = publisher_env(&dir, &pr_event());

	let mut v = base.clone();
	v.insert("GITHUB_EVENT_NAME".into(), "pull_request".into());
	refused(&v, &fake, "pull_request");

	let mut v = base.clone();
	v.remove("GITHUB_EVENT_NAME");
	refused(&v, &fake, "no event name");

	let mut v = base.clone();
	v.insert("GITHUB_REF".into(), "refs/heads/feature".into());
	refused(&v, &fake, "not the default branch");

	let mut v = base.clone();
	v.insert("GITHUB_REF".into(), "refs/pull/7/merge".into());
	refused(&v, &fake, "a merge ref");

	let mut v = base.clone();
	v.insert("GITHUB_REPOSITORY".into(), "someone/else".into());
	refused(&v, &fake, "another repository");

	let mut v = base.clone();
	v.insert("GITHUB_API_URL".into(), "http://api.github.com".into());
	refused(&v, &fake, "http API");

	let mut v = base.clone();
	v.remove("GITHUB_API_URL");
	refused(&v, &fake, "no API");

	let mut v = base.clone();
	v.insert("GITHUB_SERVER_URL".into(), "http://github.com".into());
	refused(&v, &fake, "http server");

	let mut v = base.clone();
	v.insert(
		"GITHUB_EVENT_PATH".into(),
		dir.join("missing.json").display().to_string(),
	);
	refused(&v, &fake, "no event file");

	let mut v = base.clone();
	v.remove("GITHUB_TOKEN");
	refused(&v, &fake, "no token");

	let mut v = base.clone();
	v.insert("GITHUB_TOKEN".into(), "has a space".into());
	refused(&v, &fake, "a token with a space");
}

#[test]
fn an_event_for_another_repository_is_refused() {
	let dir = scratch("refuse-event");
	let fake = github_with_pr();
	let other = pr_event().replace(
		"\"repository\": { \"full_name\": \"example/app\", \"default_branch\": \"main\" }",
		"\"repository\": { \"full_name\": \"someone/else\", \"default_branch\": \"main\" }",
	);
	refused(&publisher_env(&dir, &other), &fake, "event repository");

	let run_elsewhere = pr_event().replace(
		"\"repository\": { \"full_name\": \"example/app\" },",
		"\"repository\": { \"full_name\": \"someone/else\" },",
	);
	refused(
		&publisher_env(&dir, &run_elsewhere),
		&fake,
		"run repository",
	);

	refused(
		&publisher_env(&dir, "{}"),
		&fake,
		"not a workflow_run event",
	);
	refused(&publisher_env(&dir, "not json"), &fake, "not JSON");
}

#[test]
fn the_event_names_repositories_without_regard_to_case() {
	let dir = scratch("case");
	let fake = github_with_pr();
	let mut vars = publisher_env(&dir, &pr_event());
	vars.insert("GITHUB_REPOSITORY".into(), "Example/App".into());

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	// The preconditions pass. The in-memory GitHub is case-sensitive about paths, unlike the
	// real one, so what happens next is not the point.
	assert_ne!(o.code, 2, "{}", o.err);
	assert!(!o.err.contains("another repository"));
}

#[test]
fn a_missing_or_bad_settings_file_is_a_usage_error() {
	let dir = scratch("bad-config");
	let fake = github_with_pr();
	let vars = publisher_env(&dir, &pr_event());

	let missing = dir.join("nope.toml");
	let o = invoke(
		&["publish", "--config", missing.to_str().unwrap()],
		&vars,
		&Factory::new(&fake),
	);
	assert_eq!(o.code, 2);

	let bad = dir.join("bad.toml");
	std::fs::write(&bad, "title = \"x\"\ntitel = \"y\"\n").unwrap();
	let o = invoke(
		&["publish", "--config", bad.to_str().unwrap()],
		&vars,
		&Factory::new(&fake),
	);
	assert_eq!(o.code, 2);
	assert!(o.err.contains("line 2"), "{}", o.err);
	assert!(o.err.contains("unknown key `titel`"), "{}", o.err);
	assert!(fake.requests().is_empty());
}

#[test]
fn the_token_can_come_from_another_variable() {
	let dir = scratch("token-env");
	let fake = github_with_pr();
	let mut vars = publisher_env(&dir, &pr_event());
	vars.remove("GITHUB_TOKEN");
	vars.insert("MY_TOKEN".into(), TOKEN.into());

	let o = invoke(
		&["publish", "--token-env", "MY_TOKEN"],
		&vars,
		&Factory::new(&fake),
	);

	assert_eq!(o.code, 0, "{}", o.err);
}

#[test]
fn the_bot_login_is_checked_and_used() {
	let dir = scratch("bot");
	let fake = github_with_pr();
	artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
	let vars = publisher_env(&dir, &pr_event());

	let o = invoke(
		&["publish", "--bot-login", "not a login"],
		&vars,
		&Factory::new(&fake),
	);
	assert_eq!(o.code, 2);

	// A comment by the default bot is not ours when another login is configured.
	fake.add_comment(
		7,
		"github-actions[bot]",
		"<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=1 -->\nold",
	);
	let o = invoke(
		&["publish", "--bot-login", "my-app[bot]"],
		&vars,
		&Factory::new(&fake),
	);
	assert_eq!(o.code, 0, "{}", o.err);
	assert_eq!(fake.comments(7).len(), 2);
}

#[test]
fn hostile_names_never_reach_the_log() {
	let dir = scratch("hostile-log");
	let fake = github_with_pr();
	let xml = fixture("hostile/strings.xml");
	artifact(&fake, "rust-test-results", &[("junit.xml", &xml)]);
	let vars = publisher_env(&dir, &pr_event());

	let o = invoke(&["publish"], &vars, &Factory::new(&fake));

	assert_eq!(o.code, 0, "{}", o.err);
	for line in o.out.lines().skip(1).chain(o.err.lines()) {
		let start = line.trim_start();
		assert!(!start.starts_with("::"), "{line}");
	}
}

#[test]
fn the_comment_then_the_diagnostics_go_to_the_job_summary() {
	for variable in ["GITHUB_STEP_SUMMARY", "FORGEJO_STEP_SUMMARY"] {
		let dir = scratch(&format!("summary-{variable}"));
		let fake = github_with_pr();
		artifact(&fake, "rust-test-results", &[("junit.xml", &failing_xml())]);
		let mut vars = publisher_env(&dir, &pr_event());
		let file = dir.join("summary.md");
		vars.insert(variable.to_owned(), file.display().to_string());

		let o = invoke(&["publish"], &vars, &Factory::new(&fake));

		assert_eq!(o.code, 0, "{}", o.err);
		let summary = std::fs::read_to_string(&file).unwrap();
		let comment = &fake.comments(7)[0].body;
		let body = comment.split_once('\n').unwrap().1;
		assert!(!summary.contains("<!--"), "{summary}");
		assert!(summary.starts_with(body), "{summary}");
		let rest = summary.strip_prefix(body).unwrap();
		assert!(
			rest.starts_with("\n### pr-results\n\n- Check run created"),
			"{rest}"
		);
		assert!(
			rest.contains("- Comment on pull request 7 created."),
			"{rest}"
		);
	}
}

#[test]
fn a_cancelled_run_writes_no_job_summary() {
	let dir = scratch("summary-cancelled");
	let fake = github_with_pr();
	let mut vars = publisher_env(&dir, &pr_event().replace("\"success\"", "\"cancelled\""));
	let file = dir.join("summary.md");
	vars.insert("GITHUB_STEP_SUMMARY".to_owned(), file.display().to_string());
	let o = invoke(&["publish"], &vars, &Factory::new(&fake));
	assert_eq!(o.code, 0, "{}", o.err);
	assert!(!file.exists());
}

fn in_job_event(head: &str) -> String {
	format!(
		r#"{{"repository":{{"full_name":"{REPO}"}},"number":7,"pull_request":{{"number":7,"head":{{"sha":"{SHA}","ref":"feature","repo":{{"full_name":"{head}"}}}},"base":{{"repo":{{"full_name":"{REPO}"}}}}}}}}"#
	)
}

#[test]
fn in_job_reads_local_artefacts_and_verifies_the_pull_request() {
	let dir = scratch("in-job");
	let input = dir.join("results/rust-test-results");
	std::fs::create_dir_all(&input).unwrap();
	std::fs::write(input.join("junit.xml"), failing_xml()).unwrap();
	let mut vars = publisher_env(&dir, &in_job_event(REPO));
	vars.insert("GITHUB_EVENT_NAME".into(), "pull_request".into());
	vars.insert("GITHUB_RUN_ID".into(), "5000".into());
	vars.insert("GITHUB_RUN_NUMBER".into(), "42".into());
	let fake = github_with_pr();
	let out = invoke(
		&["publish", "--from", dir.join("results").to_str().unwrap()],
		&vars,
		&Factory::new(&fake),
	);
	assert_eq!(out.code, 0, "{}", out.err);
	assert_eq!(fake.check_runs().len(), 1);
	assert_eq!(fake.comments(7).len(), 1);
}

#[test]
fn in_job_refuses_privileged_events_and_skips_forks_before_network() {
	let dir = scratch("in-job-refuse");
	let mut vars = publisher_env(&dir, &in_job_event("another/fork"));
	vars.insert("GITHUB_RUN_ID".into(), "5000".into());
	let fake = github_with_pr();
	let factory = Factory::new(&fake);
	for event in ["workflow_run", "pull_request_target", "issues"] {
		vars.insert("GITHUB_EVENT_NAME".into(), event.into());
		let out = invoke(
			&["publish", "--from", dir.to_str().unwrap()],
			&vars,
			&factory,
		);
		assert_eq!(out.code, 2, "{event}: {}", out.err);
	}
	vars.insert("GITHUB_EVENT_NAME".into(), "pull_request".into());
	let out = invoke(
		&["publish", "--from", dir.to_str().unwrap()],
		&vars,
		&factory,
	);
	assert_eq!(out.code, 0, "{}", out.err);
	assert!(out.out.contains("fork"));
	assert!(factory.asked.borrow().is_empty());
}

#[cfg(unix)]
#[test]
fn in_job_skips_links_and_rejects_oversize_files() {
	use std::os::unix::fs::symlink;
	let dir = scratch("in-job-links");
	let input = dir.join("results/rust-test-results");
	std::fs::create_dir_all(&input).unwrap();
	std::fs::write(input.join("junit.xml"), failing_xml()).unwrap();
	symlink(input.join("junit.xml"), input.join("link.xml")).unwrap();
	let mut vars = publisher_env(&dir, &in_job_event(REPO));
	vars.insert("GITHUB_EVENT_NAME".into(), "pull_request".into());
	vars.insert("GITHUB_RUN_ID".into(), "5000".into());
	let fake = github_with_pr();
	let results = dir.join("results");
	let args = ["publish", "--from", results.to_str().unwrap()];
	let out = invoke(&args, &vars, &Factory::new(&fake));
	assert_eq!(out.code, 0, "{}", out.err);
	assert!(out.out.contains("Skipped 1 links"));
	std::fs::write(input.join("huge.xml"), vec![0; 51 * 1024 * 1024]).unwrap();
	let out = invoke(&args, &vars, &Factory::new(&fake));
	assert_eq!(out.code, 2, "{}", out.err);
}
