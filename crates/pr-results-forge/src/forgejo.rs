// Forgejo as a `Host`: reads, commit statuses and the sticky comment, over a `Client`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Forgejo's routes and shapes behind [`Host`], from the v17 Swagger and source (the fixtures in
//! `tests/fixtures/forgejo/` say which). Forgejo has no check runs, so a section's result is a
//! commit status whose context is the check name; and the sticky comment goes through the issues
//! API, where the list of comments comes whole with an `X-Total-Count` that is checked.
//!
//! **Whose comment is ours.** With the job's automatic token the comment author is
//! `forgejo-actions`, a system account with id -2 that no person can have, so a comment is ours
//! only if the login *and* that id match. With a personal token of a bot account (`bot_login`) the
//! account is read from `GET /user` and both of its login and id must match. A comment whose
//! author is anyone else is never updated, counted or deleted, whatever its body says.

use std::cell::OnceCell;
use std::collections::BTreeSet;
use std::time::Duration;

use pr_results_core::{CommentKey, Marker, RepoSlug, RunId, Sha};
use pr_results_render::StatusState;
use serde_json::{json, Value};

use crate::api::{ApiPath, ApiRequest, Forge, ForgeError, Method};
use crate::client::{Client, Paging, BACKOFF_S};
use crate::host::{
	comment_from, pull_from, Artifact, CheckResult, Comment, CommentResult, CommentSpec, Host,
	HostKind, Pull, ResultSpec, ResultWritten, StatusSpec, Tree,
};
use crate::sticky::{upsert_sticky, Sticky};

/// The login of the automatic token's identity.
pub const ACTIONS_LOGIN: &str = "forgejo-actions";
/// Its id. Forgejo gives system accounts negative ids; no person has one.
pub const ACTIONS_ID: i64 = -2;

/// How many times a create is repeated after it was confirmed not to have happened.
const CREATE_ATTEMPTS: usize = 3;
/// Trees come this many entries to a page, which is also the most Forgejo serves.
const TREE_PER_PAGE: u64 = 1000;
/// At most this many pages of a tree are read: 100 000 entries.
const MAX_TREE_PAGES: u64 = 100;

#[derive(Debug)]
enum Identity {
	/// The job's own token: `forgejo-actions`, id -2.
	Actions,
	/// A bot account's personal token. `login` is the setting; `resolved` is who `GET /user` says.
	Account {
		login: String,
		resolved: OnceCell<(String, i64)>,
	},
}

/// Forgejo's REST API over a [`Client`]: a commit status per section and an issue comment, found
/// by the marker and by author.
pub struct Forgejo<F: Forge> {
	client: Client<F>,
	identity: Identity,
}

impl<F: Forge> Forgejo<F> {
	/// A host that acts as the job's automatic token, `forgejo-actions` with id -2.
	pub fn new(forge: F, repo: RepoSlug) -> Self {
		Self {
			client: Client::new(forge, repo),
			identity: Identity::Actions,
		}
	}

	/// A host whose token is the personal token of the bot account `login`. [`Forgejo::identity`]
	/// asks the server who that is and refuses a token that belongs to someone else.
	pub fn with_account(mut self, login: &str) -> Self {
		self.identity = Identity::Account {
			login: login.to_owned(),
			resolved: OnceCell::new(),
		};
		self
	}

	/// How many seconds of waiting, in total, one publish may spend on retries before it stops
	/// retrying and reports what it has.
	pub fn with_retry_budget(mut self, seconds: u64) -> Self {
		self.client = self.client.with_retry_budget(seconds);
		self
	}

	pub fn forge(&self) -> &F {
		self.client.forge()
	}

	/// The login and id of the account whose comments and statuses are ours. For a bot account it
	/// is read from `GET /user` once; a token that belongs to another account is
	/// [`ForgeError::Refused`], so the command line can stop with a usage error before it writes.
	pub fn identity(&self) -> Result<(String, i64), ForgeError> {
		match &self.identity {
			Identity::Actions => Ok((ACTIONS_LOGIN.to_owned(), ACTIONS_ID)),
			Identity::Account { login, resolved } => {
				if let Some(known) = resolved.get() {
					return Ok(known.clone());
				}
				let path = ApiPath::parse("/user")
					.ok_or(ForgeError::Decode("a request path could not be built"))?;
				let resp = Client::<F>::ok(
					self.client.request(&ApiRequest::get(path))?,
					"reading the token's account",
				)?;
				let doc = resp.json()?;
				let (Some(name), Some(id)) = (
					doc.get("login").and_then(Value::as_str),
					doc.get("id").and_then(Value::as_i64),
				) else {
					return Err(ForgeError::Decode("an account was expected"));
				};
				if !name.eq_ignore_ascii_case(login) {
					return Err(ForgeError::Refused(
						"the token belongs to another account than the configured bot login",
					));
				}
				let _ = resolved.set((name.to_owned(), id));
				Ok((name.to_owned(), id))
			}
		}
	}

