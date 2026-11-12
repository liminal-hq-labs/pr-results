// The pr-results renderer: the pull request comment, the check runs and their annotations.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Markdown from the model, built only from [`Safe`] fragments, so a raw string cannot reach it.
//! Everything here is pure: the same inputs give byte-identical output, with no clocks and no
//! network.
//!
//! Three rules shape every line:
//!
//! - **HTML lines hold only trusted words and numbers.** Markdown inside a raw HTML line is not
//!   rendered, so a name placed in a `<summary>` would show literally. Names go on a Markdown
//!   line of their own, after a blank line.
//! - **Untrusted text goes only through `code_span`, `code_block` or `plain_text`.** GitHub looks
//!   for mentions and issue references in ordinary text, not in code.
//! - **Fragments never join into a reference.** `GH` next to `-123` is two safe pieces and one
//!   issue link, so every line is put together by `cat`, which separates such a pair.

#![forbid(unsafe_code)]

mod blocks;
mod budget;
mod check;
mod comment;
mod text;
mod view;

use pr_results_core::{CommentKey, RepoPath, RepoSlug, RunId, Safe, ServerUrl, Sha, Suite};

/// Where a run happened. Every field is a checked value, so the links built from them are safe.
#[derive(Debug, Clone)]
pub struct RunContext {
	pub server: ServerUrl,
	pub repo: RepoSlug,
	pub head_sha: Sha,
	pub run_id: RunId,
	pub key: CommentKey,
	/// Written in the footer. A literal, so it cannot carry text from a report.
	pub tool_version: &'static str,
}

/// One configured suite: one check run and one row of the table. It may hold many JUnit suites
/// from many files.
#[derive(Debug, Clone)]
pub struct Section<'a> {
	/// Shown in the table and in the names of failures.
	pub title: &'a str,
	/// The name of the check run.
	pub check_name: &'a str,
	/// With no suites, a required section is a failure and an optional one is neutral.
	pub required: bool,
	pub suites: &'a [Suite],
	/// Files that were found but could not be read or went over a limit.
	pub inputs_skipped: usize,
}

/// Whether status words carry a glyph in front of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbols {
	Text,
	Glyphs,
}

#[derive(Debug, Clone)]
pub struct Options {
	/// Failures listed before the budget makes it fewer.
	pub max_failures: usize,
	/// Lines of each failure's body.
	pub excerpt_lines: usize,
	pub symbols: Symbols,
	/// Character limits, counted after escaping. Each is held to 65,535 when used, GitHub's own
	/// limit for a check run's summary and text and just under its limit for a comment.
	pub comment_limit: usize,
	pub summary_limit: usize,
	pub text_limit: usize,
	/// At most 500, GitHub's own limit for one check run.
	pub max_annotations: usize,
	/// Flaky cases are annotated as warnings.
	pub flaky_annotations: bool,
}

/// The most any of the three limits may be, whatever the setting.
const LIMIT_CEILING: usize = 65_535;

impl Options {
	pub(crate) fn comment_limit(&self) -> usize {
		self.comment_limit.min(LIMIT_CEILING)
	}

	pub(crate) fn summary_limit(&self) -> usize {
		self.summary_limit.min(LIMIT_CEILING)
	}

	pub(crate) fn text_limit(&self) -> usize {
		self.text_limit.min(LIMIT_CEILING)
	}
}

impl Default for Options {
	fn default() -> Self {
		Self {
			max_failures: 20,
			excerpt_lines: 20,
			symbols: Symbols::Text,
			comment_limit: 60_000,
			summary_limit: 60_000,
			text_limit: 60_000,
			max_annotations: 50,
			flaky_annotations: false,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Input<'a> {
	pub ctx: &'a RunContext,
	/// The heading's title, from the settings.
	pub title: &'a str,
	pub sections: &'a [Section<'a>],
	pub options: &'a Options,
}

/// What a comment gives up to fit its limit, in the order it gives it up. A step that would
/// change nothing is not taken. `Failures20` caps the list at the smaller of the setting and 20,
/// so it matters only when `max_failures` is set higher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Degradation {
	/// Flaky tests as a count, not a list.
	ExtrasToCounts,
	/// Failure excerpts of 8 lines.
	Excerpt8,
	Excerpt3,
	Failures20,
	Failures5,
	NoFailures,
	/// Passing suites as one row.
	PassingCollapsed,
	/// The heading, the totals and the footer.
	Minimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
	/// The first line is the marker.
	pub body: Safe,
	pub steps_taken: Vec<Degradation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
	Notice,
	Warning,
	Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conclusion {
	Success,
	Failure,
	Neutral,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
	pub path: RepoPath,
	pub start_line: u32,
	pub end_line: u32,
	pub level: Level,
	/// Plain text, not Markdown.
	pub title: Safe,
	/// Plain text, not Markdown.
	pub message: Safe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutput {
	pub conclusion: Conclusion,
	pub title: Safe,
	pub summary: Safe,
	pub text: Safe,
	pub annotations: Vec<Annotation>,
	/// Annotations that were found but not kept, past `Options::max_annotations`.
	pub annotations_capped: usize,
}

/// The sticky comment for a run.
pub fn comment(i: &Input<'_>) -> Comment {
	comment::comment(i)
}

/// The check run for `section`, or nothing if there is no such section.
pub fn check(i: &Input<'_>, section: usize) -> Option<CheckOutput> {
	check::check(i, section)
}

/// The comment for a run that uploaded nothing.
pub fn no_results(ctx: &RunContext, title: &str) -> Comment {
	comment::no_results(ctx, title)
}

/// The comment without its marker, for the workflow run's own page.
pub fn job_summary(c: &Comment) -> Safe {
	comment::job_summary(c)
}

/// How big `text` is for the limits: its characters, or its UTF-16 code units if that is more,
/// because GitHub may count either and a limit is only worth having if both stay under it.
pub fn measure(text: &Safe) -> usize {
	text::measure(text)
}
