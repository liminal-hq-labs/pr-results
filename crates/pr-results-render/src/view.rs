// What the renderer reads out of the model: counts, failures, groups and excerpts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use pr_results_core::{Case, Flavour, RepoPath, Status, Suite, Untrusted};

use crate::text::Piece;
use crate::Section;

/// Totals over the suites of one section, or of several.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Counts {
	pub passed: u64,
	/// Failed and errored.
	pub failed: u64,
	pub skipped: u64,
	pub flaky: u64,
	pub time_ms: u64,
}

impl Counts {
	pub fn of(suites: &[Suite]) -> Self {
		let mut c = Self::default();
		for suite in suites {
			c.time_ms = c.time_ms.saturating_add(suite.time_ms);
			for case in &suite.cases {
				let slot = match case.status {
					Status::Passed => &mut c.passed,
					Status::Failed | Status::Errored => &mut c.failed,
					Status::Skipped => &mut c.skipped,
					Status::Flaky => &mut c.flaky,
				};
				*slot = slot.saturating_add(1);
			}
		}
		c
	}

	pub fn add(&mut self, other: &Self) {
		self.passed = self.passed.saturating_add(other.passed);
		self.failed = self.failed.saturating_add(other.failed);
		self.skipped = self.skipped.saturating_add(other.skipped);
		self.flaky = self.flaky.saturating_add(other.flaky);
		self.time_ms = self.time_ms.saturating_add(other.time_ms);
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
	Failed,
	Passed,
	/// No suites at all: nothing was uploaded, or nothing could be read.
	NoResults,
}

pub(crate) struct SectionInfo<'a> {
	pub section: &'a Section<'a>,
	pub counts: Counts,
	pub verdict: Verdict,
}

impl SectionInfo<'_> {
	pub fn new<'a>(section: &'a Section<'a>) -> SectionInfo<'a> {
		let counts = Counts::of(section.suites);
		let verdict = if counts.failed > 0 {
			Verdict::Failed
		} else if section.suites.is_empty() {
			Verdict::NoResults
		} else {
			Verdict::Passed
		};
		SectionInfo {
			section,
			counts,
			verdict,
		}
	}

	/// A required suite that sent nothing counts as a failure.
	pub fn missing_required(&self) -> bool {
		self.verdict == Verdict::NoResults && self.section.required
	}
}

/// A case that did not pass, or passed only after a retry, with where it came from.
pub(crate) struct Entry<'a> {
	pub section: usize,
	pub suite: &'a Suite,
	pub case: &'a Case,
}

/// Every case of `status` in input order: sections, then suites, then cases.
pub(crate) fn entries<'a>(
	sections: &'a [Section<'a>],
	keep: impl Fn(Status) -> bool,
) -> Vec<Entry<'a>> {
	let mut found = Vec::new();
	for (index, section) in sections.iter().enumerate() {
		for suite in section.suites {
			for case in &suite.cases {
				if keep(case.status) {
					found.push(Entry {
						section: index,
						suite,
						case,
					});
				}
			}
		}
	}
	found
}

pub(crate) fn is_failure(status: Status) -> bool {
	matches!(status, Status::Failed | Status::Errored)
}

/// The group a case belongs to, as it is shown between the suite and the name.
///
/// For step results a class name reads `script:group` or `check.group`, so the group is what
/// follows the last colon, or failing that the last full stop. Any other reporter's class name is
/// shown as it is. A group that repeats the suite's name is dropped.
pub(crate) fn group_of(suite: &Suite, case: &Case) -> Option<Piece> {
	let class = &case.classname;
	let raw = class.as_str();
	let shown = if suite.flavour == Flavour::Steps {
		raw.rsplit_once(':')
			.or_else(|| raw.rsplit_once('.'))
			.map_or(raw, |(_, group)| group)
	} else {
		raw
	};
	let shown = shown.trim();
	if shown.is_empty() || (shown == suite.name.as_str() && class.omitted() == suite.name.omitted())
	{
		return None;
	}
	Some(Piece {
		text: shown.to_owned(),
		omitted: class.omitted(),
	})
}

/// A suite's name, followed by the project it ran under in brackets where the reporter names one
/// (Playwright), so the same spec file in two projects reads as two suites.
pub(crate) fn suite_label(suite: &Suite) -> Piece {
	match &suite.project {
		Some(project) => Piece {
			text: format!("{} [{}]", suite.name.as_str(), project.as_str()),
			omitted: suite.name.omitted().saturating_add(project.omitted()),
		},
		None => Piece {
			text: suite.name.as_str().to_owned(),
			omitted: suite.name.omitted(),
		},
	}
}

