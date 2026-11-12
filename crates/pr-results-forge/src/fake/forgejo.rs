// An in-memory Forgejo for tests: the routes the publisher uses, with Forgejo's shapes and quirks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`FakeForgejo`] is [`FakeForge`]'s store answering Forgejo's routes, with the shapes of the v17
//! Swagger (every key it writes is checked against the sourced fixtures in
//! `tests/fixtures/forgejo/`) and the behaviour that the source shows and GitHub does not have:
//!
//! - the comments of an issue come whole in one response with an `X-Total-Count`;
//! - the other lists come two to a page with a `Link` header and an `X-Total-Count`;
//! - a tree comes in pages of at most `per_page` entries, `truncated` is true on every page of a
//!   tree longer than a page (the last included) and a page past the end is empty;
//! - `commits/{sha}/pull` knows only a *merged* commit;
//! - the job token acts as `forgejo-actions` with id -2, and a person has a positive id;
//! - statuses are rows, and the latest of a context supersedes the earlier ones;
//! - a comment body has no size limit.
//!
//! Which state names Forgejo accepts for a status is not verified (the plan says so), so the fake
//! takes the six the Swagger lists.

use std::ops::Deref;

use serde_json::{json, Value};

use super::{message, parse_query, q, FakeComment, FakeForge, FakePull, State};
use crate::api::{ApiRequest, ApiResponse, BlobUrl, Forge, ForgeError, Method};
use crate::fake::Dialect;

const HOST: &str = "https://forgejo.example.invalid";
const STATES: [&str; 6] = [
	"pending", "success", "error", "failure", "warning", "skipped",
];

/// A commit status as Forgejo keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeStatus {
	pub id: u64,
	pub sha: String,
	pub context: String,
	pub state: String,
	pub description: String,
	pub target_url: String,
	pub creator_login: String,
	pub creator_id: i64,
}

/// A fake Forgejo. It derefs to the [`FakeForge`] it is built on, so the setters and the record
/// of requests, faults and hooks are the same ones.
#[derive(Clone)]
pub struct FakeForgejo(FakeForge);

impl Deref for FakeForgejo {
	type Target = FakeForge;

	fn deref(&self) -> &FakeForge {
		&self.0
	}
}

impl FakeForgejo {
	/// A repository `owner/name` with nothing in it. The job token acts as `forgejo-actions`
	/// (id -2).
	pub fn new(repo: &str) -> Self {
		let inner = FakeForge::new(repo);
		inner.with(|s| {
			s.dialect = Dialect::Forgejo;
			s.bot = "forgejo-actions".to_owned();
			s.bot_id = -2;
			s.limits.comment_chars = usize::MAX;
		});
		Self(inner)
	}

	/// The underlying store as a [`FakeForge`], to hand to code that takes one.
	pub fn forge(&self) -> FakeForge {
		self.0.clone()
	}

	/// Who the token acts as: the author of what it writes and the answer of `GET /user`. A bot
	/// account's personal token is a positive id.
	pub fn set_token_user(&self, login: &str, id: i64) {
		self.with(|s| {
			s.bot = login.to_owned();
			s.bot_id = id;
		});
	}

	/// The most entries one page of a tree holds, whatever `per_page` asks for (1 000 by default).
	pub fn set_tree_page_cap(&self, n: usize) {
		self.with(|s| s.tree_page_cap = n.max(1));
	}

	/// A comment by `login` with account id `user_id`.
	pub fn add_comment_as(&self, pr: u64, login: &str, user_id: i64, body: &str) -> u64 {
		self.with(|s| {
			s.next_id += 1;
			let id = s.next_id;
			s.comments.push(FakeComment {
				id,
				pr,
				login: login.to_owned(),
				user_id,
				body: body.to_owned(),
			});
			id
		})
	}

	/// A comment by `login`: `forgejo-actions` is the system account (id -2), anyone else a
	/// person with a positive id.
	pub fn add_comment(&self, pr: u64, login: &str, body: &str) -> u64 {
		let id = if login == "forgejo-actions" { -2 } else { 1001 };
		self.add_comment_as(pr, login, id, body)
	}

	/// A status written by someone else, such as another tool's.
	pub fn add_status(
		&self,
		sha: &str,
		context: &str,
		state: &str,
		description: &str,
		creator: (&str, i64),
	) -> u64 {
		self.with(|s| {
			s.next_id += 1;
			let id = s.next_id;
			s.statuses.push(FakeStatus {
				id,
				sha: sha.to_owned(),
				context: context.to_owned(),
				state: state.to_owned(),
				description: description.to_owned(),
				target_url: String::new(),
				creator_login: creator.0.to_owned(),
				creator_id: creator.1,
			});
			id
		})
	}

