// The fixed words of the comment that differ between forges: where the full results live and
// whose size limit a cut is for.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Every sentence here is a literal chosen by a `match` on the [`Target`], never a format. The
//! words "check run" and "GitHub" belong to [`Target::GitHub`]: a Forgejo has commit statuses
//! and no check runs, and a test fails if either word reaches a Forgejo comment.

use crate::Target;

/// The comment's last resort: only the heading, the totals and the footer remain.
pub(crate) fn minimal_note(target: Target) -> &'static str {
	match target {
		Target::GitHub => {
			"Most of this comment was left out to fit GitHub's size limit. The check runs hold the full results."
		}
		Target::Forgejo => {
			"Most of this comment was left out to fit the size limit. The commit's statuses give each suite's result."
		}
	}
}

/// No failure is listed: one and many.
pub(crate) fn none_listed(target: Target) -> (&'static str, &'static str) {
	match target {
		Target::GitHub => (
			" failure is not listed here. See the check runs.",
			" failures are not listed here. See the check runs.",
		),
		Target::Forgejo => (
			" failure is not listed here.",
			" failures are not listed here.",
		),
	}
}

/// Some failures are listed and the rest are counted: one and many, after "…and N".
pub(crate) fn more_listed(target: Target) -> (&'static str, &'static str) {
	match target {
		Target::GitHub => (
			" more failure in the check runs.",
			" more failures in the check runs.",
		),
		Target::Forgejo => (
			" more failure, not listed here.",
			" more failures, not listed here.",
		),
	}
}

/// After the count of flaky tests whose list was left out.
pub(crate) fn flaky_left_out(target: Target) -> &'static str {
	match target {
		Target::GitHub => " was left out to fit GitHub's size limit. The table counts them.",
		Target::Forgejo => " was left out to fit the size limit. The table counts them.",
	}
}

/// After the number of lines each excerpt was cut to.
pub(crate) fn excerpts_cut(target: Target) -> &'static str {
	match target {
		Target::GitHub => " lines each to fit GitHub's size limit.",
		Target::Forgejo => " lines each to fit the size limit.",
	}
}

/// After the number of passing suites folded into one row.
pub(crate) fn passing_combined(target: Target) -> &'static str {
	match target {
		Target::GitHub => " passing suites were combined into one row to fit GitHub's size limit.",
		Target::Forgejo => " passing suites were combined into one row to fit the size limit.",
	}
}
