// Comparing a pull request's tests with its base's: which run is the baseline, and what it says.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The workflow comes from the trusted event and every run offered by the host has passed its
//! rules (a completed push run of this repository, never a fork's), so the artefacts read here
//! are the base branch's own. They are read like the run's: the same sections, the same limits,
//! the same parser. The budget is separate, so a baseline cannot starve the run's results, and
//! nothing a baseline does is an error: a section that cannot be read is left out of the
//! comparison on both sides, and when none can be there is no baseline and the comment is silent.
//! Only JUnit sections that are not `steps` are compared; findings and steps are not tests.

use pr_results_core::{test_delta, Flavour, InputFormat, Suite, TestDelta};
use pr_results_render::{Baseline, Style};

use crate::host::{BaselineQuery, Host, HostKind, Pull, RunRef};
use crate::publish::{load, Inputs, Note, Plan, PublishReport, SectionPlan, WorkflowRun};

/// Why a section of the baseline run is left out of the comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BaselineGap {
	/// An artefact for it has expired.
	Expired,
	/// An artefact for it went over a download or size limit.
	TooLarge,
	/// An artefact for it was refused as an archive.
	Refused,
	/// A file for it could not be parsed.
	Unreadable,
	/// An artefact for it could not be downloaded.
	Unavailable,
	/// No file matched, or the files held no tests.
	NoResults,
}

impl BaselineGap {
	/// A fixed phrase for the diagnostics.
	pub(crate) fn describe(self) -> &'static str {
		match self {
			Self::Expired => "an artefact has expired",
			Self::TooLarge => "an artefact went over a limit",
			Self::Refused => "an artefact was refused",
			Self::Unreadable => "a file could not be read",
			Self::Unavailable => "an artefact could not be downloaded",
			Self::NoResults => "it has no test results for the suites compared",
		}
	}
}

/// What a pull request's tests were compared with, and how they differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
	pub delta: TestDelta,
	pub against: String,
	pub fell_back_from: Option<String>,
}

impl Resolved {
	pub fn as_baseline(&self) -> Baseline<'_> {
		Baseline {
			delta: &self.delta,
			against: &self.against,
			fell_back_from: self.fell_back_from.as_deref(),
		}
	}
}

/// Where the run found was looked for.
#[derive(Clone, Copy)]
enum Source {
	/// The pull request's base commit or base branch.
	Base,
	/// `baseline.branch`.
	Fallback,
}

type Key = (Option<String>, Option<String>);

/// Finds and reads baselines for the pull requests of one run. A baseline is found once for each
/// distinct base commit and branch, however many pull requests share it.
pub(crate) struct Comparer<'a, H: Host> {
	gh: &'a H,
	run: &'a WorkflowRun,
	workflow_id: u64,
	plan: &'a Plan,
	loaded: &'a Inputs,
	/// The sections that take part, as indexes into the plan.
	eligible: Vec<usize>,
	memo: Vec<(Key, Option<Resolved>)>,
}

/// Whether a section's tests can be compared: JUnit, not the `steps` flavour.
fn comparable(s: &SectionPlan) -> bool {
	s.format == InputFormat::Junit && s.flavour != Flavour::Steps
}

impl<'a, H: Host> Comparer<'a, H> {
	/// `None` when nothing is to be compared, and then no request is made: the line is off or the
	/// style is plain, the host is not GitHub, the event has no workflow, `baseline.branch` is
	/// empty or no section can be compared. A section whose own files were not all read, or that
	/// has no tests, does not take part, since its tests would show as removed.
	pub fn new(
		gh: &'a H,
		run: &'a WorkflowRun,
		plan: &'a Plan,
		loaded: &'a Inputs,
	) -> Option<Self> {
		let workflow_id = run.workflow_id?;
		if !plan.options.test_delta
			|| plan.options.style == Style::Plain
			|| gh.kind() != HostKind::GitHub
			|| plan.baseline_branch.is_empty()
		{
			return None;
		}
		let eligible: Vec<usize> = plan
			.sections
			.iter()
			.enumerate()
			.filter(|(i, s)| {
				comparable(s)
					&& loaded.skipped.get(*i) == Some(&0)
					&& loaded
						.suites
						.get(*i)
						.is_some_and(|suites| !suites.is_empty())
			})
			.map(|(i, _)| i)
			.collect();
		if eligible.is_empty() {
			return None;
		}
		Some(Self {
			gh,
			run,
			workflow_id,
			plan,
			loaded,
			eligible,
			memo: Vec::new(),
		})
	}

