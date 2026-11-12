// GitHub as a `Host`: reads, check runs and the sticky comment, over a `Client`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! GitHub's routes and shapes behind [`Host`]: artefacts, pull requests and the commit's tree to
//! read, a check run per section (found by `external_id` and by app, annotations 50 to a request)
//! and the sticky comment (found by marker and by author). Spacing, retries and paging are the
//! [`Client`]'s.

use std::time::Duration;

use pr_results_core::{CommentKey, Marker, RepoSlug, RunId, Sha};
use pr_results_render::{Annotation, Conclusion, Level};
use serde_json::{json, Value};

use crate::api::{ApiRequest, Forge, ForgeError, Method};
use crate::client::{Client, Paging, BACKOFF_S};
use crate::host::{
	comment_from, pull_from, Artifact, CheckResult, CheckSpec, Comment, CommentResult, CommentSpec,
	Host, HostKind, Pull, ResultSpec, ResultWritten, Tree,
};
use crate::sticky::{upsert_sticky, Sticky};

/// The login of `GITHUB_TOKEN`'s identity, whose comments are the only ones taken for ours.
pub const DEFAULT_BOT_LOGIN: &str = "github-actions[bot]";

/// The app that `GITHUB_TOKEN` acts as.
pub const DEFAULT_CHECK_APP: &str = "github-actions";

/// How many times a create is repeated after it was confirmed not to have happened.
const CREATE_ATTEMPTS: usize = 3;
const ANNOTATION_BATCH: usize = 50;

/// GitHub's REST API over a [`Client`]: check runs for the sections and an issue comment, found
/// by the marker and by author.
pub struct GitHub<F: Forge> {
	client: Client<F>,
	bot: String,
	/// The app whose check runs are ours.
	check_app: String,
}

impl<F: Forge> GitHub<F> {
	pub fn new(forge: F, repo: RepoSlug, bot_login: &str) -> Self {
		Self {
			client: Client::new(forge, repo),
			bot: bot_login.to_owned(),
			check_app: DEFAULT_CHECK_APP.to_owned(),
		}
	}

	/// The app slug whose check runs are ours (`github-actions` for `GITHUB_TOKEN`). A check run
	/// with our name and `external_id` from another app is not touched.
	pub fn with_check_app(mut self, slug: &str) -> Self {
		self.check_app = slug.to_owned();
		self
	}

	/// How many seconds of waiting, in total, one publish may spend on retries before it stops
	/// retrying and reports what it has. The default is [`RETRY_BUDGET_S`].
	pub fn with_retry_budget(mut self, seconds: u64) -> Self {
		self.client = self.client.with_retry_budget(seconds);
		self
	}

	pub fn forge(&self) -> &F {
		self.client.forge()
	}

	// ---- check runs --------------------------------------------------------------------------

	/// Our check run for the spec: its id and how many annotations it already has. It must have
	/// our `external_id` and have been made by our app, so another tool's check with the same
	/// name and a copied `external_id` is not taken for ours.
	fn find_check(&self, spec: &CheckSpec<'_>) -> Result<Option<(u64, usize)>, ForgeError> {
		let req = self
			.client
			.get(&format!("commits/{}/check-runs", spec.head_sha.as_str()))?
			.with_query("check_name", spec.name)
			.with_query("filter", "latest");
		let items = self.client.paged(
			req,
			Paging::GitHub,
			Some("check_runs"),
			"listing check runs",
			|c| {
				let ours = c.get("external_id").and_then(Value::as_str) == Some(spec.external_id)
					&& c.get("app")
						.and_then(|a| a.get("slug"))
						.and_then(Value::as_str)
						== Some(self.check_app.as_str());
				ours.then(|| {
					let count = c
						.get("output")
						.and_then(|o| o.get("annotations_count"))
						.and_then(Value::as_u64)
						.unwrap_or(0);
					Some((
						c.get("id")?.as_u64()?,
						usize::try_from(count).unwrap_or(usize::MAX),
					))
				})
				.flatten()
			},
		)?;
		Ok(items.first().copied())
	}

	/// How many annotations the check run has now.
	fn annotation_count(&self, id: u64) -> Result<usize, ForgeError> {
		let resp = Client::<F>::ok(
			self.client
				.request(&self.client.get(&format!("check-runs/{id}"))?)?,
			"reading a check run",
		)?;
		let count = resp
			.json()?
			.get("output")
			.and_then(|o| o.get("annotations_count"))
			.and_then(Value::as_u64)
			.unwrap_or(0);
		Ok(usize::try_from(count).unwrap_or(usize::MAX))
	}

