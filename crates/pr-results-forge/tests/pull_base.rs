// A pull request's base branch and commit, as GitHub's routes report them.
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

use pr_results_core::{RepoSlug, Sha};
use pr_results_forge::{FakeForge, FakePull, GitHub, Host, DEFAULT_BOT_LOGIN};

use common::{REPO, SHA};

const BASE: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const OTHER: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";

#[test]
fn a_pull_request_names_its_base_branch_and_commit() {
	let fake = FakeForge::new(REPO);
	let gh = GitHub::new(
		fake.clone(),
		RepoSlug::parse(REPO).unwrap(),
		DEFAULT_BOT_LOGIN,
	);
	fake.add_pull(FakePull {
		base_ref: "release/1.x".to_owned(),
		base_sha: BASE.to_owned(),
		..FakePull::same_repo(7, REPO, SHA, "feature")
	});
	fake.add_pull(FakePull {
		base_ref: "x".repeat(256),
		base_sha: "short".to_owned(),
		..FakePull::same_repo(8, REPO, OTHER, "other")
	});
	let p = gh.pull(7).unwrap().unwrap();
	assert_eq!(p.base_ref.as_deref(), Some("release/1.x"));
	assert_eq!(p.base_sha, Sha::parse(BASE));
	let p = gh.pull(8).unwrap().unwrap();
	assert_eq!((p.base_ref, p.base_sha), (None, None));
}
