// An in-memory GitHub for tests: routes, paging, scripted faults and a record of what was sent.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`FakeForge`] follows GitHub's REST documentation for the dozen routes the publisher uses and
//! imposes the limits that matter: a comment body of at most 65 536 characters, a check run's
//! summary and text of at most 65 535, 50 annotations to a request, and a path in the commit's
//! tree for each annotation. Lists come back two items to a page with a `Link` header, so a
//! missed page is a failing test. It is cheap to clone, and the clones share their state, so a
//! test keeps a handle after giving one to [`crate::GitHub`].

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::api::{
	percent_decode, ApiPath, ApiRequest, ApiResponse, BlobUrl, Forge, ForgeError, Method,
};

const BLOB_HOST: &str = "https://blob.example.invalid/artifacts/";

/// A comment as GitHub keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeComment {
	pub id: u64,
	pub pr: u64,
	pub login: String,
	pub body: String,
}

/// A check run as GitHub keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct FakeCheck {
	pub id: u64,
	pub name: String,
	pub head_sha: String,
	pub external_id: Option<String>,
	pub conclusion: Option<String>,
	pub title: String,
	pub summary: String,
	pub text: String,
	/// Every annotation added so far, in the order they arrived.
	pub annotations: Vec<Value>,
	/// How many annotations each request that carried some brought.
	pub batches: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct FakePull {
	pub number: u64,
	pub open: bool,
	pub head_sha: String,
	pub head_ref: String,
	/// `None` for a fork that has been deleted.
	pub head_repo: Option<String>,
	pub base_repo: String,
	/// Whether `commits/{sha}/pulls` lists it. A pull request from a fork may be missing there.
	pub in_commit_listing: bool,
}

impl FakePull {
	pub fn same_repo(number: u64, repo: &str, head_sha: &str, branch: &str) -> Self {
		Self {
			number,
			open: true,
			head_sha: head_sha.to_owned(),
			head_ref: branch.to_owned(),
			head_repo: Some(repo.to_owned()),
			base_repo: repo.to_owned(),
			in_commit_listing: true,
		}
	}

	pub fn fork(number: u64, base: &str, fork: &str, head_sha: &str, branch: &str) -> Self {
		Self {
			head_repo: Some(fork.to_owned()),
			in_commit_listing: false,
			..Self::same_repo(number, base, head_sha, branch)
		}
	}
}

/// What a scripted fault does instead of answering.
#[derive(Debug, Clone)]
pub enum Fault {
	Status {
		status: u16,
		retry_after_s: Option<u64>,
		rate_remaining: Option<u64>,
		rate_reset: Option<u64>,
		body: String,
	},
	Transport(String),
	/// The request is carried out and then answered with this failing status, as when a
	/// connection or a gateway fails after GitHub acted.
	AppliedThenStatus(u16),
}

impl Fault {
	pub fn status(status: u16) -> Self {
		Self::Status {
			status,
			retry_after_s: None,
			rate_remaining: None,
			rate_reset: None,
			body: "{\"message\":\"scripted\"}".to_owned(),
		}
	}

	pub fn retry_after(status: u16, seconds: u64) -> Self {
		Self::Status {
			status,
			retry_after_s: Some(seconds),
			rate_remaining: None,
			rate_reset: None,
			body: "{\"message\":\"You have exceeded a secondary rate limit.\"}".to_owned(),
		}
	}

	/// A primary limit: 403 with nothing remaining, resetting at `reset`.
	pub fn primary_limit(reset: u64) -> Self {
		Self::Status {
			status: 403,
			retry_after_s: None,
			rate_remaining: Some(0),
			rate_reset: Some(reset),
			body: "{\"message\":\"API rate limit exceeded\"}".to_owned(),
		}
	}
}

struct Rule {
	method: Method,
	contains: String,
	remaining: usize,
	fault: Fault,
}

struct Artifact {
	id: u64,
	run: u64,
	name: String,
	zip: Vec<u8>,
	expired: bool,
}

#[derive(Debug, Clone, Copy)]
struct Limits {
	comment_chars: usize,
	check_chars: usize,
	annotations: usize,
}

/// Something to do once, right after a request has been answered.
type Hook = Box<dyn FnOnce(&FakeForge)>;