	/// Creates or updates the check run for `spec.external_id` on the commit, then adds its
	/// annotations 50 to a request, each in a request of its own.
	///
	/// GitHub appends annotations, so a request that carries them is never repeated blindly: the
	/// run itself is written without any, and after a batch fails in a way that may have applied
	/// it the run's annotation count is read back and the batch is sent again only if it did not
	/// land. A run that already has annotations (from an earlier publish of the same commit) is
	/// sent only those beyond what it has, so a re-run does not duplicate them. A batch GitHub
	/// refuses with 422 is left out and counted. A 422 on the run itself comes back as an error,
	/// for the caller to retry with less.
	pub(crate) fn upsert_check(&self, spec: &CheckSpec<'_>) -> Result<CheckResult, ForgeError> {
		let existing = self.find_check(spec)?;
		let annotations: Vec<Value> = spec
			.output
			.annotations
			.iter()
			.map(annotation_json)
			.collect();
		let (id, created) = self.write_check(spec, existing.map(|(id, _)| id))?;
		let mut landed = if created {
			0
		} else {
			existing.map_or(0, |(_, n)| n)
		};
		let mut rejected = 0usize;
		let mut sent = 0usize;
		let mut sent_batches = 0usize;
		for batch in annotations
			.get(landed..)
			.unwrap_or(&[])
			.chunks(ANNOTATION_BATCH)
		{
			match self.send_batch(spec, id, batch, landed)? {
				true => {
					landed += batch.len();
					sent += batch.len();
					sent_batches += 1;
				}
				false => rejected += batch.len(),
			}
		}
		Ok(CheckResult {
			id,
			created,
			annotations_sent: sent,
			annotation_batches: sent_batches,
			annotations_rejected: rejected,
		})
	}

	/// Adds one batch of annotations to a check run that has `landed` already. `Ok(true)` when it
	/// is there, `Ok(false)` when GitHub refused it with 422.
	fn send_batch(
		&self,
		spec: &CheckSpec<'_>,
		id: u64,
		batch: &[Value],
		landed: usize,
	) -> Result<bool, ForgeError> {
		let body = check_body(spec, false, batch);
		let req = self
			.client
			.json_request(Method::Patch, &format!("check-runs/{id}"), &body)?;
		for attempt in 0..CREATE_ATTEMPTS {
			match self.client.request_with(&req, false) {
				Ok(resp) if resp.ok() => return Ok(true),
				Ok(resp) if resp.status == 422 => return Ok(false),
				Ok(resp) if resp.status < 500 => {
					return Err(ForgeError::Status {
						status: resp.status,
						op: "adding annotations",
					})
				}
				Ok(_) | Err(ForgeError::Transport(_)) => {}
				Err(e) => return Err(e),
			}
			// It may have been applied before the failure: read the count back before repeating.
			let wait = BACKOFF_S.get(attempt).copied().unwrap_or(1);
			if !self.client.may_wait(wait) {
				break;
			}
			self.client.forge().sleep(Duration::from_secs(wait));
			if self.annotation_count(id)? >= landed.saturating_add(batch.len()) {
				return Ok(true);
			}
		}
		Err(ForgeError::Status {
			status: 502,
			op: "adding annotations",
		})
	}

	fn write_check(
		&self,
		spec: &CheckSpec<'_>,
		existing: Option<u64>,
	) -> Result<(u64, bool), ForgeError> {
		if let Some(id) = existing {
			let body = check_body(spec, false, &[]);
			let req =
				self.client
					.json_request(Method::Patch, &format!("check-runs/{id}"), &body)?;
			let resp = self.client.request(&req)?;
			if resp.status != 404 {
				return Client::<F>::ok(resp, "updating a check run").map(|_| (id, false));
			}
			// It was deleted since it was listed: make it again.
		}
		let body = check_body(spec, true, &[]);
		let req = self
			.client
			.json_request(Method::Post, "check-runs", &body)?;
		for attempt in 0..CREATE_ATTEMPTS {
			match self.client.request(&req) {
				Ok(resp) if resp.ok() => {
					let id = resp
						.json()?
						.get("id")
						.and_then(Value::as_u64)
						.ok_or(ForgeError::Decode("a check run was expected"))?;
					return Ok((id, true));
				}
				Ok(resp) if resp.status < 500 => {
					return Err(ForgeError::Status {
						status: resp.status,
						op: "creating a check run",
					})
				}
				Ok(_) | Err(ForgeError::Transport(_)) => {}
				Err(e) => return Err(e),
			}
			// The create may have been applied before the failure: look before repeating it.
			let wait = BACKOFF_S.get(attempt).copied().unwrap_or(1);
			if !self.client.may_wait(wait) {
				break;
			}
			self.client.forge().sleep(Duration::from_secs(wait));
			if let Some((id, _)) = self.find_check(spec)? {
				return Ok((id, true));
			}
		}
		Err(ForgeError::Status {
			status: 502,
			op: "creating a check run",
		})
	}

