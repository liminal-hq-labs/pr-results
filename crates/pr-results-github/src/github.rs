// The GitHub operations the publisher needs, over any `Forge`: reads, check runs and the comment.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Every call goes through [`GitHub::request`], which spaces writes, follows `Link` headers for
//! paging, and retries what is safe to retry: a read or an idempotent write on a server error or
//! a dropped connection, and any call GitHub refused before acting on it (429 and the rate
//! limits). A POST that may have been applied is never repeated blindly: the caller looks for
//! what it would have created first.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::time::Duration;

use pr_results_core::{CommentKey, Marker, RepoSlug, RunId, Sha};
use pr_results_render::{Annotation, CheckOutput, Conclusion, Level};
use serde_json::{json, Value};

use crate::api::{ApiPath, ApiRequest, ApiResponse, Forge, ForgeError, Method};

/// The login of `GITHUB_TOKEN`'s identity, whose comments are the only ones taken for ours.
pub const DEFAULT_BOT_LOGIN: &str = "github-actions[bot]";

/// The default total time one publish may spend waiting to retry.
pub const RETRY_BUDGET_S: u64 = 600;
/// The app that `GITHUB_TOKEN` acts as.
pub const DEFAULT_CHECK_APP: &str = "github-actions";

/// Writes are at least this far apart, which stays well inside the secondary rate limit.
const WRITE_GAP: Duration = Duration::from_millis(500);
const MAX_PAGES: usize = 100;
const PER_PAGE: &str = "100";
/// The pause before each retry of a server error.
const BACKOFF_S: [u64; 4] = [1, 2, 4, 8];
/// The longest wait a rate limit may ask for.
const MAX_WAIT_S: u64 = 60;
/// How many times a create is repeated after it was confirmed not to have happened.
const CREATE_ATTEMPTS: usize = 3;
const ANNOTATION_BATCH: usize = 50;

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

/// The first `BODY_PREFIX_BYTES` bytes of `s`, cut at a character boundary.
fn prefix(s: &str) -> String {
	let mut end = s.len().min(BODY_PREFIX_BYTES);
	while !s.is_char_boundary(end) {
		end -= 1;
	}
	s.get(..end).unwrap_or("").to_owned()
}

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

pub struct GitHub<F: Forge> {
	forge: F,
	repo: RepoSlug,
	bot: String,
	/// The app whose check runs are ours.
	check_app: String,
	writes: Cell<u64>,
	/// When this run of the publisher began, by the forge's clock.
	started: Cell<Option<u64>>,
	retry_budget_s: u64,
	budget_spent: Cell<bool>,
}

