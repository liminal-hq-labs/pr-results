// The sticky comment's algorithm, shared by every host: which comment to update, when to leave it, what to remove.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A host supplies four operations over its own routes and its own idea of whose comment is ours;
//! [`upsert_sticky`] is the same everywhere: the stale-run guard, the oldest of our comments
//! updated, the others deleted, and a re-list after a create so concurrent runs converge on one.
//! The conformance suite in `tests/common/conformance.rs` runs against every host.

use pr_results_core::{CommentKey, Marker};

use crate::api::ForgeError;
use crate::host::{Comment, CommentAction, CommentResult, CommentSpec};

pub(crate) trait Sticky {
	/// The comments of pull request `pr` that are ours for `key`, oldest first. A comment is ours
	/// only if its author is and its first line is the marker.
	fn ours(&self, pr: u64, key: &CommentKey) -> Result<Vec<Comment>, ForgeError>;

	/// Replaces a comment's body. `Ok(false)` when the comment is gone.
	fn patch(&self, id: u64, body: &str) -> Result<bool, ForgeError>;

	/// Creates the comment and returns its id, after making sure a failed create that may have
	/// been applied is not repeated.
	fn create(&self, spec: &CommentSpec<'_>) -> Result<u64, ForgeError>;

	/// Deletes comments, ignoring those already gone. Returns how many were removed.
	fn remove(&self, ids: &mut dyn Iterator<Item = u64>) -> usize;
}

/// Keeps one sticky comment per pull request and key. It leaves the comment alone when one for
/// the same commit from a later run is there, updates the oldest of ours for the key, and creates
/// one when there is none, then deletes any other of ours for the key so concurrent runs converge
/// on one.
pub(crate) fn upsert_sticky<S: Sticky + ?Sized>(
	s: &S,
	spec: &CommentSpec<'_>,
) -> Result<CommentResult, ForgeError> {
	let key = &spec.marker.key;
	let ours = s.ours(spec.pr, key)?;
	let newer = ours.iter().find(|c| {
		Marker::parse(&c.body).is_some_and(|m| m.sha == spec.marker.sha && m.run > spec.marker.run)
	});
	if let Some(c) = newer {
		return Ok(CommentResult {
			id: c.id,
			action: CommentAction::SkippedNewer,
			duplicates_removed: 0,
		});
	}
	if let Some(oldest) = ours.first() {
		if s.patch(oldest.id, spec.body)? {
			let removed = s.remove(&mut ours.iter().skip(1).map(|c| c.id));
			return Ok(CommentResult {
				id: oldest.id,
				action: CommentAction::Updated,
				duplicates_removed: removed,
			});
		}
		// It was deleted since it was listed (404): fall through and create one.
	}
	let created = s.create(spec)?;
	let ours = s.ours(spec.pr, key)?;
	let Some(keep) = ours.first() else {
		return Ok(CommentResult {
			id: created,
			action: CommentAction::Created,
			duplicates_removed: 0,
		});
	};
	let removed = s.remove(&mut ours.iter().skip(1).map(|c| c.id));
	if keep.id != created {
		// Another run's create landed first and ours was removed: its comment stays, and says
		// what this run found unless a later run for the same commit wrote it.
		let later = Marker::parse(&keep.body)
			.is_some_and(|m| m.sha == spec.marker.sha && m.run > spec.marker.run);
		if !later && !s.patch(keep.id, spec.body)? {
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