struct State {
	repo: String,
	bot: String,
	page_size: usize,
	now: u64,
	artifacts: Vec<Artifact>,
	pulls: Vec<FakePull>,
	comments: Vec<FakeComment>,
	checks: Vec<FakeCheck>,
	trees: BTreeMap<String, (Vec<String>, bool)>,
	next_id: u64,
	limits: Limits,
	reject_unknown_paths: bool,
	rules: Vec<Rule>,
	requests: Vec<ApiRequest>,
	blobs: Vec<String>,
	sleeps: Vec<Duration>,
	racer: Option<String>,
	hooks: Vec<(String, Option<Hook>)>,
}

#[derive(Clone)]
pub struct FakeForge {
	s: Rc<RefCell<State>>,
}

impl FakeForge {
	/// A repository `owner/name` with nothing in it. The bot that comments is
	/// `github-actions[bot]`.
	pub fn new(repo: &str) -> Self {
		Self {
			s: Rc::new(RefCell::new(State {
				repo: repo.to_owned(),
				bot: "github-actions[bot]".to_owned(),
				page_size: 2,
				now: 1_700_000_000,
				artifacts: Vec::new(),
				pulls: Vec::new(),
				comments: Vec::new(),
				checks: Vec::new(),
				trees: BTreeMap::new(),
				next_id: 1000,
				limits: Limits {
					comment_chars: 65_536,
					check_chars: 65_535,
					annotations: 50,
				},
				reject_unknown_paths: true,
				rules: Vec::new(),
				requests: Vec::new(),
				blobs: Vec::new(),
				sleeps: Vec::new(),
				racer: None,
				hooks: Vec::new(),
			})),
		}
	}

	fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
		f(&mut self.s.borrow_mut())
	}

	pub fn set_page_size(&self, n: usize) {
		self.with(|s| s.page_size = n.max(1));
	}

	pub fn set_now(&self, epoch: u64) {
		self.with(|s| s.now = epoch);
	}

	/// Sets the three limits GitHub imposes: the comment body, a check run's summary and text,
	/// and the annotations in one request. The defaults are 65 536, 65 535 and 50.
	pub fn set_limits(&self, comment_chars: usize, check_chars: usize, annotations: usize) {
		self.with(|s| {
			s.limits = Limits {
				comment_chars,
				check_chars,
				annotations,
			}
		});
	}

	/// Whether an annotation on a path outside a known tree makes the request fail with 422.
	/// This is an assumption about GitHub, which is why it can be switched off.
	pub fn set_reject_unknown_paths(&self, on: bool) {
		self.with(|s| s.reject_unknown_paths = on);
	}

	pub fn add_artifact(&self, run: u64, name: &str, zip: Vec<u8>) -> u64 {
		self.with(|s| {
			s.next_id += 1;
			let id = s.next_id;
			s.artifacts.push(Artifact {
				id,
				run,
				name: name.to_owned(),
				zip,
				expired: false,
			});
			id
		})
	}

	pub fn expire_artifact(&self, id: u64) {
		self.with(|s| {
			for a in s.artifacts.iter_mut().filter(|a| a.id == id) {
				a.expired = true;
			}
		});
	}

	pub fn add_pull(&self, pull: FakePull) {
		self.with(|s| s.pulls.push(pull));
	}

	pub fn move_head(&self, number: u64, sha: &str) {
		self.with(|s| {
			for p in s.pulls.iter_mut().filter(|p| p.number == number) {
				p.head_sha = sha.to_owned();
			}
		});
	}

	pub fn close_pull(&self, number: u64) {
		self.with(|s| {
			for p in s.pulls.iter_mut().filter(|p| p.number == number) {
				p.open = false;
			}
		});
	}

	pub fn set_tree(&self, sha: &str, paths: &[&str], truncated: bool) {
		self.with(|s| {
			s.trees.insert(
				sha.to_owned(),
				(paths.iter().map(|p| (*p).to_owned()).collect(), truncated),
			);
		});
	}

	/// A check run that this tool did not write, such as another workflow's.
	pub fn add_check(&self, name: &str, head_sha: &str, external_id: Option<&str>) -> u64 {
		self.with(|s| {
			s.next_id += 1;
			let id = s.next_id;
			s.checks.push(FakeCheck {
				id,
				name: name.to_owned(),
				head_sha: head_sha.to_owned(),
				external_id: external_id.map(str::to_owned),
				conclusion: Some("success".to_owned()),
				title: "other".to_owned(),
				summary: "other".to_owned(),
				text: String::new(),
				annotations: Vec::new(),
				batches: Vec::new(),
			});
			id
		})
	}

	pub fn add_comment(&self, pr: u64, login: &str, body: &str) -> u64 {
		self.with(|s| {
			s.next_id += 1;
			let id = s.next_id;
			s.comments.push(FakeComment {
				id,
				pr,
				login: login.to_owned(),
				body: body.to_owned(),
			});
			id
		})
	}

	/// The next comment created by the bot is preceded by one with `body`, from the bot, as if a
	/// concurrent run's create had landed first.
	pub fn race_on_next_comment_create(&self, body: &str) {
		self.with(|s| s.racer = Some(body.to_owned()));
	}

	/// Answers the next `times` requests with this `method` whose path and query contain
	/// `contains` with `fault`, in the order the rules were added.
	pub fn fail(&self, method: Method, contains: &str, times: usize, fault: Fault) {
		self.with(|s| {
			s.rules.push(Rule {
				method,
				contains: contains.to_owned(),
				remaining: times,
				fault,
			});
		});
	}

	/// Runs `action` once, right after the first request whose path and query contain `contains`
	/// has been answered, to change the world between two of the publisher's calls.
	pub fn after_request(&self, contains: &str, action: impl FnOnce(&FakeForge) + 'static) {
		self.with(|s| s.hooks.push((contains.to_owned(), Some(Box::new(action)))));
	}

	pub fn requests(&self) -> Vec<ApiRequest> {
		self.with(|s| s.requests.clone())
	}

	pub fn writes(&self) -> Vec<ApiRequest> {
		self.with(|s| {
			s.requests
				.iter()
				.filter(|r| r.method.is_write())
				.cloned()
				.collect()
		})
	}

	pub fn clear_requests(&self) {
		self.with(|s| s.requests.clear());
	}

	pub fn blob_fetches(&self) -> Vec<String> {
		self.with(|s| s.blobs.clone())
	}

	pub fn sleeps(&self) -> Vec<Duration> {
		self.with(|s| s.sleeps.clone())
	}

	pub fn comments(&self, pr: u64) -> Vec<FakeComment> {
		self.with(|s| s.comments.iter().filter(|c| c.pr == pr).cloned().collect())
	}

	pub fn check_runs(&self) -> Vec<FakeCheck> {
		self.with(|s| s.checks.clone())
	}
}