	// ---- the sticky comment ------------------------------------------------------------------

	/// The comments of a pull request, each reduced to what finding ours needs: the id, the
	/// login, the account type, when it was made and the first `BODY_PREFIX_BYTES` bytes of the
	/// body (the marker is on its first line). A hundred pages of large comments is therefore a
	/// few kilobytes, not hundreds of megabytes.
	/// Public for the tests, which read the listing the flow sees; the flow itself goes through
	/// [`Host::upsert_comment`].
	pub fn comments(&self, pr: u64) -> Result<Vec<Comment>, ForgeError> {
		let req = self.client.get(&format!("issues/{pr}/comments"))?;
		self.client
			.paged(req, Paging::GitHub, None, "listing comments", comment_from)
	}

	/// The bot's comments for `key`, oldest first. A user's comment that copies the marker is not
	/// ours: the author is checked as well as the marker.
	fn ours_of(&self, comments: Vec<Comment>, key: &CommentKey) -> Vec<Comment> {
		let mut ours: Vec<Comment> = comments
			.into_iter()
			.filter(|c| c.login == self.bot && Marker::prefix_matches(&c.body, key))
			.collect();
		ours.sort_by_key(|c| c.id);
		ours
	}

	/// `Ok(false)` when the comment is gone.
	fn patch_comment(&self, id: u64, body: &str) -> Result<bool, ForgeError> {
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

	fn create_comment(&self, spec: &CommentSpec<'_>) -> Result<u64, ForgeError> {
		let req = self.client.json_request(
			Method::Post,
			&format!("issues/{}/comments", spec.pr),
			&json!({ "body": spec.body }),
		)?;
		for attempt in 0..CREATE_ATTEMPTS {
			match self.client.request(&req) {
				Ok(resp) if resp.ok() => {
					return resp
						.json()?
						.get("id")
						.and_then(Value::as_u64)
						.ok_or(ForgeError::Decode("a comment was expected"));
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
			let ours = self.ours_of(self.comments(spec.pr)?, &spec.marker.key);
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

	/// Deletes comments, ignoring those that are already gone. Returns how many were removed.
	fn remove(&self, ids: &mut dyn Iterator<Item = u64>) -> usize {
		let mut removed = 0;
		for id in ids {
			let deleted = self
				.client
				.path(&format!("issues/comments/{id}"))
				.map(|p| ApiRequest {
					method: Method::Delete,
					path: p,
					query: Vec::new(),
					body: None,
				})
				.and_then(|r| self.client.request(&r))
				.is_ok_and(|r| r.ok());
			removed += usize::from(deleted);
		}
		removed
	}
}

impl<F: Forge> Host for GitHub<F> {
	fn kind(&self) -> HostKind {
		HostKind::GitHub
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

	fn artifacts(&self, run: RunId) -> Result<Vec<Artifact>, ForgeError> {
		let req = self
			.client
			.get(&format!("actions/runs/{}/artifacts", run.0))?;
		self.client.paged(
			req,
			Paging::GitHub,
			Some("artifacts"),
			"listing artefacts",
			|a| {
				Some(Artifact {
					id: a.get("id")?.as_u64()?,
					name: a.get("name")?.as_str()?.to_owned(),
					size_in_bytes: a.get("size_in_bytes")?.as_u64()?,
					expired: a.get("expired").and_then(Value::as_bool).unwrap_or(false),
				})
			},
		)
	}

	/// Downloads an artefact's zip: the API answers with a redirect to a presigned URL, which is
	/// fetched by the forge without credentials.
	fn download(&self, artifact: &Artifact, max_bytes: u64) -> Result<Vec<u8>, ForgeError> {
		let req = self
			.client
			.get(&format!("actions/artifacts/{}/zip", artifact.id))?;
		let resp = self.client.request(&req)?;
		match (resp.status, &resp.location) {
			(301 | 302 | 303 | 307 | 308, Some(url)) => {
				self.client.forge().fetch_blob(url, max_bytes)
			}
			(301 | 302 | 303 | 307 | 308, None) => Err(ForgeError::Refused(
				"the redirect does not name an https URL",
			)),
			_ => Err(ForgeError::Status {
				status: resp.status,
				op: "downloading an artefact",
			}),
		}
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

	fn pulls_for_commit(&self, sha: &Sha) -> Result<Vec<Pull>, ForgeError> {
		let req = self
			.client
			.get(&format!("commits/{sha}/pulls", sha = sha.as_str()))?;
		self.client.paged(
			req,
			Paging::GitHub,
			None,
			"listing a commit's pull requests",
			pull_from,
		)
	}

	/// Open pull requests from `owner`'s branch `branch`, which is how a fork's pull request is
	/// found when the other two routes miss it.
	fn pulls_by_head(&self, owner: &str, branch: &str) -> Result<Vec<Pull>, ForgeError> {
		let req = self
			.client
			.get("pulls")?
			.with_query("head", format!("{owner}:{branch}"))
			.with_query("state", "open");
		self.client.paged(
			req,
			Paging::GitHub,
			None,
			"listing pull requests by head",
			pull_from,
		)
	}

	fn tree_paths(&self, sha: &Sha) -> Result<Tree, ForgeError> {
		let req = self
			.client
			.get(&format!("git/trees/{}", sha.as_str()))?
			.with_query("recursive", "1");
		let resp = Client::<F>::ok(self.client.request(&req)?, "reading a tree")?;
		let body = resp.json()?;
		let entries = body
			.get("tree")
			.and_then(Value::as_array)
			.ok_or(ForgeError::Decode("a tree was expected"))?;
		Ok(Tree {
			paths: entries
				.iter()
				.filter(|e| e.get("type").and_then(Value::as_str) == Some("blob"))
				.filter_map(|e| e.get("path").and_then(Value::as_str).map(str::to_owned))
				.collect(),
			truncated: body
				.get("truncated")
				.and_then(Value::as_bool)
				.unwrap_or(false),
		})
	}

	fn upsert_result(&self, spec: &ResultSpec<'_>) -> Result<ResultWritten, ForgeError> {
		match spec {
			ResultSpec::Check(check) => self.upsert_check(check),
			ResultSpec::Status(_) => Err(ForgeError::Refused(
				"GitHub has check runs, not commit statuses",
			)),
		}
	}

	/// Keeps one sticky comment per pull request and key; see [`upsert_sticky`]. Ours are the
	/// bot's comments carrying the key's marker.
	fn upsert_comment(&self, spec: &CommentSpec<'_>) -> Result<CommentResult, ForgeError> {
		upsert_sticky(self, spec)
	}
}

impl<F: Forge> Sticky for GitHub<F> {
	fn ours(&self, pr: u64, key: &CommentKey) -> Result<Vec<Comment>, ForgeError> {
		Ok(self.ours_of(self.comments(pr)?, key))
	}

	fn patch(&self, id: u64, body: &str) -> Result<bool, ForgeError> {
		self.patch_comment(id, body)
	}

	fn create(&self, spec: &CommentSpec<'_>) -> Result<u64, ForgeError> {
		self.create_comment(spec)
	}

	fn remove(&self, ids: &mut dyn Iterator<Item = u64>) -> usize {
		GitHub::remove(self, ids)
	}
}

fn annotation_json(a: &Annotation) -> Value {
	json!({
		"path": a.path.as_str(),
		"start_line": a.start_line,
		"end_line": a.end_line,
		"annotation_level": match a.level {
			Level::Notice => "notice",
			Level::Warning => "warning",
			Level::Failure => "failure",
		},
		"title": a.title.as_str(),
		"message": a.message.as_str(),
	})
}

/// The body of a create or an update, with one batch of annotations.
fn check_body(spec: &CheckSpec<'_>, create: bool, batch: &[Value]) -> Value {
	let o = spec.output;
	let mut output = json!({
		"title": o.title.as_str(),
		"summary": o.summary.as_str(),
		"text": o.text.as_str(),
	});
	if let (false, Some(o)) = (batch.is_empty(), output.as_object_mut()) {
		o.insert("annotations".to_owned(), Value::Array(batch.to_vec()));
	}
	let mut body = json!({
		"status": "completed",
		"conclusion": match o.conclusion {
			Conclusion::Success => "success",
			Conclusion::Failure => "failure",
			Conclusion::Neutral => "neutral",
		},
		"external_id": spec.external_id,
		"output": output,
	});
	if let (true, Some(o)) = (create, body.as_object_mut()) {
		o.insert("name".to_owned(), Value::from(spec.name));
		o.insert("head_sha".to_owned(), Value::from(spec.head_sha.as_str()));
	}
	body
}