	/// Every status of the commit, oldest first.
	pub fn statuses(&self, sha: &str) -> Vec<FakeStatus> {
		self.with(|s| {
			s.statuses
				.iter()
				.filter(|x| x.sha == sha)
				.cloned()
				.collect()
		})
	}

	/// The latest status of each context on the commit: what Forgejo shows.
	pub fn latest_statuses(&self, sha: &str) -> Vec<FakeStatus> {
		let mut latest: Vec<FakeStatus> = Vec::new();
		for status in self.statuses(sha) {
			match latest.iter_mut().find(|l| l.context == status.context) {
				Some(slot) => *slot = status,
				None => latest.push(status),
			}
		}
		latest
	}
}

impl Forge for FakeForgejo {
	fn send(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		self.0.send(req)
	}

	fn fetch_blob(&self, url: &BlobUrl, max_bytes: u64) -> Result<Vec<u8>, ForgeError> {
		self.0.fetch_blob(url, max_bytes)
	}

	fn sleep(&self, d: std::time::Duration) {
		self.0.sleep(d);
	}

	fn now_epoch(&self) -> u64 {
		self.0.now_epoch()
	}
}

fn json_ok(status: u16, body: &Value) -> ApiResponse {
	ApiResponse {
		status,
		body: body.to_string().into_bytes(),
		..ApiResponse::default()
	}
}

fn user_json(login: &str, id: i64) -> Value {
	json!({
		"id": id,
		"login": login,
		"login_name": login,
		"full_name": login,
		"html_url": format!("{HOST}/{login}"),
		"avatar_url": format!("{HOST}/avatars/{id}"),
		"active": true,
		"visibility": "public",
	})
}

fn repo_json(full: &str, id: u64) -> Value {
	let (owner, name) = full.split_once('/').unwrap_or(("", full));
	json!({
		"id": id,
		"name": name,
		"full_name": full,
		"owner": user_json(owner, 3),
		"html_url": format!("{HOST}/{full}"),
	})
}

fn comment_json(c: &FakeComment) -> Value {
	json!({
		"id": c.id,
		"body": c.body,
		"user": user_json(&c.login, c.user_id),
		"created_at": "2026-11-02T14:40:00Z",
		"updated_at": "2026-11-02T14:40:00Z",
		"html_url": format!("{HOST}/pulls/{}#issuecomment-{}", c.pr, c.id),
		"pull_request_url": format!("{HOST}/pulls/{}", c.pr),
		"issue_url": "",
		"assets": [],
	})
}

fn status_json(x: &FakeStatus) -> Value {
	json!({
		"id": x.id,
		"context": x.context,
		"status": x.state,
		"description": x.description,
		"target_url": x.target_url,
		"creator": user_json(&x.creator_login, x.creator_id),
		"created_at": "2026-11-02T14:40:00Z",
		"updated_at": "2026-11-02T14:40:00Z",
		"url": format!("{HOST}/api/v1/statuses/{}", x.id),
	})
}

impl State {
	fn forgejo_pull(&self, p: &FakePull) -> Value {
		json!({
			"id": p.number + 500,
			"number": p.number,
			"state": if p.open { "open" } else { "closed" },
			"merged": false,
			"head": {
				"label": p.head_ref,
				"ref": p.head_ref,
				"sha": p.head_sha,
				"repo": p.head_repo.as_ref().map(|r| repo_json(r, 11)),
			},
			"base": {
				"label": "main",
				"ref": "main",
				"sha": "a".repeat(40),
				"repo": repo_json(&p.base_repo, 10),
			},
		})
	}