impl<F: Forge> GitHub<F> {
	pub fn new(forge: F, repo: RepoSlug, bot_login: &str) -> Self {
		Self {
			forge,
			repo,
			bot: bot_login.to_owned(),
			check_app: DEFAULT_CHECK_APP.to_owned(),
			writes: Cell::new(0),
			started: Cell::new(None),
			retry_budget_s: RETRY_BUDGET_S,
			budget_spent: Cell::new(false),
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
		self.retry_budget_s = seconds;
		self
	}

	/// Starts the clock of the retry budget. `publish` calls it first.
	pub fn begin_run(&self) {
		self.started.set(Some(self.forge.now_epoch()));
		self.budget_spent.set(false);
	}

	/// Whether a retry was refused because the budget for waiting was used up.
	pub fn retry_budget_spent(&self) -> bool {
		self.budget_spent.get()
	}

	/// Whether waiting `wait` more seconds stays inside the budget; if not, says so for the report.
	fn may_wait(&self, wait: u64) -> bool {
		let now = self.forge.now_epoch();
		let started = self.started.get().unwrap_or_else(|| {
			self.started.set(Some(now));
			now
		});
		let ok = now.saturating_sub(started).saturating_add(wait) <= self.retry_budget_s;
		if !ok {
			self.budget_spent.set(true);
		}
		ok
	}

	pub fn forge(&self) -> &F {
		&self.forge
	}

	pub fn repo(&self) -> &RepoSlug {
		&self.repo
	}

	fn path(&self, tail: &str) -> Result<ApiPath, ForgeError> {
		ApiPath::parse(&format!("/repos/{}/{tail}", self.repo))
			.ok_or(ForgeError::Decode("a request path could not be built"))
	}

	fn get(&self, tail: &str) -> Result<ApiRequest, ForgeError> {
		Ok(ApiRequest::get(self.path(tail)?))
	}

	fn json_request(
		&self,
		method: Method,
		tail: &str,
		body: &Value,
	) -> Result<ApiRequest, ForgeError> {
		Ok(ApiRequest::json(method, self.path(tail)?, body))
	}

	/// Sends `req` with spacing between writes and the retries described above. A response with a
	/// failing status that is not worth retrying comes back as `Ok`, for the caller to judge.
	fn request(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		self.request_with(req, req.method.is_idempotent())
	}

	/// The same, for a call that is `idempotent` or not whatever its verb: a PATCH that appends
	/// annotations is not, and is not repeated after a failure that may have applied it.
	fn request_with(&self, req: &ApiRequest, idempotent: bool) -> Result<ApiResponse, ForgeError> {
		if req.method.is_write() {
			if self.writes.get() > 0 {
				self.forge.sleep(WRITE_GAP);
			}
			self.writes.set(self.writes.get().saturating_add(1));
		}
		let mut attempt = 0usize;
		loop {
			let resp = match self.forge.send(req) {
				Ok(r) => r,
				Err(e @ ForgeError::Transport(_)) if idempotent => {
					let Some(s) = BACKOFF_S.get(attempt) else {
						return Err(e);
					};
					if !self.may_wait(*s) {
						return Err(e);
					}
					self.forge.sleep(Duration::from_secs(*s));
					attempt += 1;
					continue;
				}
				Err(e) => return Err(e),
			};
			let wait = match self.wait_for(&resp, idempotent, attempt)? {
				Some(w) => w,
				None => return Ok(resp),
			};
			if attempt >= BACKOFF_S.len() || !self.may_wait(wait) {
				return Ok(resp);
			}
			self.forge.sleep(Duration::from_secs(wait));
			attempt += 1;
		}
	}

	/// How long to wait before repeating `req`, or `None` when the response is final. A rate limit
	/// that will not lift within a minute is an error.
	fn wait_for(
		&self,
		resp: &ApiResponse,
		idempotent: bool,
		attempt: usize,
	) -> Result<Option<u64>, ForgeError> {
		let backoff = BACKOFF_S.get(attempt).copied().unwrap_or(MAX_WAIT_S);
		let limited = matches!(resp.status, 403 | 429);
		if limited && resp.rate_remaining == Some(0) {
			// The primary limit: the request was not acted on, and the reset time is exact.
			let wait = resp
				.rate_reset
				.unwrap_or(0)
				.saturating_sub(self.forge.now_epoch());
			if wait > MAX_WAIT_S {
				return Err(ForgeError::RateLimited { wait_s: wait });
			}
			return Ok(Some(wait.max(1)));
		}
		let secondary = resp.status == 429
			|| (resp.status == 403
				&& (resp.retry_after_s.is_some()
					|| String::from_utf8_lossy(&resp.body)
						.to_ascii_lowercase()
						.contains("rate limit")));
		if secondary {
			return match resp.retry_after_s {
				Some(s) if s > MAX_WAIT_S => Err(ForgeError::RateLimited { wait_s: s }),
				Some(s) => Ok(Some(s.max(1))),
				None => Ok(Some(backoff)),
			};
		}
		if resp.status >= 500 && idempotent {
			return match resp.retry_after_s {
				Some(s) if s > MAX_WAIT_S => Err(ForgeError::RateLimited { wait_s: s }),
				Some(s) => Ok(Some(s.max(backoff))),
				None => Ok(Some(backoff)),
			};
		}
		Ok(None)
	}

	fn ok(resp: ApiResponse, op: &'static str) -> Result<ApiResponse, ForgeError> {
		if resp.ok() {
			Ok(resp)
		} else {
			Err(ForgeError::Status {
				status: resp.status,
				op,
			})
		}
	}

	/// Every item of a list, following `Link` headers for at most 100 pages. `key` names the
	/// array when the list is wrapped, as the artefacts are. Each item is reduced by `keep` as its
	/// page is read and the page is dropped, so what is held is what the caller asked to keep and
	/// not a hundred pages of other people's text.
	fn paged<T>(
		&self,
		first: ApiRequest,
		key: Option<&str>,
		op: &'static str,
		keep: impl Fn(&Value) -> Option<T>,
	) -> Result<Vec<T>, ForgeError> {
		let mut items = Vec::new();
		let mut req = first;
		for _ in 0..MAX_PAGES {
			let resp = Self::ok(self.request(&req)?, op)?;
			let next = resp.next.clone();
			let body = resp.json()?;
			drop(resp);
			let page = match key {
				Some(k) => body.get(k),
				None => Some(&body),
			};
			let page = page
				.and_then(Value::as_array)
				.ok_or(ForgeError::Decode("a list was expected"))?;
			items.extend(page.iter().filter_map(&keep));
			match next {
				Some(next) => req = ApiRequest::get(next),
				None => return Ok(items),
			}
		}
		Err(ForgeError::Decode("a list has more than 100 pages"))
	}

	// ---- reads -------------------------------------------------------------------------------

	pub fn artifacts(&self, run: RunId) -> Result<Vec<Artifact>, ForgeError> {
		let req = self
			.get(&format!("actions/runs/{}/artifacts", run.0))?
			.with_query("per_page", PER_PAGE);
		self.paged(req, Some("artifacts"), "listing artefacts", |a| {
			Some(Artifact {
				id: a.get("id")?.as_u64()?,
				name: a.get("name")?.as_str()?.to_owned(),
				size_in_bytes: a.get("size_in_bytes")?.as_u64()?,
				expired: a.get("expired").and_then(Value::as_bool).unwrap_or(false),
			})
		})
	}

	/// Downloads an artefact's zip: the API answers with a redirect to a presigned URL, which is
	/// fetched by the forge without credentials.
	pub fn download(&self, artifact: &Artifact, max_bytes: u64) -> Result<Vec<u8>, ForgeError> {
		let req = self.get(&format!("actions/artifacts/{}/zip", artifact.id))?;
		let resp = self.request(&req)?;
		match (resp.status, &resp.location) {
			(301 | 302 | 303 | 307 | 308, Some(url)) => self.forge.fetch_blob(url, max_bytes),
			(301 | 302 | 303 | 307 | 308, None) => Err(ForgeError::Refused(
				"the redirect does not name an https URL",
			)),
			_ => Err(ForgeError::Status {
				status: resp.status,
				op: "downloading an artefact",
			}),
		}
	}

	pub fn pull(&self, number: u64) -> Result<Option<Pull>, ForgeError> {
		let resp = self.request(&self.get(&format!("pulls/{number}"))?)?;
		if resp.status == 404 {
			return Ok(None);
		}
		let resp = Self::ok(resp, "reading a pull request")?;
		Ok(Some(pull_from(&resp.json()?).ok_or(ForgeError::Decode(
			"a pull request was expected",
		))?))
	}

	pub fn pulls_for_commit(&self, sha: &Sha) -> Result<Vec<Pull>, ForgeError> {
		let req = self
			.get(&format!("commits/{sha}/pulls", sha = sha.as_str()))?
			.with_query("per_page", PER_PAGE);
		self.paged(req, None, "listing a commit's pull requests", pull_from)
	}

	/// Open pull requests from `owner`'s branch `branch`, which is how a fork's pull request is
	/// found when the other two routes miss it.
	pub fn pulls_by_head(&self, owner: &str, branch: &str) -> Result<Vec<Pull>, ForgeError> {
		let req = self
			.get("pulls")?
			.with_query("head", format!("{owner}:{branch}"))
			.with_query("state", "open")
			.with_query("per_page", PER_PAGE);
		self.paged(req, None, "listing pull requests by head", pull_from)
	}

	pub fn tree_paths(&self, sha: &Sha) -> Result<Tree, ForgeError> {
		let req = self
			.get(&format!("git/trees/{}", sha.as_str()))?
			.with_query("recursive", "1");
		let resp = Self::ok(self.request(&req)?, "reading a tree")?;
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

	// ---- check runs --------------------------------------------------------------------------

	/// Our check run for the spec: its id and how many annotations it already has. It must have
	/// our `external_id` and have been made by our app, so another tool's check with the same
	/// name and a copied `external_id` is not taken for ours.
	fn find_check(&self, spec: &CheckSpec<'_>) -> Result<Option<(u64, usize)>, ForgeError> {
		let req = self
			.get(&format!("commits/{}/check-runs", spec.head_sha.as_str()))?
			.with_query("check_name", spec.name)
			.with_query("filter", "latest")
			.with_query("per_page", PER_PAGE);
		let items = self.paged(req, Some("check_runs"), "listing check runs", |c| {
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
		})?;
		Ok(items.first().copied())
	}

	/// How many annotations the check run has now.
	fn annotation_count(&self, id: u64) -> Result<usize, ForgeError> {
		let resp = Self::ok(
			self.request(&self.get(&format!("check-runs/{id}"))?)?,
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
	pub fn upsert_check(&self, spec: &CheckSpec<'_>) -> Result<CheckResult, ForgeError> {
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
		let req = self.json_request(Method::Patch, &format!("check-runs/{id}"), &body)?;
		for attempt in 0..CREATE_ATTEMPTS {
			match self.request_with(&req, false) {
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
			if !self.may_wait(wait) {
				break;
			}
			self.forge.sleep(Duration::from_secs(wait));
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
			let req = self.json_request(Method::Patch, &format!("check-runs/{id}"), &body)?;
			let resp = self.request(&req)?;
			if resp.status != 404 {
				return Self::ok(resp, "updating a check run").map(|_| (id, false));
			}
			// It was deleted since it was listed: make it again.
		}
		let body = check_body(spec, true, &[]);
		let req = self.json_request(Method::Post, "check-runs", &body)?;
		for attempt in 0..CREATE_ATTEMPTS {
			match self.request(&req) {
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
			if !self.may_wait(wait) {
				break;
			}
			self.forge.sleep(Duration::from_secs(wait));
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
	pub fn comments(&self, pr: u64) -> Result<Vec<Comment>, ForgeError> {
		let req = self
			.get(&format!("issues/{pr}/comments"))?
			.with_query("per_page", PER_PAGE);
		self.paged(req, None, "listing comments", |c| {
			let user = c.get("user")?;
			Some(Comment {
				id: c.get("id")?.as_u64()?,
				login: user.get("login")?.as_str()?.to_owned(),
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
		})
	}

	/// The bot's comments for `key`, oldest first. A user's comment that copies the marker is not
	/// ours: the author is checked as well as the marker.
	fn ours(&self, comments: Vec<Comment>, key: &CommentKey) -> Vec<Comment> {
		let mut ours: Vec<Comment> = comments
			.into_iter()
			.filter(|c| c.login == self.bot && Marker::prefix_matches(&c.body, key))
			.collect();
		ours.sort_by_key(|c| c.id);
		ours
	}

	/// Keeps one sticky comment per pull request and key. It leaves the comment alone when one for
	/// the same commit from a later run is there, updates the oldest of the bot's comments for
	/// the key, and creates one when there is none, then deletes any other the bot wrote for the
	/// key so concurrent runs converge on one.
	pub fn upsert_comment(&self, spec: &CommentSpec<'_>) -> Result<CommentResult, ForgeError> {
		let key = &spec.marker.key;
		let ours = self.ours(self.comments(spec.pr)?, key);
		let newer = ours.iter().find(|c| {
			Marker::parse(&c.body)
				.is_some_and(|m| m.sha == spec.marker.sha && m.run > spec.marker.run)
		});
		if let Some(c) = newer {
			return Ok(CommentResult {
				id: c.id,
				action: CommentAction::SkippedNewer,
				duplicates_removed: 0,
			});
		}
		if let Some(oldest) = ours.first() {
			if self.patch_comment(oldest.id, spec.body)? {
				let removed = self.remove(ours.iter().skip(1).map(|c| c.id));
				return Ok(CommentResult {
					id: oldest.id,
					action: CommentAction::Updated,
					duplicates_removed: removed,
				});
			}
			// It was deleted since it was listed (404): fall through and create one.
		}
		let created = self.create_comment(spec)?;
		let ours = self.ours(self.comments(spec.pr)?, key);
		let Some(keep) = ours.first() else {
			return Ok(CommentResult {
				id: created,
				action: CommentAction::Created,
				duplicates_removed: 0,
			});
		};
		let removed = self.remove(ours.iter().skip(1).map(|c| c.id));
		if keep.id != created {
			// Another run's create landed first and ours was removed: its comment stays, and says
			// what this run found unless a later run for the same commit wrote it.
			let later = Marker::parse(&keep.body)
				.is_some_and(|m| m.sha == spec.marker.sha && m.run > spec.marker.run);
			if !later && !self.patch_comment(keep.id, spec.body)? {
				return Err(ForgeError::Status {
					status: 404,
					op: "updating a comment",
				});
			}
		}
		Ok(CommentResult {
			id: keep.id,
			action: CommentAction::Created,
			duplicates_removed: removed,
		})
	}

	/// `Ok(false)` when the comment is gone.
	fn patch_comment(&self, id: u64, body: &str) -> Result<bool, ForgeError> {
		let req = self.json_request(
			Method::Patch,
			&format!("issues/comments/{id}"),
			&json!({ "body": body }),
		)?;
		let resp = self.request(&req)?;
		if resp.status == 404 {
			return Ok(false);
		}
		Self::ok(resp, "updating a comment").map(|_| true)
	}

	fn create_comment(&self, spec: &CommentSpec<'_>) -> Result<u64, ForgeError> {
		let req = self.json_request(
			Method::Post,
			&format!("issues/{}/comments", spec.pr),
			&json!({ "body": spec.body }),
		)?;
		for attempt in 0..CREATE_ATTEMPTS {
			match self.request(&req) {
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
			if !self.may_wait(wait) {
				break;
			}
			self.forge.sleep(Duration::from_secs(wait));
			let ours = self.ours(self.comments(spec.pr)?, &spec.marker.key);
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
	fn remove(&self, ids: impl Iterator<Item = u64>) -> usize {
		let mut removed = 0;
		for id in ids {
			let deleted = self
				.path(&format!("issues/comments/{id}"))
				.map(|p| ApiRequest {
					method: Method::Delete,
					path: p,
					query: Vec::new(),
					body: None,
				})
				.and_then(|r| self.request(&r))
				.is_ok_and(|r| r.ok());
			removed += usize::from(deleted);
		}
		removed
	}
}

fn pull_from(v: &Value) -> Option<Pull> {
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
	})
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