	fn is_ours(&self, who: &(String, i64), login: &str, id: Option<i64>) -> bool {
		login == who.0 && id == Some(who.1)
	}

	// ---- commit statuses ---------------------------------------------------------------------

	/// The id of our latest status for `spec.context` on the commit, if it says what `spec` says:
	/// the same state, description and link. A status carries no run id, so "this write landed" is
	/// "ours says what we are about to write". Taking the highest id of ours means a later status
	/// from someone else, or an earlier one of ours, is not mistaken for it.
	fn find_status(&self, spec: &StatusSpec<'_>) -> Result<Option<u64>, ForgeError> {
		let who = self.identity()?;
		let req = self
			.client
			.get(&format!("commits/{}/statuses", spec.head_sha.as_str()))?;
		let mine = self.client.paged(
			req,
			Paging::Forgejo,
			None,
			"listing statuses",
			|s: &Value| {
				let creator = s.get("creator")?;
				let ours = s.get("context").and_then(Value::as_str) == Some(spec.context)
					&& self.is_ours(
						&who,
						creator.get("login")?.as_str()?,
						creator.get("id").and_then(Value::as_i64),
					);
				ours.then(|| {
					let text = |k: &str| s.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
					Some((
						s.get("id")?.as_u64()?,
						text("status"),
						text("description"),
						text("target_url"),
					))
				})
				.flatten()
			},
		)?;
		let latest = mine.into_iter().max_by_key(|s| s.0);
		Ok(latest.and_then(|(id, state, description, url)| {
			(state == state_word(spec.state)
				&& description == spec.description
				&& url == spec.target_url)
				.then_some(id)
		}))
	}

	/// Writes one commit status. A later status with the same context supersedes it, so nothing is
	/// looked for first. A POST that failed with a 5xx or a dropped connection may have been
	/// applied, so it is repeated only after a listing shows that none of ours says the same.
	pub(crate) fn upsert_status(&self, spec: &StatusSpec<'_>) -> Result<CheckResult, ForgeError> {
		let body = json!({
			"state": state_word(spec.state),
			"context": spec.context,
			"description": spec.description,
			"target_url": spec.target_url,
		});
		let req = self.client.json_request(
			Method::Post,
			&format!("statuses/{}", spec.head_sha.as_str()),
			&body,
		)?;
		let written = |id| CheckResult {
			id,
			created: true,
			annotations_sent: 0,
			annotation_batches: 0,
			annotations_rejected: 0,
		};
		for attempt in 0..CREATE_ATTEMPTS {
			match self.client.request(&req) {
				Ok(resp) if resp.ok() => {
					let id = resp
						.json()?
						.get("id")
						.and_then(Value::as_u64)
						.ok_or(ForgeError::Decode("a status was expected"))?;
					return Ok(written(id));
				}
				Ok(resp) if resp.status < 500 => {
					return Err(ForgeError::Status {
						status: resp.status,
						op: "creating a status",
					})
				}
				Ok(_) | Err(ForgeError::Transport(_)) => {}
				Err(e) => return Err(e),
			}
			let wait = BACKOFF_S.get(attempt).copied().unwrap_or(1);
			if !self.client.may_wait(wait) {
				break;
			}
			self.client.forge().sleep(Duration::from_secs(wait));
			if let Some(id) = self.find_status(spec)? {
				return Ok(written(id));
			}
		}
		Err(ForgeError::Status {
			status: 502,
			op: "creating a status",
		})
	}

	// ---- the sticky comment ------------------------------------------------------------------

	/// The comments of a pull request, each reduced to what finding ours needs. Forgejo returns
	/// them all in one response with the total in `X-Total-Count`, which must match. Public for the
	/// tests, which read the listing the flow sees.
	pub fn comments(&self, pr: u64) -> Result<Vec<Comment>, ForgeError> {
		let req = self.client.get(&format!("issues/{pr}/comments"))?;
		self.client
			.paged(req, Paging::Whole, None, "listing comments", comment_from)
	}

	fn delete(&self, id: u64) -> bool {
		self.client
			.path(&format!("issues/comments/{id}"))
			.map(|p| ApiRequest {
				method: Method::Delete,
				path: p,
				query: Vec::new(),
				body: None,
			})
			.and_then(|r| self.client.request(&r))
			.is_ok_and(|r| r.ok())
	}
}