	/// The comparison for `pull`, from the memo when another pull request has the same base.
	pub fn resolve(&mut self, pull: &Pull, notes: &mut Vec<Note>) -> Option<Resolved> {
		let key: Key = (
			pull.base_sha.as_ref().map(|s| s.as_str().to_owned()),
			pull.base_ref.clone(),
		);
		if let Some((_, found)) = self.memo.iter().find(|(k, _)| *k == key) {
			return found.clone();
		}
		let found = self.compare(pull, notes);
		self.memo.push((key, found.clone()));
		found
	}

	fn compare(&self, pull: &Pull, notes: &mut Vec<Note>) -> Option<Resolved> {
		let pr = pull.number;
		let (found, source) = match self.find(pull) {
			Ok(Some(f)) => f,
			Ok(None) => {
				notes.push(Note::BaselineNotFound { pr });
				return None;
			}
			Err(()) => {
				notes.push(Note::BaselineUnavailable { pr });
				return None;
			}
		};
		let (against, fell_back_from) = match source {
			Source::Base => (
				pull.base_ref
					.clone()
					.or_else(|| found.head_branch.clone())
					.unwrap_or_else(|| self.plan.baseline_branch.clone()),
				None,
			),
			Source::Fallback => (
				self.plan.baseline_branch.clone(),
				pull.base_ref
					.clone()
					.filter(|b| *b != self.plan.baseline_branch),
			),
		};

		let Ok(artifacts) = self.gh.artifacts(found.id) else {
			notes.push(Note::BaselineUnavailable { pr });
			return None;
		};
		let sections: Vec<&SectionPlan> = self
			.eligible
			.iter()
			.filter_map(|i| self.plan.sections.get(*i))
			.collect();
		// Its notes are about the run's own artefacts, which this is not.
		let mut scratch = PublishReport::default();
		let Ok(read) = load(
			self.gh,
			&artifacts,
			&sections,
			self.plan,
			true,
			&mut scratch,
		) else {
			notes.push(Note::BaselineUnavailable { pr });
			return None;
		};

		let mut now: Vec<Suite> = Vec::new();
		let mut then: Vec<Suite> = Vec::new();
		let mut compared = 0;
		let mut why = Vec::new();
		for (k, baseline_suites) in read.inputs.suites.into_iter().enumerate() {
			let ours = self
				.eligible
				.get(k)
				.and_then(|i| self.loaded.suites.get(*i));
			let gap = read.gaps.get(k).copied().flatten();
			let read_any = read.files.get(k).is_some_and(|n| *n > 0);
			match (ours, gap) {
				(Some(ours), None) if read_any && !baseline_suites.is_empty() => {
					now.extend(ours.iter().cloned());
					then.extend(baseline_suites);
					compared += 1;
				}
				_ => why.push(gap.unwrap_or(BaselineGap::NoResults)),
			}
		}
		if compared == 0 {
			why.sort();
			why.dedup();
			notes.extend(
				why.into_iter()
					.map(|why| Note::BaselineUnusable { pr, why }),
			);
			return None;
		}
		notes.push(Note::BaselineUsed {
			pr,
			run: found.id.0,
			compared,
			of: self.eligible.len(),
		});
		Some(Resolved {
			delta: test_delta(&now, &then),
			against,
			fell_back_from,
		})
	}

	/// The first run the lookups offer, in order: the base commit, the base branch, then
	/// `baseline.branch` (not asked twice when it is the base branch). The triggering run is never
	/// a baseline. `Err` when the host could not answer, which ends the search.
	fn find(&self, pull: &Pull) -> Result<Option<(RunRef, Source)>, ()> {
		if let Some(sha) = pull.base_sha.as_ref() {
			if let Some(r) = self.ask(&BaselineQuery::HeadSha(sha))? {
				return Ok(Some((r, Source::Base)));
			}
		}
		if let Some(branch) = pull.base_ref.as_deref() {
			if let Some(r) = self.ask(&BaselineQuery::Branch(branch))? {
				return Ok(Some((r, Source::Base)));
			}
		}
		let fallback = self.plan.baseline_branch.as_str();
		if pull.base_ref.as_deref() != Some(fallback) {
			if let Some(r) = self.ask(&BaselineQuery::Branch(fallback))? {
				return Ok(Some((r, Source::Fallback)));
			}
		}
		Ok(None)
	}

	fn ask(&self, query: &BaselineQuery<'_>) -> Result<Option<RunRef>, ()> {
		match self.gh.baseline_run(self.workflow_id, query) {
			Ok(found) => Ok(found.filter(|r| r.id != self.run.id)),
			Err(_) => Err(()),
		}
	}
}
