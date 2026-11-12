// The normalized model: a report of suites of test cases, with untrusted text kept apart.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// The schema name of the model when it is written out.
pub const REPORT_SCHEMA: &str = "pr-results-report/1";

/// Text that came from a test report. It has no `Display`: the only way to put it in output is
/// [`Safe`](crate::Safe). It remembers how many bytes were cut when it was capped, so truncation is
/// always explicit.
///
/// The text is shared, so cloning is cheap: a suite's `file` attribute is held once, however many
/// cases inherit it.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Untrusted {
	text: Arc<str>,
	omitted: usize,
}

impl Untrusted {
	/// Keeps all of `text`.
	pub fn new(text: impl Into<String>) -> Self {
		Self {
			text: Arc::from(text.into()),
			omitted: 0,
		}
	}

	/// Keeps at most `max_bytes` of `text`, cutting at a character boundary, and counts the rest.
	pub fn capped(text: &str, max_bytes: usize) -> Self {
		let mut end = max_bytes.min(text.len());
		while !text.is_char_boundary(end) {
			end -= 1;
		}
		Self {
			text: Arc::from(text.get(..end).unwrap_or_default()),
			omitted: text.get(end..).map_or(0, |tail| tail.chars().count()),
		}
	}

	/// The raw text. For comparison, hashing and tests; never for output.
	pub fn as_str(&self) -> &str {
		&self.text
	}

	/// How many characters were cut off when the text was capped.
	pub fn omitted(&self) -> usize {
		self.omitted
	}

	pub fn is_empty(&self) -> bool {
		self.text.is_empty()
	}

	/// The length of the kept text, in bytes.
	pub fn len(&self) -> usize {
		self.text.len()
	}

	/// Whether `self` and `other` are the same stored text, not just equal.
	pub fn shares_text_with(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.text, &other.text)
	}
}

/// Text being collected from an element, up to a cap, before it becomes an [`Untrusted`].
#[derive(Default)]
pub(crate) struct TextBuf {
	text: String,
	omitted: usize,
}

impl TextBuf {
	pub(crate) fn as_str(&self) -> &str {
		&self.text
	}

	pub(crate) fn push_capped(&mut self, more: &str, max_bytes: usize) {
		let room = max_bytes.saturating_sub(self.text.len());
		let mut end = room.min(more.len());
		while !more.is_char_boundary(end) {
			end -= 1;
		}
		self.text.push_str(more.get(..end).unwrap_or_default());
		let cut = more.get(end..).map_or(0, |tail| tail.chars().count());
		self.omitted = self.omitted.saturating_add(cut);
	}

	/// The text without surrounding whitespace, or nothing if it was all whitespace.
	pub(crate) fn trimmed(self) -> Option<Untrusted> {
		let text = self.text.trim();
		(!text.is_empty()).then(|| Untrusted {
			text: Arc::from(text),
			omitted: self.omitted,
		})
	}
}

impl fmt::Debug for Untrusted {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Untrusted({:?}", &*self.text)?;
		if self.omitted > 0 {
			write!(f, ", +{} omitted", self.omitted)?;
		}
		f.write_str(")")
	}
}

impl PartialEq<&str> for Untrusted {
	fn eq(&self, other: &&str) -> bool {
		self.omitted == 0 && &*self.text == *other
	}
}

/// The outcome of a test case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
	Passed,
	Failed,
	/// The test could not run to a verdict (JUnit `<error>`).
	Errored,
	Skipped,
	/// Passed after a failed attempt in the same run.
	Flaky,
}

