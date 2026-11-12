// Shared test support for Forgejo: a fake Forgejo with a pull request, inputs to publish and a plan.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use pr_results_core::{parse_junit, CommentKey, Flavour, Limits, RepoSlug, RunId, ServerUrl, Sha};
use pr_results_forge::{
	publish_inputs, FakeForgejo, FakePull, Forgejo, Inputs, Plan, PublishReport, WorkflowRun,
};

use super::{section, REPO, RUN, SHA};

pub const SERVER: &str = "https://forgejo.example.invalid";
pub const NUMBER: u64 = 77;

pub struct FjEnv {
	pub fake: FakeForgejo,
	pub host: Forgejo<FakeForgejo>,
	pub run: WorkflowRun,
	pub plan: Plan,
	pub server: ServerUrl,
	pub inputs: Inputs,
}

/// Suites parsed from JUnit documents, one per section in plan order.
pub fn inputs(documents: &[&str]) -> Inputs {
	let mut out = Inputs::default();
	for xml in documents {
		let report = parse_junit(xml.as_bytes(), Flavour::Generic, &Limits::default()).unwrap();
		out.suites.push(report.suites);
		out.skipped.push(0);
	}
	out
}

impl FjEnv {
	/// Pull request 7 from a branch of the repository itself, and a plan with one section.
	pub fn new(xml: &str) -> Self {
		let fake = FakeForgejo::new(REPO);
		fake.add_pull(FakePull::same_repo(7, REPO, SHA, "feature"));
		let host = Forgejo::new(fake.clone(), RepoSlug::parse(REPO).unwrap());
		let run = WorkflowRun {
			id: RunId(RUN),
			run_number: Some(NUMBER),
			head_sha: Sha::parse(SHA).unwrap(),
			head_branch: "feature".to_owned(),
			head_repo: RepoSlug::parse(REPO).unwrap(),
			event: "pull_request".to_owned(),
			conclusion: None,
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
			"0.2.0",
		);
		Self {
			fake,
			host,
			run,
			plan,
			server: ServerUrl::parse(SERVER).unwrap(),
			inputs: inputs(&[xml]),
		}
	}

	pub fn publish(&self) -> PublishReport {
		publish_inputs(
			&self.host,
			&self.run,
			&self.server,
			&self.plan,
			&self.inputs,
		)
	}

	/// The comments by our identity on pull request 7.
	pub fn ours(&self) -> Vec<pr_results_forge::FakeComment> {
		self.fake
			.comments(7)
			.into_iter()
			.filter(|c| c.login == "forgejo-actions" && c.user_id == -2)
			.collect()
	}
}
