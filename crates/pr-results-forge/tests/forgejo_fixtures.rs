// The Forgejo host against the sourced fixtures, and the fake against the same fixtures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `fixtures/forgejo/` holds one response for each route the host uses, built from the property
//! lists of Forgejo's v17 Swagger (`SOURCES.md` says how). The first half runs the host over a
//! forge that answers with those files, so the decoders meet the documented shapes. The second half
//! asks `FakeForgejo` the same questions and checks that every key it writes, at every depth, is
//! one the fixture has, with the same kind of value, so the fake cannot drift from the Swagger.

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use pr_results_core::{CommentKey, Marker, RepoSlug, RunId, Sha};
use pr_results_forge::{
	ApiPath, ApiRequest, ApiResponse, BlobUrl, CommentAction, CommentSpec, FakeForgejo, FakePull,
	Forge, ForgeError, Forgejo, Host, Method, ResultSpec, StatusSpec,
};
use pr_results_render::StatusState;
use serde_json::Value;

const SHA: &str = "3f9c2e1d4b5a69788796a5b4c3d2e1f001122334";

fn fixture(name: &str) -> Vec<u8> {
	std::fs::read(
		Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/fixtures/forgejo")
			.join(name),
	)
	.unwrap()
}

fn fixture_json(name: &str) -> Value {
	serde_json::from_slice(&fixture(name)).unwrap()
}

/// Answers each route with its fixture.
struct Canned;

impl Forge for Canned {
	fn send(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		let path = req.path.path();
		let answer = |status: u16, file: &str, total: Option<u64>| {
			Ok(ApiResponse {
				status,
				total_count: total,
				body: fixture(file),
				..ApiResponse::default()
			})
		};
		match (req.method, path) {
			(Method::Get, "/user") => answer(200, "user.json", None),
			(Method::Get, "/repos/alice/app/pulls/7") => answer(200, "pull.json", None),
			(Method::Get, "/repos/alice/app/pulls") => answer(200, "pulls-list.json", Some(1)),
			(Method::Get, p) if p.ends_with("/pull") => answer(200, "commit-pull.json", None),
			(Method::Get, p) if p.contains("/git/trees/") => answer(200, "tree-page.json", None),
			(Method::Get, "/repos/alice/app/issues/7/comments") => {
				answer(200, "comments.json", Some(2))
			}
			(Method::Post, "/repos/alice/app/issues/7/comments") => {
				answer(201, "comment-created.json", None)
			}
			(Method::Patch, "/repos/alice/app/issues/comments/41") => {
				answer(200, "comment-patched.json", None)
			}
			(Method::Delete, "/repos/alice/app/issues/comments/42") => Ok(ApiResponse {
				status: 204,
				..ApiResponse::default()
			}),
			(Method::Post, p) if p.contains("/statuses/") => {
				answer(201, "status-created.json", None)
			}
			(Method::Get, p) if p.ends_with("/statuses") => {
				answer(200, "statuses-list.json", Some(2))
			}
			_ => Ok(ApiResponse {
				status: 404,
				body: b"{}".to_vec(),
				..ApiResponse::default()
			}),
		}
	}

	fn fetch_blob(&self, _: &BlobUrl, _: u64) -> Result<Vec<u8>, ForgeError> {
		Err(ForgeError::Refused("no blobs"))
	}

	fn sleep(&self, _: Duration) {}

	fn now_epoch(&self) -> u64 {
		0
	}
}

fn host() -> Forgejo<Canned> {
	Forgejo::new(Canned, RepoSlug::parse("alice/app").unwrap())
}

fn marker(run: u64) -> Marker {
	Marker {
		key: CommentKey::parse("ci").unwrap(),
		sha: Sha::parse(SHA).unwrap(),
		run: RunId(run),
	}
}

#[test]
fn every_fixture_is_json_and_names_its_source() {
	let notes = std::fs::read_to_string(
		Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forgejo/SOURCES.md"),
	)
	.unwrap();
	let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forgejo");
	for entry in std::fs::read_dir(dir).unwrap() {
		let name = entry.unwrap().file_name().into_string().unwrap();
		if let Some(stem) = name.strip_suffix(".json") {
			fixture_json(&name);
			assert!(
				notes.contains(&format!("`{name}`")),
				"{stem} is not in SOURCES.md"
			);
		}
	}
}

