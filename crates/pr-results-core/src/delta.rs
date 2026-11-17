// Which tests a run has that another run does not: the pull request's tests against its base's.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Test identity is the suite's name and project, the case's class name and its name, each with
//! runs of whitespace collapsed. The configured title of a section is not part of it, so renaming
//! a section in the settings does not make every test look new. Status is not part of it either:
//! a test that was skipped and now runs is the same test.
//!
//! A renamed test is a removed test and an added one, which the comment says. The comparison
//! counts duplicates, so a test that appears twice in one run and once in the other is one
//! added or removed.

use std::collections::BTreeMap;

use crate::model::{collapse_ws, Suite};

/// The most tests of each kind kept by name. The totals are always exact.
pub const MAX_LISTED: usize = 100;

/// One test, as the comparison sees it. The strings come from a report, so they are untrusted
/// and a renderer must escape them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TestId {
	pub suite: String,
	/// Empty unless the reporter names a project (Playwright).
	pub project: String,
	pub class: String,
	pub name: String,
}

/// How one run's tests differ from another's. Both lists are in name order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TestDelta {
	/// How many tests the run has that the baseline does not.
	pub added_total: usize,
	/// How many tests the baseline has that the run does not.
	pub removed_total: usize,
	/// The first `MAX_LISTED` of each, by name.
	pub added: Vec<TestId>,
	pub removed: Vec<TestId>,
}

impl TestDelta {
	pub fn is_empty(&self) -> bool {
		self.added_total == 0 && self.removed_total == 0
	}
}

fn tally(suites: &[Suite]) -> BTreeMap<TestId, usize> {
	let mut counts: BTreeMap<TestId, usize> = BTreeMap::new();
	for suite in suites {
		let suite_name = collapse_ws(suite.name.as_str());
		let project = suite
			.project
			.as_ref()
			.map(|p| collapse_ws(p.as_str()))
			.unwrap_or_default();
		for case in &suite.cases {
			let id = TestId {
				suite: suite_name.clone(),
				project: project.clone(),
				class: collapse_ws(case.classname.as_str()),
				name: collapse_ws(case.name.as_str()),
			};
			*counts.entry(id).or_insert(0) += 1;
		}
	}
	counts
}

/// The tests in `current` that `baseline` lacks, and the other way round.
pub fn test_delta(current: &[Suite], baseline: &[Suite]) -> TestDelta {
	let now = tally(current);
	let then = tally(baseline);
	let mut delta = TestDelta::default();
	for (id, &n) in &now {
		let extra = n.saturating_sub(then.get(id).copied().unwrap_or(0));
		take(&mut delta.added_total, &mut delta.added, id, extra);
	}
	for (id, &n) in &then {
		let missing = n.saturating_sub(now.get(id).copied().unwrap_or(0));
		take(&mut delta.removed_total, &mut delta.removed, id, missing);
	}
	delta
}

fn take(total: &mut usize, listed: &mut Vec<TestId>, id: &TestId, times: usize) {
	*total = total.saturating_add(times);
	for _ in 0..times {
		if listed.len() >= MAX_LISTED {
			break;
		}
		listed.push(id.clone());
	}
}