fn json_response(status: u16, body: &Value) -> ApiResponse {
	ApiResponse {
		status,
		body: body.to_string().into_bytes(),
		..ApiResponse::default()
	}
}

fn message(status: u16, text: &str) -> ApiResponse {
	json_response(status, &json!({ "message": text }))
}

fn chars(s: &str) -> usize {
	s.chars().count().max(s.encode_utf16().count())
}

fn parse_query(target: &str) -> Vec<(String, String)> {
	target
		.split_once('?')
		.map(|(_, q)| q)
		.unwrap_or("")
		.split('&')
		.filter(|p| !p.is_empty())
		.map(|p| {
			let (k, v) = p.split_once('=').unwrap_or((p, ""));
			(percent_decode(k), percent_decode(v))
		})
		.collect()
}

fn q<'a>(query: &'a [(String, String)], key: &str) -> Option<&'a str> {
	query
		.iter()
		.find(|(k, _)| k == key)
		.map(|(_, v)| v.as_str())
}

impl State {
	fn pull_json(p: &FakePull) -> Value {
		json!({
			"number": p.number,
			"state": if p.open { "open" } else { "closed" },
			"head": {
				"sha": p.head_sha,
				"ref": p.head_ref,
				"repo": p.head_repo.as_ref().map(|r| json!({ "full_name": r })),
			},
			"base": { "repo": { "full_name": p.base_repo } },
		})
	}

	/// One page of `items` (two to a page by default), with the `Link` to the next.
	fn page(
		&self,
		path: &str,
		query: &[(String, String)],
		items: Vec<Value>,
		wrap: Option<&str>,
	) -> ApiResponse {
		let page: usize = q(query, "page")
			.and_then(|p| p.parse().ok())
			.unwrap_or(1)
			.max(1);
		let size = self.page_size;
		let total = items.len();
		let start = (page - 1).saturating_mul(size);
		let slice: Vec<Value> = items.into_iter().skip(start).take(size).collect();
		let next = (start.saturating_add(size) < total).then(|| {
			let mut parts: Vec<String> = query
				.iter()
				.filter(|(k, _)| k != "page")
				.map(|(k, v)| {
					format!(
						"{}={}",
						crate::api::percent_encode(k),
						crate::api::percent_encode(v)
					)
				})
				.collect();
			parts.push(format!("page={}", page + 1));
			ApiPath::parse(&format!("{path}?{}", parts.join("&")))
		});
		let body = match wrap {
			Some(key) => json!({ "total_count": total, key: slice }),
			None => Value::Array(slice),
		};
		ApiResponse {
			next: next.flatten(),
			..json_response(200, &body)
		}
	}

