// Shared test support: a fake GitHub behind the command line, an environment and temporary files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]
#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

#[path = "../../../pr-results-github/tests/common/zip.rs"]
pub mod zip;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pr_results::{run, ForgeFactory, Streams};
use pr_results_github::{FakeForge, FakePull, ForgeError, Token};

pub const REPO: &str = "example/app";
pub const SHA: &str = "3f9c2e1d4b5a69788796a5b4c3d2e1f001122334";
pub const TOKEN: &str = "ghs_testtoken1234567890";

pub fn root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn fixture(name: &str) -> String {
	std::fs::read_to_string(root().join("tests/fixtures").join(name)).unwrap()
}

/// A directory of its own under the target directory, empty.
pub fn scratch(name: &str) -> PathBuf {
	let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cli-{name}"));
	let _ = std::fs::remove_dir_all(&dir);
	std::fs::create_dir_all(&dir).unwrap();
	dir
}

/// Hands the command a clone of the fake and remembers what it was asked for.
pub struct Factory {
	pub fake: FakeForge,
	pub asked: RefCell<Vec<String>>,
}

impl Factory {
	pub fn new(fake: &FakeForge) -> Self {
		Self {
			fake: fake.clone(),
			asked: RefCell::new(Vec::new()),
		}
	}
}

impl ForgeFactory for Factory {
	type Forge = FakeForge;

	fn make(&self, api_base: &str, _token: Token) -> Result<FakeForge, ForgeError> {
		self.asked.borrow_mut().push(api_base.to_owned());
		Ok(self.fake.clone())
	}
}

pub struct Outcome {
	pub code: u8,
	pub out: String,
	pub err: String,
}

/// Runs the command line with `vars` as the environment.
pub fn invoke(args: &[&str], vars: &BTreeMap<String, String>, factory: &Factory) -> Outcome {
	let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
	let lookup = |name: &str| vars.get(name).cloned();
	let mut out = Vec::new();
	let mut err = Vec::new();
	let code = {
		let mut io = Streams {
			out: &mut out,
			err: &mut err,
		};
		run(&args, &lookup, factory, &mut io)
	};
	Outcome {
		code,
		out: String::from_utf8(out).unwrap(),
		err: String::from_utf8(err).unwrap(),
	}
}

/// The environment of a publisher running on the default branch with `event` as its event file.
pub fn publisher_env(dir: &Path, event_json: &str) -> BTreeMap<String, String> {
	let event = dir.join("event.json");
	std::fs::write(&event, event_json).unwrap();
	let mut vars = BTreeMap::new();
	for (k, v) in [
		("GITHUB_EVENT_NAME", "workflow_run"),
		("GITHUB_REPOSITORY", REPO),
		("GITHUB_REF", "refs/heads/main"),
		("GITHUB_API_URL", "https://api.github.com"),
		("GITHUB_SERVER_URL", "https://github.com"),
		("GITHUB_TOKEN", TOKEN),
	] {
		vars.insert(k.to_owned(), v.to_owned());
	}
	vars.insert("GITHUB_EVENT_PATH".to_owned(), event.display().to_string());
	vars
}

/// A fake GitHub with pull request 7 from a branch of the repository.
pub fn github_with_pr() -> FakeForge {
	let fake = FakeForge::new(REPO);
	fake.add_pull(FakePull::same_repo(7, REPO, SHA, "feature"));
	fake
}

pub fn artifact(fake: &FakeForge, name: &str, files: &[(&str, &str)]) -> u64 {
	let specs: Vec<zip::Spec> = files
		.iter()
		.map(|(n, c)| zip::Spec::deflated(n, c.as_bytes()))
		.collect();
	fake.add_artifact(5000, name, zip::build(&specs))
}

pub const DESK_TOML: &str = r#"
schema = 1
title = "Desk checks"

[[suite]]
name = "Desk steps"
check = "Desk check steps"
artifact = "results-*"
files = ["**/steps.xml"]
flavour = "steps"
required = true

[[suite]]
name = "Rust tests"
check = "Desk Rust tests"
artifact = "results-*"
files = ["**/nextest-*.xml"]
flavour = "nextest"
"#;
