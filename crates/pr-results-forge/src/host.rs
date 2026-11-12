// The `Host` trait: what the publisher needs from a forge's API, and the plain types it trades in.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A host turns the publisher's questions (the run's artefacts, a pull request, a commit's tree) and
//! its writes (a result for a section, the sticky comment) into one forge's REST calls. The
//! transport underneath ([`Forge`](crate::Forge), through [`Client`](crate::Client)) knows nothing
//! of any host's routes, and a host never touches the token. [`publish`](crate::publish) is generic
//! over [`Host`], so nothing in the flow names a forge.

use std::collections::BTreeSet;

use pr_results_core::{Marker, RunId, Sha};
use pr_results_render::CheckOutput;

use crate::api::ForgeError;

/// Which forge a host speaks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
	GitHub,
	Forgejo,
}

/// What to write for one section's result: a check run on GitHub.
pub type ResultSpec<'a> = CheckSpec<'a>;
/// What was written for one section's result.
pub type ResultWritten = CheckResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
	pub id: u64,
	/// Chosen by whoever wrote the workflow, so untrusted.
	pub name: String,
	pub size_in_bytes: u64,
	pub expired: bool,
}

/// A pull request, as far as verification needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pull {
	pub number: u64,
	pub open: bool,
	pub head_sha: String,
	pub head_ref: String,
	/// `None` when the head repository has been deleted.
	pub head_repo: Option<String>,
	pub base_repo: Option<String>,
}

/// A comment as listed: its body is only the first [`BODY_PREFIX_BYTES`] bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
	pub id: u64,
	pub login: String,
	/// `User`, `Bot` and so on, as GitHub reports the account.
	pub user_type: String,
	pub created_at: String,
	pub body: String,
}

/// How much of a comment's body is kept while listing.
pub const BODY_PREFIX_BYTES: usize = 256;

/// The paths in a commit's tree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tree {
	pub paths: BTreeSet<String>,
	/// GitHub cut the listing short, so a path that is missing may still exist.
	pub truncated: bool,
}

/// What to write for one check run.
#[derive(Debug, Clone, Copy)]
pub struct CheckSpec<'a> {
	pub name: &'a str,
	pub head_sha: &'a Sha,
	pub external_id: &'a str,
	pub output: &'a CheckOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
	pub id: u64,
	pub created: bool,
	pub annotations_sent: usize,
	pub annotation_batches: usize,
	/// Annotations in batches that GitHub refused (422) and that were left out.
	pub annotations_rejected: usize,
}

/// What to write for one comment.
#[derive(Debug, Clone, Copy)]
pub struct CommentSpec<'a> {
	pub pr: u64,
	pub marker: &'a Marker,
	pub body: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentAction {
	Created,
	Updated,
	/// A comment for the same commit from a later run is already there.
	SkippedNewer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentResult {
	pub id: u64,
	pub action: CommentAction,
	/// The bot's other comments with this key that were deleted so one remains.
	pub duplicates_removed: usize,
}

/// The operations the publisher needs from a forge. `begin_run` starts the clock of the retry
/// budget and `retry_budget_spent` says whether a retry was refused for want of it.
pub trait Host {
	fn kind(&self) -> HostKind;
	fn repo(&self) -> &pr_results_core::RepoSlug;
	fn begin_run(&self);
	fn retry_budget_spent(&self) -> bool;
	fn artifacts(&self, run: RunId) -> Result<Vec<Artifact>, ForgeError>;
	fn download(&self, artifact: &Artifact, max_bytes: u64) -> Result<Vec<u8>, ForgeError>;
	fn pull(&self, number: u64) -> Result<Option<Pull>, ForgeError>;
	fn pulls_for_commit(&self, sha: &Sha) -> Result<Vec<Pull>, ForgeError>;
	fn pulls_by_head(&self, owner: &str, branch: &str) -> Result<Vec<Pull>, ForgeError>;
	fn tree_paths(&self, sha: &Sha) -> Result<Tree, ForgeError>;
	fn upsert_result(&self, spec: &ResultSpec<'_>) -> Result<ResultWritten, ForgeError>;
	fn upsert_comment(&self, spec: &CommentSpec<'_>) -> Result<CommentResult, ForgeError>;
}