/// The part of a failure's body that is shown: its first lines, or for step logs its last.
pub(crate) struct Excerpt {
	pub piece: Piece,
	pub total_lines: usize,
	pub shown_lines: usize,
	pub from_end: bool,
}

fn blank(line: &str) -> bool {
	line.trim().is_empty()
}

/// The first `cap` lines of `detail`, or the last `cap` when `from_end`. Trailing blank lines do
/// not count. With a cap of nought there is no excerpt.
pub(crate) fn excerpt(detail: &Untrusted, cap: usize, from_end: bool) -> Option<Excerpt> {
	let mut lines: Vec<&str> = detail.as_str().lines().collect();
	while lines.last().is_some_and(|l| blank(l)) {
		lines.pop();
	}
	let total = lines.len();
	if total == 0 || cap == 0 {
		return None;
	}
	if total <= cap {
		return Some(Excerpt {
			piece: Piece {
				text: lines.join("\n"),
				omitted: detail.omitted(),
			},
			total_lines: total,
			shown_lines: total,
			from_end,
		});
	}
	let (kept, omitted) = if from_end {
		(
			lines.get(total - cap..).unwrap_or_default(),
			detail.omitted(),
		)
	} else {
		(lines.get(..cap).unwrap_or_default(), 0)
	};
	Some(Excerpt {
		piece: Piece {
			text: kept.join("\n"),
			omitted,
		},
		total_lines: total,
		shown_lines: cap,
		from_end,
	})
}

/// `path:line` of a case that reports a whole repository path, or `path` when the line is
/// unknown.
pub(crate) fn location_piece(case: &Case) -> Option<Piece> {
	let path = RepoPath::from_case(case.file.as_ref()?)?;
	let text = match case.line.filter(|l| *l > 0) {
		Some(line) => format!("{}:{line}", path.as_str()),
		None => path.as_str().to_owned(),
	};
	Some(Piece { text, omitted: 0 })
}

/// What `cargo nextest run` starts with when the settings name nothing else.
const NEXTEST_PREFIX: &str = "cargo nextest run";

/// A binary id as nextest writes one: a package name, optionally `::` and a test target.
fn binary_id_ok(s: &str) -> bool {
	let mut chars = s.chars();
	chars
		.next()
		.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
		&& s.len() <= 120
		&& s.chars()
			.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '/' | '-'))
}

/// A test's path as Rust writes one: identifiers joined by `::`.
fn test_name_ok(s: &str) -> bool {
	let mut chars = s.chars();
	chars
		.next()
		.is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
		&& s.len() <= 200
		&& s.chars()
			.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':'))
}

/// What the settings put in front of the filter: printable ASCII with no quote or backtick, or
/// the flavour's own when none is set. An empty setting turns the command off.
fn prefix_ok(s: &str) -> bool {
	!s.is_empty()
		&& s.len() <= 200
		&& s.chars()
			.all(|c| (' '..='~').contains(&c) && !matches!(c, '`' | '\'' | '\\'))
}

/// A command that runs just this failed nextest test again.
///
/// It is built only from words that are all ASCII letters, digits, `_` and `:` (and `-` and `/` in
/// the binary id), and only when nothing was cut from either. Anything else gets no command, so
/// no quote, `$`, backtick, `;`, line break or nextest filter syntax in a name can reach a shell
/// or the filter. The filter uses `=` so each name is matched whole.
pub(crate) fn nextest_command(prefix: Option<&str>, suite: &Suite, case: &Case) -> Option<Piece> {
	if suite.flavour != Flavour::Nextest {
		return None;
	}
	let prefix = prefix.unwrap_or(NEXTEST_PREFIX);
	if !prefix_ok(prefix) {
		return None;
	}
	let (binary, omitted) = if case.classname.as_str().is_empty() {
		(suite.name.as_str(), suite.name.omitted())
	} else {
		(case.classname.as_str(), case.classname.omitted())
	};
	if omitted > 0 || case.name.omitted() > 0 {
		return None;
	}
	let name = case.name.as_str();
	if !binary_id_ok(binary) || !test_name_ok(name) {
		return None;
	}
	Some(Piece {
		text: format!("{prefix} -E 'binary_id(={binary}) & test(={name})'"),
		omitted: 0,
	})
}