/// One earlier try of a test that was retried: nextest's `flakyFailure`, `flakyError`,
/// `rerunFailure` and `rerunError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
	pub status: Status,
	pub message: Option<Untrusted>,
	pub detail: Option<Untrusted>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
	pub name: Untrusted,
	pub classname: Untrusted,
	/// A repository-relative path, only ever one that passed the path rules (relative, no `..`).
	pub file: Option<Untrusted>,
	pub line: Option<u32>,
	pub status: Status,
	pub time_ms: u64,
	pub attempts: Vec<Attempt>,
	/// How many further attempts were in the report but not kept, past `Limits::max_attempts`.
	/// A renderer shows this whenever it is not zero, so a cut is never silent.
	pub attempts_omitted: usize,
	/// The first line of why the case failed, errored or was skipped. Capped at 1 KiB.
	pub message: Option<Untrusted>,
	/// The body of the failure. Capped at 16 KiB.
	pub detail: Option<Untrusted>,
	/// Captured output, kept only so reporters can be recognised. Never rendered by default.
	pub system_out: Option<Untrusted>,
	pub system_err: Option<Untrusted>,
}

impl Case {
	/// The identity used to merge and compare cases: classname and name with runs of whitespace
	/// collapsed, joined by U+001F. The suite's key is added by the caller. Not for output.
	pub fn identity(&self) -> String {
		format!(
			"{}\u{1f}{}",
			collapse_ws(self.classname.as_str()),
			collapse_ws(self.name.as_str())
		)
	}
}

fn collapse_ws(s: &str) -> String {
	s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Which reporter wrote the file. It decides how names and locations are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flavour {
	/// Pick from the file: the root name, `assertions` attributes and Playwright attachments.
	Auto,
	Generic,
	Nextest,
	/// The step results a product's converter writes from `check.sh`.
	Steps,
	Vitest,
	Jest,
	Bun,
	Playwright,
}

impl Flavour {
	/// The name used in settings.
	pub fn name(self) -> &'static str {
		match self {
			Self::Auto => "auto",
			Self::Generic => "generic",
			Self::Nextest => "nextest",
			Self::Steps => "steps",
			Self::Vitest => "vitest",
			Self::Jest => "jest",
			Self::Bun => "bun",
			Self::Playwright => "playwright",
		}
	}

	pub fn from_name(name: &str) -> Option<Self> {
		[
			Self::Auto,
			Self::Generic,
			Self::Nextest,
			Self::Steps,
			Self::Vitest,
			Self::Jest,
			Self::Bun,
			Self::Playwright,
		]
		.into_iter()
		.find(|f| f.name() == name)
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suite {
	pub name: Untrusted,
	/// Never `Auto`: the flavour the parser settled on.
	pub flavour: Flavour,
	pub cases: Vec<Case>,
	pub time_ms: u64,
}

impl Suite {
	pub fn count(&self, status: Status) -> usize {
		self.cases.iter().filter(|c| c.status == status).count()
	}

	/// Folds `other` into this suite: its cases follow this suite's, and the times add.
	pub fn merge(&mut self, other: Suite) {
		self.time_ms = self.time_ms.saturating_add(other.time_ms);
		self.cases.extend(other.cases);
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
	pub suites: Vec<Suite>,
}

impl Report {
	/// Merges suites that share a name and flavour, keeping the order in which each was first
	/// seen, so a matrix of jobs that each upload part of a suite shows as one.
	///
	/// The lookup is an ordered map, so the work grows with the number of suites times the log of
	/// that number, however hostile the input, and the result does not depend on hashing.
	pub fn merge_same_named(&mut self) {
		let suites = std::mem::take(&mut self.suites);
		// Where each suite ends up: the position of the first suite with its name and flavour.
		let mut first: BTreeMap<(&str, Flavour), usize> = BTreeMap::new();
		let mut slots = Vec::with_capacity(suites.len());
		for suite in &suites {
			let next = first.len();
			slots.push(
				*first
					.entry((suite.name.as_str(), suite.flavour))
					.or_insert(next),
			);
		}
		drop(first);
		let mut merged: Vec<Suite> = Vec::with_capacity(slots.iter().max().map_or(0, |m| m + 1));
		for (suite, slot) in suites.into_iter().zip(slots) {
			match merged.get_mut(slot) {
				Some(existing) => existing.merge(suite),
				None => merged.push(suite),
			}
		}
		self.suites = merged;
	}
}