fn state_word(s: StatusState) -> &'static str {
	match s {
		StatusState::Success => "success",
		StatusState::Failure => "failure",
	}
}

impl<F: Forge> Sticky for Forgejo<F> {
	fn ours(&self, pr: u64, key: &CommentKey) -> Result<Vec<Comment>, ForgeError> {
		let who = self.identity()?;
		let mut ours: Vec<Comment> = self
			.comments(pr)?
			.into_iter()
			.filter(|c| {
				self.is_ours(&who, &c.login, c.user_id) && Marker::prefix_matches(&c.body, key)
			})
			.collect();
		ours.sort_by_key(|c| c.id);
		Ok(ours)
	}

	fn patch(&self, id: u64, body: &str) -> Result<bool, ForgeError> {
		let req = self.client.json_request(
			Method::Patch,
			&format!("issues/comments/{id}"),
			&json!({ "body": body }),
		)?;
		let resp = self.client.request(&req)?;
		if resp.status == 404 {
			return Ok(false);
		}
		Client::<F>::ok(resp, "updating a comment").map(|_| true)
	}

	/// Creates the comment. Its author, as Forgejo reports it, must be the identity this host
	/// looks for: if it is not (a personal token without `bot_login`), later runs would never
	/// find the comment and would add another each time, so the comment is removed and the write
	/// is refused.
	fn create(&self, spec: &CommentSpec<'_>) -> Result<u64, ForgeError> {
		let who = self.identity()?;
		let req = self.client.json_request(
			Method::Post,
			&format!("issues/{}/comments", spec.pr),
			&json!({ "body": spec.body }),
		)?;
		for attempt in 0..CREATE_ATTEMPTS {
			match self.client.request(&req) {
				Ok(resp) if resp.ok() => {
					let doc = resp.json()?;
					let id = doc
						.get("id")
						.and_then(Value::as_u64)
						.ok_or(ForgeError::Decode("a comment was expected"))?;
					let user = doc.get("user");
					let author_ok = user.is_some_and(|u| {
						self.is_ours(
							&who,
							u.get("login").and_then(Value::as_str).unwrap_or(""),
							u.get("id").and_then(Value::as_i64),
						)
					});
					if !author_ok {
						let _ = self.delete(id);
						return Err(ForgeError::Refused(
							"the comment was written by another account than the token's identity: set bot_login for a personal token",
						));
					}
					return Ok(id);
				}
				Ok(resp) if resp.status < 500 => {
					return Err(ForgeError::Status {
						status: resp.status,
						op: "creating a comment",
					})
				}
				Ok(_) | Err(ForgeError::Transport(_)) => {}
				Err(e) => return Err(e),
			}
			// It may have been applied: look for the same marker before repeating.
			let wait = BACKOFF_S.get(attempt).copied().unwrap_or(1);
			if !self.client.may_wait(wait) {
				break;
			}
			self.client.forge().sleep(Duration::from_secs(wait));
			let ours = self.ours(spec.pr, &spec.marker.key)?;
			if let Some(c) = ours
				.iter()
				.find(|c| Marker::parse(&c.body).as_ref() == Some(spec.marker))
			{
				return Ok(c.id);
			}
		}
		Err(ForgeError::Status {
			status: 502,
			op: "creating a comment",
		})
	}

	fn remove(&self, ids: &mut dyn Iterator<Item = u64>) -> usize {
		ids.filter(|id| self.delete(*id)).count()
	}
}

impl<F: Forge> Host for Forgejo<F> {
	fn kind(&self) -> HostKind {
		HostKind::Forgejo
	}

	fn repo(&self) -> &RepoSlug {
		self.client.repo()
	}

	fn begin_run(&self) {
		self.client.begin_run();
	}

	fn retry_budget_spent(&self) -> bool {
		self.client.retry_budget_spent()
	}

	/// Forgejo has no `workflow_run` trigger and so no run to list artefacts of; the results of a
	/// run are read from files in the job itself.
	fn artifacts(&self, _run: RunId) -> Result<Vec<Artifact>, ForgeError> {
		Err(ForgeError::Refused(
			"Forgejo artefacts are not read through the API: results come from files in the job",
		))
	}

	fn download(&self, _artifact: &Artifact, _max_bytes: u64) -> Result<Vec<u8>, ForgeError> {
		Err(ForgeError::Refused(
			"Forgejo artefacts are not read through the API: results come from files in the job",
		))
	}

