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
use pr_results_render::{CheckOutput, StatusState};
use serde_json::Value;

use crate::api::ForgeError;

/// Which forge a host speaks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
	GitHub,
	Forgejo,
}

impl HostKind {
	/// The name settings and the command line use: `github` or `forgejo`.
	pub fn name(self) -> &'static str {
		match self {
			Self::GitHub => "github",
			Self::Forgejo => "forgejo",
		}
	}

	pub fn from_name(name: &str) -> Option<Self> {
		match name {
			"github" => Some(Self::GitHub),
			"forgejo" => Some(Self::Forgejo),
			_ => None,
		}
	}
}

/// What to write for one section's result: a check run on GitHub, a commit status on Forgejo.
/// A host given the other kind refuses it.
#[derive(Debug, Clone, Copy)]
pub enum ResultSpec<'a> {
	Check(CheckSpec<'a>),
	Status(StatusSpec<'a>),
}

/// What was written for one section's result. For a commit status `id` is the status's id,
/// `created` is true (a status is never edited: a later one supersedes it) and no annotations
/// were sent.
pub type ResultWritten = CheckResult;

/// What to write for one commit status. The context is the check name, so there is one per
/// section, and a later status with the same context supersedes the earlier one.
#[derive(Debug, Clone, Copy)]
pub struct StatusSpec<'a> {
	pub context: &'a str,
	pub head_sha: &'a Sha,
	pub state: StatusState,
	/// Plain text, at most 140 characters.
	pub description: &'a str,
	/// A link the tool built: the comment's anchor, else the run's page.
	pub target_url: &'a str,
}

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
	/// The branch the pull request merges into. `None` when the API gave none or gave one that
	/// is not a plausible branch name (see [`ref_name`]).
	pub base_ref: Option<String>,
	/// The commit of the base branch the API reports. `None` when it is absent or is not a full
	/// lower-case commit hash. Whether it is the branch's tip or its merge base with the head is
	/// not something the API documents, so a baseline lookup falls back to the branch.
	pub base_sha: Option<Sha>,
}

/// A branch name as the API reports it is kept to this many bytes.
pub(crate) const REF_NAME_BYTES: usize = 255;

/// `s` if it can be a branch name that is safe to carry around and to show later: 1 to
/// [`REF_NAME_BYTES`] bytes with no control character. A longer name is refused and not cut,
/// because half a branch name names another branch. Git's own rules for a ref are stricter, which
/// is not this function's business: a name that Git would refuse simply matches nothing.
pub(crate) fn ref_name(s: &str) -> Option<String> {
	let ok = !s.is_empty() && s.len() <= REF_NAME_BYTES && !s.chars().any(char::is_control);
	ok.then(|| s.to_owned())
}

/// A comment as listed: its body is only the first [`BODY_PREFIX_BYTES`] bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
	pub id: u64,
	pub login: String,
	/// The account's id when the forge reports one. Forgejo's automatic token acts as
	/// `forgejo-actions` with id -2, which no person can have.
	pub user_id: Option<i64>,
	/// `User`, `Bot` and so on, as GitHub reports the account. Forgejo has no such field.
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

/// The start of every `external_id` this tool gives a check run, ahead of the comment key and the
/// check's name. It is how a listing tells our own check runs from other tools'.
pub(crate) const EXTERNAL_ID_PREFIX: &str = "pr-results:1:";

