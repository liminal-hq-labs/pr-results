// Runs the host conformance suite for GitHub and for Forgejo over the in-memory forges.
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

use pr_results_core::RepoSlug;
use pr_results_forge::{FakeForge, FakeForgejo, Forgejo, GitHub, ACTIONS_LOGIN, DEFAULT_BOT_LOGIN};

use common::conformance::{comment_conformance, World};

struct GitHubWorld {
	fake: FakeForge,
	gh: GitHub<FakeForge>,
}

impl World for GitHubWorld {
	type H = GitHub<FakeForge>;

	fn host(&self) -> &Self::H {
		&self.gh
	}

	fn bot(&self) -> &str {
		DEFAULT_BOT_LOGIN
	}

	fn seed(&self, pr: u64, login: &str, body: &str) -> u64 {
		self.fake.add_comment(pr, login, body)
	}

	fn page_size(&self, n: usize) {
		self.fake.set_page_size(n);
	}

	fn comments(&self, pr: u64) -> Vec<(u64, String, String)> {
		self.fake
			.comments(pr)
			.into_iter()
			.map(|c| (c.id, c.login, c.body))
			.collect()
	}
}

fn world() -> GitHubWorld {
	let fake = FakeForge::new("example/app");
	let gh = GitHub::new(
		fake.clone(),
		RepoSlug::parse("example/app").unwrap(),
		DEFAULT_BOT_LOGIN,
	);
	GitHubWorld { fake, gh }
}

#[test]
fn github_passes_the_comment_conformance_suite() {
	comment_conformance(world);
}

struct ForgejoWorld {
	fake: FakeForgejo,
	host: Forgejo<FakeForgejo>,
}

impl World for ForgejoWorld {
	type H = Forgejo<FakeForgejo>;

	fn host(&self) -> &Self::H {
		&self.host
	}

	fn bot(&self) -> &str {
		ACTIONS_LOGIN
	}

	/// The system account has id -2, and anyone else a positive one, as on a real instance.
	fn seed(&self, pr: u64, login: &str, body: &str) -> u64 {
		self.fake.add_comment(pr, login, body)
	}

	fn page_size(&self, n: usize) {
		self.fake.set_page_size(n);
	}

	fn comments(&self, pr: u64) -> Vec<(u64, String, String)> {
		self.fake
			.comments(pr)
			.into_iter()
			.map(|c| (c.id, c.login, c.body))
			.collect()
	}
}

fn forgejo_world() -> ForgejoWorld {
	let fake = FakeForgejo::new("example/app");
	let host = Forgejo::new(fake.clone(), RepoSlug::parse("example/app").unwrap());
	ForgejoWorld { fake, host }
}

#[test]
fn forgejo_passes_the_comment_conformance_suite() {
	comment_conformance(forgejo_world);
}