#[test]
fn the_account_pull_requests_and_tree_decode_from_the_sourced_shapes() {
	let h = host().with_account("ci-bot");
	assert_eq!(h.identity().unwrap(), ("ci-bot".to_owned(), 7));

	let pull = host().pull(7).unwrap().unwrap();
	assert_eq!(pull.number, 7);
	assert!(pull.open);
	assert_eq!(pull.head_sha, SHA);
	assert_eq!(pull.head_ref, "feature");
	assert_eq!(pull.head_repo.as_deref(), Some("alice/app"));
	assert_eq!(pull.base_repo.as_deref(), Some("alice/app"));

	let sha = Sha::parse(SHA).unwrap();
	let merged = host().pulls_for_commit(&sha).unwrap();
	assert_eq!(merged.len(), 1);
	assert!(!merged[0].open, "a merged pull request is closed");

	let by_head = host().pulls_by_head("alice", "feature").unwrap();
	assert_eq!(by_head.len(), 1);
	assert_eq!(by_head[0].number, 7);

	let tree = host().tree_paths(&sha).unwrap();
	assert_eq!(
		tree.paths,
		["README.md", "src/lib.rs"]
			.iter()
			.map(|p| (*p).to_owned())
			.collect::<BTreeSet<_>>(),
		"a directory is not a path"
	);
	assert!(!tree.truncated);
}

#[test]
fn the_comment_routes_decode_from_the_sourced_shapes() {
	let h = host();
	let list = h.comments(7).unwrap();
	assert_eq!(list.len(), 2);
	assert_eq!(
		(list[0].id, list[0].login.as_str(), list[0].user_id),
		(41, "forgejo-actions", Some(-2))
	);
	assert_eq!(
		(list[1].id, list[1].login.as_str(), list[1].user_id),
		(42, "bob", Some(9))
	);
	assert_eq!(list[0].user_type, "", "Forgejo's user has no type");

	// Comment 41 is ours with an older run: it is patched in place.
	let m = marker(20);
	let body = format!("{}\nnew", m.render());
	let r = h
		.upsert_comment(&CommentSpec {
			pr: 7,
			marker: &m,
			body: &body,
		})
		.unwrap();
	assert_eq!((r.id, r.action), (41, CommentAction::Updated));

	// A run older than the comment is left alone.
	let m = marker(3);
	let body = m.render().as_str().to_owned();
	let r = h
		.upsert_comment(&CommentSpec {
			pr: 7,
			marker: &m,
			body: &body,
		})
		.unwrap();
	assert_eq!((r.id, r.action), (41, CommentAction::SkippedNewer));
}

#[test]
fn the_status_routes_decode_from_the_sourced_shapes() {
	let sha = Sha::parse(SHA).unwrap();
	let spec = StatusSpec {
		context: "Example Rust tests",
		head_sha: &sha,
		state: StatusState::Success,
		description: "3 passed",
		target_url: "https://forgejo.example.invalid/alice/app/pulls/7#issuecomment-41",
	};
	let written = host().upsert_result(&ResultSpec::Status(spec)).unwrap();
	assert_eq!(written.id, 61);
	assert!(written.created);

	// The list holds a success (id 61) after a failure (id 60), both by `forgejo-actions` (-2).
	let h = host();
	let found = h
		.upsert_result(&ResultSpec::Status(StatusSpec {
			state: StatusState::Success,
			..spec
		}))
		.unwrap();
	assert_eq!(found.id, 61);
}

// ---- the fake against the fixtures ------------------------------------------------------------

/// Every key path in `v`, with the kind of its value, arrays flattened to `[]`.
fn shape(v: &Value, at: &str, out: &mut BTreeSet<String>) {
	match v {
		Value::Object(map) => {
			for (k, x) in map {
				let next = format!("{at}.{k}");
				out.insert(format!("{next}:{}", kind(x)));
				shape(x, &next, out);
			}
		}
		Value::Array(items) => {
			for x in items {
				shape(x, &format!("{at}[]"), out);
			}
		}
		_ => {}
	}
}