	fn pull(&self, number: u64) -> Result<Option<Pull>, ForgeError> {
		let resp = self
			.client
			.request(&self.client.get(&format!("pulls/{number}"))?)?;
		if resp.status == 404 {
			return Ok(None);
		}
		let resp = Client::<F>::ok(resp, "reading a pull request")?;
		Ok(Some(pull_from(&resp.json()?).ok_or(ForgeError::Decode(
			"a pull request was expected",
		))?))
	}

	/// `commits/{sha}/pull` is singular and answers with the pull request the commit was *merged*
	/// by (the source looks the commit up among merge commits), so for an open pull request's head
	/// it is a 404, which is an empty list. The publisher then falls back to the branch.
	fn pulls_for_commit(&self, sha: &Sha) -> Result<Vec<Pull>, ForgeError> {
		let resp = self
			.client
			.request(&self.client.get(&format!("commits/{}/pull", sha.as_str()))?)?;
		if resp.status == 404 {
			return Ok(Vec::new());
		}
		let resp = Client::<F>::ok(resp, "reading a commit's pull request")?;
		let pull =
			pull_from(&resp.json()?).ok_or(ForgeError::Decode("a pull request was expected"))?;
		Ok(vec![pull])
	}

	/// Open pull requests from `owner`'s branch `branch`. Forgejo filters by the branch's name
	/// alone, so the head repository's owner is checked here.
	fn pulls_by_head(&self, owner: &str, branch: &str) -> Result<Vec<Pull>, ForgeError> {
		let req = self
			.client
			.get("pulls")?
			.with_query("state", "open")
			.with_query("head", branch);
		let found = self.client.paged(
			req,
			Paging::Forgejo,
			None,
			"listing pull requests by head",
			pull_from,
		)?;
		Ok(found
			.into_iter()
			.filter(|p| {
				p.head_repo
					.as_deref()
					.and_then(|r| r.split('/').next())
					.is_some_and(|o| o.eq_ignore_ascii_case(owner))
			})
			.collect())
	}

	/// The paths of a commit's tree, a page of [`TREE_PER_PAGE`] entries at a time.
	///
	/// Forgejo's `truncated` is true on *every* page of a tree with more entries than a page,
	/// the last included (`services/repository/files/tree.go`), so it does not say when to stop.
	/// A page past the end is empty. The loop stops at an empty page, at `truncated: false`, or
	/// when it has seen `total_count` entries, so a tree of 2 500 entries takes three requests.
	/// Only if the page limit is reached with entries still to come is the result `truncated`.
	fn tree_paths(&self, sha: &Sha) -> Result<Tree, ForgeError> {
		let mut paths = BTreeSet::new();
		let mut seen = 0u64;
		for page in 1..=MAX_TREE_PAGES {
			let req = self
				.client
				.get(&format!("git/trees/{}", sha.as_str()))?
				.with_query("recursive", "true")
				.with_query("per_page", TREE_PER_PAGE.to_string())
				.with_query("page", page.to_string());
			let resp = Client::<F>::ok(self.client.request(&req)?, "reading a tree")?;
			let body = resp.json()?;
			drop(resp);
			let entries: &[Value] = match body.get("tree") {
				None | Some(Value::Null) => &[],
				Some(t) => t
					.as_array()
					.map(Vec::as_slice)
					.ok_or(ForgeError::Decode("a tree was expected"))?,
			};
			seen = seen.saturating_add(entries.len() as u64);
			paths.extend(
				entries
					.iter()
					.filter(|e| e.get("type").and_then(Value::as_str) == Some("blob"))
					.filter_map(|e| e.get("path").and_then(Value::as_str).map(str::to_owned)),
			);
			let more = body
				.get("truncated")
				.and_then(Value::as_bool)
				.unwrap_or(false);
			let total = body.get("total_count").and_then(Value::as_u64);
			if entries.is_empty() || !more || total.is_some_and(|t| seen >= t) {
				return Ok(Tree {
					paths,
					truncated: false,
				});
			}
		}
		Ok(Tree {
			paths,
			truncated: true,
		})
	}

	fn upsert_result(&self, spec: &ResultSpec<'_>) -> Result<ResultWritten, ForgeError> {
		match spec {
			ResultSpec::Status(status) => self.upsert_status(status),
			ResultSpec::Check(_) => Err(ForgeError::Refused(
				"Forgejo has commit statuses, not check runs",
			)),
		}
	}

	/// Keeps one sticky comment per pull request and key; see [`upsert_sticky`]. Ours are the
	/// comments of the token's identity carrying the key's marker.
	fn upsert_comment(&self, spec: &CommentSpec<'_>) -> Result<CommentResult, ForgeError> {
		upsert_sticky(self, spec)
	}
}