/// A check run on the commit as another tool or job reports it. The name is untrusted: whoever
/// wrote that workflow chose it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRunInfo {
	pub name: String,
	/// `queued`, `in_progress`, `completed` and so on.
	pub status: String,
	/// `success`, `failure`, `cancelled` and so on, once it has completed.
	pub conclusion: Option<String>,
	/// The page the check links to, such as a job in a workflow run.
	pub details_url: Option<String>,
	/// A check run this tool wrote: its `external_id` is ours and our app made it.
	pub ours: bool,
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
	/// The check runs on a commit, the latest of each name. A forge without check runs has none
	/// to list.
	fn check_runs(&self, _sha: &Sha) -> Result<Vec<CheckRunInfo>, ForgeError> {
		Ok(Vec::new())
	}
	fn upsert_result(&self, spec: &ResultSpec<'_>) -> Result<ResultWritten, ForgeError>;
	fn upsert_comment(&self, spec: &CommentSpec<'_>) -> Result<CommentResult, ForgeError>;
}

/// A pull request as both forges report it: `number`, `state`, `head.{sha,ref,repo.full_name}`,
/// `base.repo.full_name` and `base.{ref,sha}`. A head repository that has been deleted is `null`.
/// The base's `ref` and `sha` are read leniently: a pull request is still read when they are
/// missing or malformed.
pub(crate) fn pull_from(v: &Value) -> Option<Pull> {
	let repo = |side: &str| {
		v.get(side)?
			.get("repo")?
			.get("full_name")?
			.as_str()
			.map(str::to_owned)
	};
	Some(Pull {
		number: v.get("number")?.as_u64()?,
		open: v.get("state")?.as_str()? == "open",
		head_sha: v.get("head")?.get("sha")?.as_str()?.to_owned(),
		head_ref: v.get("head")?.get("ref")?.as_str()?.to_owned(),
		head_repo: repo("head"),
		base_repo: repo("base"),
		base_ref: v
			.get("base")
			.and_then(|b| b.get("ref"))
			.and_then(Value::as_str)
			.and_then(ref_name),
		base_sha: v
			.get("base")
			.and_then(|b| b.get("sha"))
			.and_then(Value::as_str)
			.and_then(Sha::parse),
	})
}

/// The first [`BODY_PREFIX_BYTES`] bytes of `s`, cut at a character boundary.
pub(crate) fn prefix(s: &str) -> String {
	let mut end = s.len().min(BODY_PREFIX_BYTES);
	while !s.is_char_boundary(end) {
		end -= 1;
	}
	s.get(..end).unwrap_or("").to_owned()
}

/// A listed comment, from either forge's JSON, reduced to what finding ours needs.
pub(crate) fn comment_from(c: &Value) -> Option<Comment> {
	let user = c.get("user")?;
	Some(Comment {
		id: c.get("id")?.as_u64()?,
		login: user.get("login")?.as_str()?.to_owned(),
		user_id: user.get("id").and_then(Value::as_i64),
		user_type: user
			.get("type")
			.and_then(Value::as_str)
			.unwrap_or("")
			.to_owned(),
		created_at: c
			.get("created_at")
			.and_then(Value::as_str)
			.unwrap_or("")
			.to_owned(),
		body: prefix(c.get("body").and_then(Value::as_str).unwrap_or("")),
	})
}

#[cfg(test)]
#[allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]
mod tests {
	use serde_json::json;

	use super::*;

	const SHA: &str = "3f9c2e1d4b5a69788796a5b4c3d2e1f001122334";

	fn pull(base: Value) -> Value {
		json!({
			"number": 7,
			"state": "open",
			"head": { "sha": SHA, "ref": "feature", "repo": { "full_name": "o/r" } },
			"base": base,
		})
	}

	#[test]
	fn a_pull_request_reports_its_base_branch_and_commit() {
		let base_sha = "a".repeat(40);
		let p = pull_from(&pull(json!({
			"ref": "main", "sha": base_sha, "repo": { "full_name": "o/r" },
		})))
		.unwrap();
		assert_eq!(p.base_ref.as_deref(), Some("main"));
		assert_eq!(p.base_sha, Sha::parse(&base_sha));
		assert_eq!(p.base_repo.as_deref(), Some("o/r"));
	}

	#[test]
	fn a_missing_base_ref_or_sha_does_not_fail_the_read() {
		let p = pull_from(&pull(json!({ "repo": { "full_name": "o/r" } }))).unwrap();
		assert_eq!((p.base_ref, p.base_sha), (None, None));
		let p = pull_from(&pull(json!(null))).unwrap();
		assert_eq!((p.base_ref, p.base_sha, p.base_repo), (None, None, None));
	}

	#[test]
	fn a_malformed_base_ref_or_sha_is_dropped_and_the_pull_request_still_reads() {
		let long = "b".repeat(REF_NAME_BYTES + 1);
		for ref_value in [
			json!(""),
			json!(long),
			json!("fea\nture"),
			json!("fea\u{1b}[31mture"),
			json!(7),
			json!(["main"]),
			json!(null),
		] {
			let p = pull_from(&pull(json!({ "ref": ref_value, "sha": SHA }))).unwrap();
			assert_eq!(p.base_ref, None, "{ref_value}");
			assert_eq!(p.base_sha, Sha::parse(SHA));
		}
		for sha_value in [
			json!(""),
			json!("abc123"),
			json!(SHA.to_uppercase()),
			json!(format!("{SHA}0")),
			json!(7),
			json!(null),
		] {
			let p = pull_from(&pull(json!({ "ref": "main", "sha": sha_value }))).unwrap();
			assert_eq!(p.base_sha, None, "{sha_value}");
			assert_eq!(p.base_ref.as_deref(), Some("main"));
		}
	}

	#[test]
	fn a_branch_name_is_kept_whole_or_not_at_all() {
		assert_eq!(ref_name("release/1.x").as_deref(), Some("release/1.x"));
		assert_eq!(ref_name("héron").as_deref(), Some("héron"));
		let longest = "x".repeat(REF_NAME_BYTES);
		assert_eq!(ref_name(&longest).as_deref(), Some(longest.as_str()));
		assert_eq!(ref_name(&format!("{longest}x")), None);
		assert_eq!(ref_name(""), None);
		assert_eq!(ref_name("a\tb"), None);
		assert_eq!(ref_name("a\u{7f}b"), None);
	}
}