	pub(super) fn handle_forgejo(&mut self, req: &ApiRequest) -> ApiResponse {
		let target = req.target();
		let path = req.path.path().to_owned();
		let query = parse_query(&target);
		let segs: Vec<&str> = path.split('/').skip(1).collect();
		if req.method == Method::Get && segs == ["user"] {
			return json_ok(200, &user_json(&self.bot, self.bot_id));
		}
		let repo_parts: Vec<&str> = self.repo.split('/').collect();
		let rest: Vec<&str> = match segs.as_slice() {
			["repos", o, r, rest @ ..] if repo_parts == [*o, *r] => rest.to_vec(),
			_ => return message(404, "Not Found"),
		};
		let body: Option<Value> = req
			.body
			.as_deref()
			.and_then(|b| serde_json::from_slice(b).ok());
		let text = |key: &str| {
			body.as_ref()
				.and_then(|b| b.get(key))
				.and_then(Value::as_str)
		};
		match (req.method, rest.as_slice()) {
			(Method::Get, ["pulls", n]) => {
				let n: u64 = n.parse().unwrap_or(0);
				match self.pulls.iter().find(|p| p.number == n) {
					Some(p) => json_ok(200, &self.forgejo_pull(p)),
					None => message(404, "Not Found"),
				}
			}
			(Method::Get, ["pulls"]) => {
				let branch = q(&query, "head").unwrap_or("");
				let state = q(&query, "state").unwrap_or("open");
				let items = self
					.pulls
					.iter()
					.filter(|p| state == "all" || p.open == (state == "open"))
					.filter(|p| branch.is_empty() || p.head_ref == branch)
					.map(|p| self.forgejo_pull(p))
					.collect();
				self.page(&path, &query, items, None)
			}
			// Only a merged commit has a pull request here, and the fake never merges.
			(Method::Get, ["commits", _sha, "pull"]) => message(404, "Not Found"),
			(Method::Get, ["git", "trees", sha]) => self.forgejo_tree(sha, &query),
			(Method::Get, ["issues", n, "comments"]) => {
				let n: u64 = n.parse().unwrap_or(0);
				let items: Vec<Value> = self
					.comments
					.iter()
					.filter(|c| c.pr == n)
					.map(comment_json)
					.collect();
				ApiResponse {
					total_count: Some(items.len() as u64),
					..json_ok(200, &Value::Array(items))
				}
			}
			(Method::Post, ["issues", n, "comments"]) => {
				let n: u64 = n.parse().unwrap_or(0);
				let Some(text) = text("body") else {
					return message(422, "Validation Failed");
				};
				if let Some(other) = self.racer.take() {
					self.next_id += 1;
					let id = self.next_id;
					self.comments.push(FakeComment {
						id,
						pr: n,
						login: self.bot.clone(),
						user_id: self.bot_id,
						body: other,
					});
				}
				self.next_id += 1;
				let c = FakeComment {
					id: self.next_id,
					pr: n,
					login: self.bot.clone(),
					user_id: self.bot_id,
					body: text.to_owned(),
				};
				let out = comment_json(&c);
				self.comments.push(c);
				json_ok(201, &out)
			}
			(Method::Patch, ["issues", "comments", id]) => {
				let id: u64 = id.parse().unwrap_or(0);
				let Some(text) = text("body") else {
					return message(422, "Validation Failed");
				};
				match self.comments.iter_mut().find(|c| c.id == id) {
					Some(c) => {
						c.body = text.to_owned();
						json_ok(200, &comment_json(c))
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
			(Method::Post, ["statuses", sha]) => {
				let (Some(state), Some(context)) = (text("state"), text("context")) else {
					return message(422, "Validation Failed");
				};
				if !STATES.contains(&state) || context.is_empty() {
					return message(422, "Validation Failed");
				}
				self.next_id += 1;
				let status = FakeStatus {
					id: self.next_id,
					sha: (*sha).to_owned(),
					context: context.to_owned(),
					state: state.to_owned(),
					description: text("description").unwrap_or("").to_owned(),
					target_url: text("target_url").unwrap_or("").to_owned(),
					creator_login: self.bot.clone(),
					creator_id: self.bot_id,
				};
				let out = status_json(&status);
				self.statuses.push(status);
				json_ok(201, &out)
			}
			(Method::Get, ["commits", sha, "statuses"]) => {
				let items = self
					.statuses
					.iter()
					.filter(|x| x.sha == *sha)
					.filter(|x| q(&query, "state").is_none_or(|s| s == x.state))
					.map(status_json)
					.collect();
				self.page(&path, &query, items, None)
			}
			_ => message(404, "Not Found"),
		}
	}

	/// A page of a tree as `services/repository/files/tree.go` writes it.
	fn forgejo_tree(&self, sha: &str, query: &[(String, String)]) -> ApiResponse {
		let Some((paths, _)) = self.trees.get(sha) else {
			return message(404, "Not Found");
		};
		let per_page = q(query, "per_page")
			.and_then(|p| p.parse::<usize>().ok())
			.filter(|p| (1..=self.tree_page_cap).contains(p))
			.unwrap_or(self.tree_page_cap);
		let page = q(query, "page")
			.and_then(|p| p.parse::<usize>().ok())
			.filter(|p| *p >= 1)
			.unwrap_or(1);
		let total = paths.len();
		let start = per_page.saturating_mul(page - 1);
		let entries: Vec<Value> = paths
			.iter()
			.skip(start)
			.take(per_page)
			.map(|p| json!({ "path": p, "type": "blob", "mode": "100644", "size": 1 }))
			.collect();
		json_ok(
			200,
			&json!({
				"sha": sha,
				"page": page,
				"total_count": total,
				"truncated": start < total && total > per_page,
				"tree": entries,
				"url": format!("{HOST}/api/v1/git/trees/{sha}"),
			}),
		)
	}
}