	fn handle(&mut self, req: &ApiRequest) -> ApiResponse {
		let target = req.target();
		let path = req.path.path().to_owned();
		let query = parse_query(&target);
		let segs: Vec<&str> = path.split('/').skip(1).collect();
		let repo_parts: Vec<&str> = self.repo.split('/').collect();
		let rest: Vec<&str> = match segs.as_slice() {
			["repos", o, r, rest @ ..] if repo_parts == [*o, *r] => rest.to_vec(),
			_ => return message(404, "Not Found"),
		};
		let body: Option<Value> = req
			.body
			.as_deref()
			.and_then(|b| serde_json::from_slice(b).ok());
		match (req.method, rest.as_slice()) {
			(Method::Get, ["actions", "runs", run, "artifacts"]) => {
				let run: u64 = run.parse().unwrap_or(0);
				let items = self
					.artifacts
					.iter()
					.filter(|a| a.run == run)
					.map(|a| {
						json!({
							"id": a.id, "name": a.name, "size_in_bytes": a.zip.len(),
							"expired": a.expired, "workflow_run": { "id": a.run },
						})
					})
					.collect();
				self.page(&path, &query, items, Some("artifacts"))
			}
			(Method::Get, ["actions", "artifacts", id, "zip"]) => {
				let id: u64 = id.parse().unwrap_or(0);
				match self.artifacts.iter().find(|a| a.id == id) {
					None => message(404, "Not Found"),
					Some(a) if a.expired => message(410, "Gone"),
					Some(a) => ApiResponse {
						status: 302,
						location: BlobUrl::parse(&format!("{BLOB_HOST}{}", a.id)),
						..ApiResponse::default()
					},
				}
			}
			(Method::Get, ["pulls", n]) => {
				let n: u64 = n.parse().unwrap_or(0);
				match self.pulls.iter().find(|p| p.number == n) {
					Some(p) => json_response(200, &Self::pull_json(p)),
					None => message(404, "Not Found"),
				}
			}
			(Method::Get, ["pulls"]) => {
				let head = q(&query, "head").unwrap_or("");
				let (owner, branch) = head.split_once(':').unwrap_or(("", head));
				let items = self
					.pulls
					.iter()
					.filter(|p| p.open || q(&query, "state") == Some("all"))
					.filter(|p| {
						p.head_ref == branch
							&& p.head_repo
								.as_deref()
								.is_some_and(|r| r.split('/').next() == Some(owner))
					})
					.map(Self::pull_json)
					.collect();
				self.page(&path, &query, items, None)
			}
			(Method::Get, ["commits", sha, "pulls"]) => {
				let items = self
					.pulls
					.iter()
					.filter(|p| p.in_commit_listing && p.head_sha == *sha)
					.map(Self::pull_json)
					.collect();
				self.page(&path, &query, items, None)
			}
			(Method::Get, ["git", "trees", sha]) => match self.trees.get(*sha) {
				None => message(404, "Not Found"),
				Some((paths, truncated)) => {
					let tree: Vec<Value> = paths
						.iter()
						.map(|p| json!({ "path": p, "type": "blob" }))
						.collect();
					json_response(
						200,
						&json!({ "sha": sha, "tree": tree, "truncated": truncated }),
					)
				}
			},
			(Method::Get, ["issues", n, "comments"]) => {
				let n: u64 = n.parse().unwrap_or(0);
				let items = self
					.comments
					.iter()
					.filter(|c| c.pr == n)
					.map(|c| json!({ "id": c.id, "body": c.body, "user": { "login": c.login } }))
					.collect();
				self.page(&path, &query, items, None)
			}
			(Method::Post, ["issues", n, "comments"]) => {
				let n: u64 = n.parse().unwrap_or(0);
				let Some(text) = body
					.as_ref()
					.and_then(|b| b.get("body"))
					.and_then(Value::as_str)
				else {
					return message(422, "Validation Failed");
				};
				if chars(text) > self.limits.comment_chars {
					return json_response(
						422,
						&json!({
							"message": "Validation Failed",
							"errors": [{ "resource": "IssueComment", "code": "unprocessable",
								"field": "data", "message": "Body is too long" }],
						}),
					);
				}
				if let Some(other) = self.racer.take() {
					self.next_id += 1;
					let id = self.next_id;
					self.comments.push(FakeComment {
						id,
						pr: n,
						login: self.bot.clone(),
						body: other,
					});
				}
				self.next_id += 1;
				let id = self.next_id;
				self.comments.push(FakeComment {
					id,
					pr: n,
					login: self.bot.clone(),
					body: text.to_owned(),
				});
				json_response(
					201,
					&json!({ "id": id, "body": text, "user": { "login": self.bot } }),
				)
			}
			(Method::Patch, ["issues", "comments", id]) => {
				let id: u64 = id.parse().unwrap_or(0);
				let Some(text) = body
					.as_ref()
					.and_then(|b| b.get("body"))
					.and_then(Value::as_str)
				else {
					return message(422, "Validation Failed");
				};
				if chars(text) > self.limits.comment_chars {
					return message(422, "Validation Failed");
				}
				match self.comments.iter_mut().find(|c| c.id == id) {
					Some(c) => {
						c.body = text.to_owned();
						json_response(200, &json!({ "id": id, "body": text }))
					}
					None => message(404, "Not Found"),
				}
			}
			(Method::Delete, ["issues", "comments", id]) => {
				let id: u64 = id.parse().unwrap_or(0);
				let before = self.comments.len();
				self.comments.retain(|c| c.id != id);
				if self.comments.len() < before {
					ApiResponse {
						status: 204,
						..ApiResponse::default()
					}
				} else {
					message(404, "Not Found")
				}
			}
			(Method::Get, ["commits", sha, "check-runs"]) => {
				let name = q(&query, "check_name");
				let items = self
					.checks
					.iter()
					.filter(|c| c.head_sha == *sha && name.is_none_or(|n| n == c.name))
					.map(|c| {
						json!({
							"id": c.id, "name": c.name, "head_sha": c.head_sha,
							"external_id": c.external_id, "conclusion": c.conclusion,
							"app": { "slug": "github-actions" },
						})
					})
					.collect();
				self.page(&path, &query, items, Some("check_runs"))
			}
			(Method::Post, ["check-runs"]) => self.write_check(None, body.as_ref()),
			(Method::Patch, ["check-runs", id]) => {
				let id: u64 = id.parse().unwrap_or(0);
				if self.checks.iter().all(|c| c.id != id) {
					return message(404, "Not Found");
				}
				self.write_check(Some(id), body.as_ref())
			}
			_ => message(404, "Not Found"),
		}
	}

