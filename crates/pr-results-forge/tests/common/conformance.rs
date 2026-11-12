// The conformance suite every `Host` must pass for the sticky comment.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `publish` verifies the pull request, but the safety of the sticky comment lives in a host's
//! `upsert_comment`: whose comments are ours, which one to update, when to leave it alone and what
//! may be deleted. A host that got any of that wrong would still pass the flow tests, so each
//! host's tests call [`comment_conformance`] with a [`World`] over its own in-memory forge. Adding
//! a host without a call here fails review.

use pr_results_core::{CommentKey, Marker, RunId, Sha};
use pr_results_forge::{CommentAction, CommentSpec, Host};

const SHA: &str = "3f9c2e1d4b5a69788796a5b4c3d2e1f001122334";
const PR: u64 = 7;

/// One host over a fresh in-memory forge, with raw comments to seed and read back.
pub trait World {
	type H: Host;
	fn host(&self) -> &Self::H;
	/// The login whose comments are ours.
	fn bot(&self) -> &str;
	/// Adds a comment as `login`, whatever its body, and returns its id.
	fn seed(&self, pr: u64, login: &str, body: &str) -> u64;
	/// Lists in `n` items to a page, as the host's forge does.
	fn page_size(&self, n: usize);
	/// Every comment of the pull request as `(id, login, body)`, in id order.
	fn comments(&self, pr: u64) -> Vec<(u64, String, String)>;
}

fn marker(key: &str, run: u64) -> Marker {
	Marker {
		key: CommentKey::parse(key).unwrap(),
		sha: Sha::parse(SHA).unwrap(),
		run: RunId(run),
	}
}

fn body(m: &Marker, text: &str) -> String {
	format!("{}\n{text}", m.render())
}

fn upsert<W: World>(w: &W, m: &Marker, text: &str) -> pr_results_forge::CommentResult {
	let b = body(m, text);
	w.host()
		.upsert_comment(&CommentSpec {
			pr: PR,
			marker: m,
			body: &b,
		})
		.unwrap()
}

fn text_of(w: &Comments<'_>, id: u64) -> Option<String> {
	w.0.iter().find(|c| c.0 == id).map(|c| c.2.clone())
}

struct Comments<'a>(&'a [(u64, String, String)]);

/// Runs every case, each on a world of its own from `new`.
pub fn comment_conformance<W: World>(new: impl Fn() -> W) {
	an_impostor_with_our_marker_is_never_updated_or_deleted(&new());
	our_comment_is_updated_beside_an_impostor(&new());
	another_key_is_left_alone(&new());
	a_newer_run_for_the_same_commit_is_not_overwritten(&new());
	duplicates_are_removed_only_if_ours(&new());
	our_comment_is_found_among_many_pages(&new());
}

fn an_impostor_with_our_marker_is_never_updated_or_deleted<W: World>(w: &W) {
	let m = marker("ci", 10);
	let fake = w.seed(PR, "mallory", &body(&marker("ci", 5), "not ours"));
	let r = upsert(w, &m, "ours");
	assert_eq!(r.action, CommentAction::Created, "an impostor was updated");
	assert_ne!(r.id, fake);
	let all = w.comments(PR);
	let all = Comments(&all);
	assert_eq!(
		text_of(&all, fake)
			.as_deref()
			.map(|t| t.contains("not ours")),
		Some(true)
	);
	assert_eq!(r.duplicates_removed, 0);
	assert_eq!(all.0.len(), 2);
}

fn our_comment_is_updated_beside_an_impostor<W: World>(w: &W) {
	let fake = w.seed(PR, "mallory", &body(&marker("ci", 5), "not ours"));
	let mine = w.seed(PR, w.bot(), &body(&marker("ci", 5), "old"));
	let r = upsert(w, &marker("ci", 10), "new");
	assert_eq!(r.action, CommentAction::Updated);
	assert_eq!(r.id, mine);
	let all = w.comments(PR);
	let all = Comments(&all);
	assert!(text_of(&all, mine).unwrap().contains("new"));
	assert!(
		text_of(&all, fake).unwrap().contains("not ours"),
		"an impostor was changed or deleted"
	);
}

fn another_key_is_left_alone<W: World>(w: &W) {
	let other = w.seed(PR, w.bot(), &body(&marker("other", 5), "other key"));
	let r = upsert(w, &marker("ci", 10), "ours");
	assert_eq!(r.action, CommentAction::Created);
	assert_ne!(r.id, other);
	let all = w.comments(PR);
	let all = Comments(&all);
	assert!(text_of(&all, other).unwrap().contains("other key"));
	assert_eq!(all.0.len(), 2);
}

fn a_newer_run_for_the_same_commit_is_not_overwritten<W: World>(w: &W) {
	let newer = w.seed(PR, w.bot(), &body(&marker("ci", 20), "newer"));
	let r = upsert(w, &marker("ci", 10), "older");
	assert_eq!(r.action, CommentAction::SkippedNewer);
	assert_eq!(r.id, newer);
	let all = w.comments(PR);
	assert_eq!(all.len(), 1);
	assert!(all[0].2.contains("newer"));
}

fn duplicates_are_removed_only_if_ours<W: World>(w: &W) {
	let first = w.seed(PR, w.bot(), &body(&marker("ci", 1), "one"));
	let dup = w.seed(PR, w.bot(), &body(&marker("ci", 1), "two"));
	let imp = w.seed(PR, "mallory", &body(&marker("ci", 1), "three"));
	let r = upsert(w, &marker("ci", 10), "new");
	assert_eq!(r.id, first);
	assert_eq!(r.duplicates_removed, 1);
	let all = w.comments(PR);
	let all = Comments(&all);
	assert!(text_of(&all, first).unwrap().contains("new"));
	assert!(text_of(&all, dup).is_none(), "our duplicate stayed");
	assert!(text_of(&all, imp).is_some(), "an impostor was deleted");
}

fn our_comment_is_found_among_many_pages<W: World>(w: &W) {
	w.page_size(7);
	for i in 0..125 {
		w.seed(PR, "someone", &format!("comment {i}"));
	}
	let mine = w.seed(PR, w.bot(), &body(&marker("ci", 1), "old"));
	for i in 0..124 {
		w.seed(PR, "someone", &format!("later {i}"));
	}
	assert_eq!(w.comments(PR).len(), 250);
	let r = upsert(w, &marker("ci", 10), "new");
	assert_eq!(r.action, CommentAction::Updated);
	assert_eq!(r.id, mine);
	let all = w.comments(PR);
	assert_eq!(all.len(), 250, "a second comment was made");
	assert!(Comments(&all)
		.0
		.iter()
		.any(|c| c.0 == mine && c.2.contains("new")));
}