fn kind(v: &Value) -> &'static str {
	match v {
		Value::Null => "null",
		Value::Bool(_) => "bool",
		Value::Number(_) => "number",
		Value::String(_) => "string",
		Value::Array(_) => "array",
		Value::Object(_) => "object",
	}
}

/// Asserts that every path of `fake` is in `fixture` with the same kind (a `null` in the fixture
/// stands for an absent optional and matches any kind).
fn assert_within(route: &str, fake: &Value, fixture: &Value) {
	let (mut a, mut b) = (BTreeSet::new(), BTreeSet::new());
	shape(fake, "", &mut a);
	shape(fixture, "", &mut b);
	let known: BTreeSet<&str> = b
		.iter()
		.filter_map(|p| p.rsplit_once(':').map(|x| x.0))
		.collect();
	for path in &a {
		let (name, _) = path.rsplit_once(':').unwrap();
		assert!(
			known.contains(name),
			"{route}: the fake writes {path}, which the fixture lacks"
		);
		let nullable = b.contains(&format!("{name}:null"));
		assert!(
			b.contains(path) || nullable,
			"{route}: {path} differs in kind from the fixture"
		);
	}
}

fn get(fake: &FakeForgejo, path: &str) -> Value {
	let req = ApiRequest::get(ApiPath::parse(path).unwrap());
	let resp = fake.send(&req).unwrap();
	assert_eq!(resp.status, 200, "{path}");
	resp.json().unwrap()
}

#[test]
fn the_fake_writes_only_what_the_fixtures_have() {
	let fake = FakeForgejo::new("alice/app");
	fake.add_pull(FakePull::same_repo(7, "alice/app", SHA, "feature"));
	fake.add_comment(7, "forgejo-actions", "hello");
	fake.set_tree(SHA, &["README.md", "src/lib.rs"], false);
	fake.set_token_user("ci-bot", 7);
	fake.add_status(SHA, "ctx", "success", "ok", ("forgejo-actions", -2));

	assert_within("user", &get(&fake, "/user"), &fixture_json("user.json"));
	assert_within(
		"pull",
		&get(&fake, "/repos/alice/app/pulls/7"),
		&fixture_json("pull.json"),
	);
	assert_within(
		"pulls",
		&get(&fake, "/repos/alice/app/pulls?state=open&head=feature"),
		&fixture_json("pulls-list.json"),
	);
	assert_within(
		"tree",
		&get(
			&fake,
			&format!("/repos/alice/app/git/trees/{SHA}?recursive=true"),
		),
		&fixture_json("tree-page.json"),
	);
	assert_within(
		"comments",
		&get(&fake, "/repos/alice/app/issues/7/comments"),
		&fixture_json("comments.json"),
	);
	assert_within(
		"statuses",
		&get(&fake, &format!("/repos/alice/app/commits/{SHA}/statuses")),
		&fixture_json("statuses-list.json"),
	);

	let post = |path: &str, method: Method, body: &Value| {
		let req = ApiRequest::json(method, ApiPath::parse(path).unwrap(), body);
		fake.send(&req).unwrap().json().unwrap()
	};
	let created = post(
		"/repos/alice/app/issues/7/comments",
		Method::Post,
		&serde_json::json!({ "body": "x" }),
	);
	assert_within(
		"comment-created",
		&created,
		&fixture_json("comment-created.json"),
	);
	let id = created["id"].as_u64().unwrap();
	let patched = post(
		&format!("/repos/alice/app/issues/comments/{id}"),
		Method::Patch,
		&serde_json::json!({ "body": "y" }),
	);
	assert_within(
		"comment-patched",
		&patched,
		&fixture_json("comment-patched.json"),
	);
	let status = post(
		&format!("/repos/alice/app/statuses/{SHA}"),
		Method::Post,
		&serde_json::json!({ "state": "success", "context": "c", "description": "d", "target_url": "u" }),
	);
	assert_within(
		"status-created",
		&status,
		&fixture_json("status-created.json"),
	);
}