	/// Creates (`id` is `None`) or updates a check run, with GitHub's checks on the body.
	fn write_check(&mut self, id: Option<u64>, body: Option<&Value>) -> ApiResponse {
		let invalid = || message(422, "Validation Failed");
		let Some(body) = body else {
			return invalid();
		};
		let text = |k: &str| body.get(k).and_then(Value::as_str).map(str::to_owned);
		let output = body.get("output");
		let o_text = |k: &str| output.and_then(|o| o.get(k)).and_then(Value::as_str);
		if id.is_none() && (text("name").is_none() || text("head_sha").is_none()) {
			return invalid();
		}
		if output.is_some() && (o_text("title").is_none() || o_text("summary").is_none()) {
			return invalid();
		}
		for k in ["summary", "text"] {
			if o_text(k).is_some_and(|t| chars(t) > self.limits.check_chars) {
				return invalid();
			}
		}
		let annotations: Vec<Value> = output
			.and_then(|o| o.get("annotations"))
			.and_then(Value::as_array)
			.cloned()
			.unwrap_or_default();
		if annotations.len() > self.limits.annotations {
			return invalid();
		}
		let head = match id {
			Some(id) => self
				.checks
				.iter()
				.find(|c| c.id == id)
				.map(|c| c.head_sha.clone())
				.unwrap_or_default(),
			None => text("head_sha").unwrap_or_default(),
		};
		for a in &annotations {
			let ok = a.get("path").and_then(Value::as_str).is_some()
				&& a.get("start_line").and_then(Value::as_u64).is_some()
				&& a.get("end_line").and_then(Value::as_u64).is_some()
				&& a.get("message").and_then(Value::as_str).is_some()
				&& matches!(
					a.get("annotation_level").and_then(Value::as_str),
					Some("notice" | "warning" | "failure")
				);
			if !ok {
				return invalid();
			}
			if self.reject_unknown_paths {
				if let Some((paths, false)) = self.trees.get(&head) {
					let path = a.get("path").and_then(Value::as_str).unwrap_or("");
					if !paths.iter().any(|p| p == path) {
						return invalid();
					}
				}
			}
		}
		let slot = match id {
			Some(id) => self.checks.iter().position(|c| c.id == id),
			None => {
				self.next_id += 1;
				self.checks.push(FakeCheck {
					id: self.next_id,
					name: text("name").unwrap_or_default(),
					head_sha: head,
					external_id: None,
					conclusion: None,
					title: String::new(),
					summary: String::new(),
					text: String::new(),
					annotations: Vec::new(),
					batches: Vec::new(),
				});
				self.checks.len().checked_sub(1)
			}
		};
		let Some(check) = slot.and_then(|i| self.checks.get_mut(i)) else {
			return invalid();
		};
		if let Some(e) = text("external_id") {
			check.external_id = Some(e);
		}
		if let Some(c) = text("conclusion") {
			check.conclusion = Some(c);
		}
		if let Some(t) = o_text("title") {
			check.title = t.to_owned();
		}
		if let Some(t) = o_text("summary") {
			check.summary = t.to_owned();
		}
		if let Some(t) = o_text("text") {
			check.text = t.to_owned();
		}
		if !annotations.is_empty() {
			check.batches.push(annotations.len());
			check.annotations.extend(annotations);
		}
		let status = if id.is_some() { 200 } else { 201 };
		json_response(status, &json!({ "id": check.id, "name": check.name }))
	}
}

