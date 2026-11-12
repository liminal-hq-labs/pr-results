// Shared test support: a fake GitHub with a pull request, artefacts to put in it and a plan.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

pub mod conformance;
pub mod zip;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use pr_results_core::{CommentKey, Flavour, Glob, RepoSlug, RunId, ServerUrl, Sha};
use pr_results_forge::{
	publish, ApiRequest, FakeForge, FakePull, GitHub, Plan, PublishReport, SectionPlan,
	WorkflowRun, DEFAULT_BOT_LOGIN,
};

pub const REPO: &str = "example/app";
pub const SHA: &str = "3f9c2e1d4b5a69788796a5b4c3d2e1f001122334";
pub const OLD_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const RUN: u64 = 5000;
pub const BOT: &str = DEFAULT_BOT_LOGIN;

pub struct Env {
	pub fake: FakeForge,
	pub gh: GitHub<FakeForge>,
	pub run: WorkflowRun,
	pub plan: Plan,
	pub server: ServerUrl,
}

impl Env {
	/// Pull request 7 from a branch of the repository itself, and a plan with one section that
	/// reads `**/*.xml` from `rust-test-results` as generic JUnit.
	pub fn new() -> Self {
		let fake = FakeForge::new(REPO);
		fake.add_pull(FakePull::same_repo(7, REPO, SHA, "feature"));
		let gh = GitHub::new(fake.clone(), RepoSlug::parse(REPO).unwrap(), BOT);
		let run = WorkflowRun {
			id: RunId(RUN),
			head_sha: Sha::parse(SHA).unwrap(),
			head_branch: "feature".to_owned(),
			head_repo: RepoSlug::parse(REPO).unwrap(),
			event: "pull_request".to_owned(),
			conclusion: Some("success".to_owned()),
			pull_requests: vec![7],
		};
		let plan = Plan::new(
			"Example checks",
			CommentKey::parse("ci").unwrap(),
			vec![section(
				"Rust tests",
				"Example Rust tests",
				"rust-test-results",
			)],
			"0.1.0",
		);
		Self {
			fake,
			gh,
			run,
			plan,
			server: ServerUrl::parse("https://github.com").unwrap(),
		}
	}

	pub fn publish(&self) -> PublishReport {
		publish(&self.gh, &self.run, &self.server, &self.plan).unwrap()
	}

	/// Puts a zip with `files` under `name` in this run's artefacts.
	pub fn artifact(&self, name: &str, files: &[(&str, &str)]) -> u64 {
		let specs: Vec<zip::Spec> = files
			.iter()
			.map(|(n, c)| zip::Spec::deflated(n, c.as_bytes()))
			.collect();
		self.fake.add_artifact(RUN, name, zip::build(&specs))
	}

	/// The same, with the pull request's `event.json` naming `number`.
	pub fn event_file(&self, number: u64) {
		self.artifact(
			"event-file",
			&[(
				"event.json",
				&format!("{{\"pull_request\":{{\"number\":{number}}}}}"),
			)],
		);
	}

	pub fn bot_comments(&self, pr: u64) -> Vec<pr_results_forge::FakeComment> {
		self.fake
			.comments(pr)
			.into_iter()
			.filter(|c| c.login == BOT)
			.collect()
	}
}

pub fn section(title: &str, check: &str, artifact: &str) -> SectionPlan {
	SectionPlan {
		title: title.to_owned(),
		check_name: check.to_owned(),
		required: false,
		artifact: Glob::new(artifact).unwrap(),
		files: vec![Glob::new("**/*.xml").unwrap()],
		flavour: Flavour::Generic,
	}
}

/// `n` passing tests.
pub fn passing(n: usize) -> String {
	let mut out = String::from("<testsuites><testsuite name=\"suite\">");
	for i in 0..n {
		let _ = write!(
			out,
			"<testcase classname=\"c\" name=\"t{i}\" time=\"0.1\"/>"
		);
	}
	out.push_str("</testsuite></testsuites>");
	out
}

/// `n` failing tests, each in its own file `src/<prefix><i>.rs`.
pub fn failing(n: usize, prefix: &str) -> String {
	let mut out = String::from("<testsuites><testsuite name=\"suite\">");
	for i in 0..n {
		let _ = write!(
			out,
			"<testcase classname=\"c\" name=\"t{i}\" file=\"src/{prefix}{i}.rs\" line=\"3\" time=\"0.1\">\
			 <failure message=\"boom {i}\">detail {i}</failure></testcase>"
		);
	}
	out.push_str("</testsuite></testsuites>");
	out
}

/// `n` failing tests whose bodies are `bytes` bytes each, to make a comment too large.
pub fn heavy(n: usize, bytes: usize) -> String {
	let body = "x".repeat(bytes);
	let mut out = String::from("<testsuites><testsuite name=\"suite\">");
	for i in 0..n {
		let _ = write!(
			out,
			"<testcase classname=\"c\" name=\"t{i}\"><failure message=\"boom\">{body}</failure></testcase>"
		);
	}
	out.push_str("</testsuite></testsuites>");
	out
}

pub fn paths(prefix: &str, n: usize) -> Vec<String> {
	(0..n).map(|i| format!("src/{prefix}{i}.rs")).collect()
}

pub fn set_tree(fake: &FakeForge, sha: &str, paths: &[String]) {
	let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
	fake.set_tree(sha, &refs, false);
}

pub fn golden_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// How the request list is compared with its golden file.
pub fn check_golden(name: &str, requests: &[ApiRequest]) {
	let text = pr_results_forge::requests_to_json(requests);
	let path = golden_dir().join(format!("{name}.json"));
	if std::env::var_os("PR_RESULTS_BLESS").is_some() {
		std::fs::create_dir_all(path.parent().unwrap()).unwrap();
		std::fs::write(&path, &text).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&path)
		.unwrap_or_else(|_| panic!("missing {}; run with PR_RESULTS_BLESS=1", path.display()));
	assert!(
		text == want,
		"{name} differs from its golden file; review with PR_RESULTS_BLESS=1"
	);
}
