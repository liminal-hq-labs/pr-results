// The boundary between the publisher, a host and the transport: a host that refuses everything, and
// the one place a token is added.
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

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use pr_results_core::{RepoSlug, RunId, Sha};
use pr_results_forge::{
	publish, Artifact, CommentResult, CommentSpec, ForgeError, Host, HostKind, Pull, ResultSpec,
	ResultWritten, Tree,
};

use common::Env;

/// A host of another kind that answers every question with a refusal and keeps the order of the
/// questions, so the flow is shown to ask only through [`Host`] and to stop at the first refusal.
struct Stub {
	repo: RepoSlug,
	calls: RefCell<Vec<&'static str>>,
}

impl Stub {
	fn new() -> Self {
		Self {
			repo: RepoSlug::parse("example/app").unwrap(),
			calls: RefCell::new(Vec::new()),
		}
	}

	fn refuse<T>(&self, call: &'static str) -> Result<T, ForgeError> {
		self.calls.borrow_mut().push(call);
		Err(ForgeError::Refused("the stub host refuses everything"))
	}
}

impl Host for Stub {
	fn kind(&self) -> HostKind {
		HostKind::Forgejo
	}
	fn repo(&self) -> &RepoSlug {
		&self.repo
	}
	fn begin_run(&self) {
		self.calls.borrow_mut().push("begin_run");
	}
	fn retry_budget_spent(&self) -> bool {
		false
	}
	fn artifacts(&self, _: RunId) -> Result<Vec<Artifact>, ForgeError> {
		self.refuse("artifacts")
	}
	fn download(&self, _: &Artifact, _: u64) -> Result<Vec<u8>, ForgeError> {
		self.refuse("download")
	}
	fn pull(&self, _: u64) -> Result<Option<Pull>, ForgeError> {
		self.refuse("pull")
	}
	fn pulls_for_commit(&self, _: &Sha) -> Result<Vec<Pull>, ForgeError> {
		self.refuse("pulls_for_commit")
	}
	fn pulls_by_head(&self, _: &str, _: &str) -> Result<Vec<Pull>, ForgeError> {
		self.refuse("pulls_by_head")
	}
	fn tree_paths(&self, _: &Sha) -> Result<Tree, ForgeError> {
		self.refuse("tree_paths")
	}
	fn upsert_result(&self, _: &ResultSpec<'_>) -> Result<ResultWritten, ForgeError> {
		self.refuse("upsert_result")
	}
	fn upsert_comment(&self, _: &CommentSpec<'_>) -> Result<CommentResult, ForgeError> {
		self.refuse("upsert_comment")
	}
}

#[test]
fn a_host_that_refuses_everything_stops_the_flow_at_its_first_question() {
	let env = Env::new();
	let stub = Stub::new();
	let err = publish(&stub, &env.run, &env.server, &env.plan).unwrap_err();
	assert_eq!(err.stage, "listing artefacts");
	assert!(matches!(err.source, ForgeError::Refused(_)));
	assert_eq!(*stub.calls.borrow(), ["begin_run", "artifacts"]);
}

#[test]
fn a_cancelled_run_asks_the_host_nothing() {
	let mut env = Env::new();
	env.run.conclusion = Some("cancelled".to_owned());
	let stub = Stub::new();
	let report = publish(&stub, &env.run, &env.server, &env.plan).unwrap();
	assert!(report.nothing_to_do);
	assert!(stub.calls.borrow().is_empty());
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
	let Ok(entries) = fs::read_dir(dir) else {
		return;
	};
	for entry in entries.flatten() {
		let path = entry.path();
		if path.is_dir() {
			sources(&path, out);
		} else if path.extension().is_some_and(|e| e == "rs") {
			out.push(path);
		}
	}
}

/// The code lines of a source file: comments and doc comments left out.
fn code_lines(path: &Path) -> Vec<(usize, String)> {
	fs::read_to_string(path)
		.unwrap()
		.lines()
		.enumerate()
		.filter(|(_, l)| !l.trim_start().starts_with("//"))
		.map(|(i, l)| (i + 1, l.to_owned()))
		.collect()
}

#[test]
fn the_token_is_added_only_in_ureq_forge_send() {
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
	let mut files = Vec::new();
	for krate in [
		"pr-results",
		"pr-results-core",
		"pr-results-forge",
		"pr-results-render",
	] {
		sources(&root.join(krate).join("src"), &mut files);
	}
	assert!(files.len() > 20, "the source files were not found");

	let mut exposes = Vec::new();
	let mut headers = Vec::new();
	for file in &files {
		for (n, line) in code_lines(file) {
			let name = file.file_name().unwrap().to_string_lossy().into_owned();
			if line.contains(".expose(") {
				exposes.push((name.clone(), n));
			}
			if line.contains("\"Authorization\"") {
				headers.push((name, n));
			}
		}
	}

	// `Token::expose` is called once, and the `Authorization` header is named once, both in the
	// real forge's file.
	assert_eq!(exposes.len(), 1, "token exposed at {exposes:?}");
	assert_eq!(headers.len(), 1, "Authorization named at {headers:?}");
	assert_eq!(exposes[0].0, "ureq_forge.rs");
	assert_eq!(headers[0].0, "ureq_forge.rs");

	// The call is inside `UreqForge`'s `send`, and the header is added by the function `send` hands
	// the value to.
	let text = fs::read_to_string(root.join("pr-results-forge/src/ureq_forge.rs")).unwrap();
	let lines: Vec<&str> = text.lines().collect();
	let start = lines
		.iter()
		.position(|l| l.contains("fn send(&self, req: &ApiRequest)"))
		.unwrap();
	let end = lines[start..]
		.iter()
		.position(|l| *l == "\t}")
		.map(|i| start + i)
		.unwrap();
	assert!(
		(start + 1..=end).contains(&exposes[0].1),
		"expose is at line {}, outside send ({}..{})",
		exposes[0].1,
		start + 1,
		end
	);
	let common = lines
		.iter()
		.position(|l| l.starts_with("fn common"))
		.unwrap();
	assert!(
		(common + 1..common + 4).contains(&(headers[0].1 - 1)),
		"the header is not added by `common`"
	);
}