impl FakeForge {
	/// Records `req` and answers it: a scripted fault if one is due, else the routes.
	fn answer(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		self.with(|s| {
			s.requests.push(req.clone());
			let target = req.target();
			let hit = s.rules.iter_mut().find(|r| {
				r.remaining > 0 && r.method == req.method && target.contains(&r.contains)
			});
			if let Some(rule) = hit {
				rule.remaining -= 1;
				let fault = rule.fault.clone();
				return match fault {
					Fault::AppliedThenStatus(status) => {
						s.handle(req);
						Ok(message(status, "scripted, after the request was applied"))
					}
					Fault::Transport(why) => Err(ForgeError::Transport(why)),
					Fault::Status {
						status,
						retry_after_s,
						rate_remaining,
						rate_reset,
						body,
					} => Ok(ApiResponse {
						status,
						retry_after_s,
						rate_remaining,
						rate_reset,
						body: body.into_bytes(),
						..ApiResponse::default()
					}),
				};
			}
			Ok(s.handle(req))
		})
	}
}

impl Forge for FakeForge {
	fn send(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		let answer = self.answer(req);
		let target = req.target();
		let due: Vec<Hook> = self.with(|s| {
			s.hooks
				.iter_mut()
				.filter(|(contains, action)| action.is_some() && target.contains(contains.as_str()))
				.filter_map(|(_, action)| action.take())
				.collect()
		});
		for action in due {
			action(self);
		}
		answer
	}

	fn fetch_blob(&self, url: &BlobUrl, max_bytes: u64) -> Result<Vec<u8>, ForgeError> {
		self.with(|s| {
			s.blobs.push(url.as_str().to_owned());
			let id: u64 = url
				.as_str()
				.strip_prefix(BLOB_HOST)
				.and_then(|i| i.parse().ok())
				.ok_or(ForgeError::Status {
					status: 404,
					op: "fetching a blob",
				})?;
			let a = s
				.artifacts
				.iter()
				.find(|a| a.id == id)
				.ok_or(ForgeError::Status {
					status: 404,
					op: "fetching a blob",
				})?;
			if a.zip.len() as u64 > max_bytes {
				return Err(ForgeError::TooLarge { limit: max_bytes });
			}
			Ok(a.zip.clone())
		})
	}

	fn sleep(&self, d: Duration) {
		self.with(|s| {
			s.sleeps.push(d);
			s.now = s.now.saturating_add(d.as_secs());
		});
	}

	fn now_epoch(&self) -> u64 {
		self.with(|s| s.now)
	}
}
