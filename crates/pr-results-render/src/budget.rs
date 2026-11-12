// The size budget: the order in which a comment gives things up to fit, and the loop that runs it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use pr_results_core::Safe;

use crate::text::measure;
use crate::{Degradation, Options};

impl Degradation {
	/// Every step, in the order they are taken.
	pub const ALL: [Degradation; 8] = [
		Degradation::ExtrasToCounts,
		Degradation::Excerpt8,
		Degradation::Excerpt3,
		Degradation::Failures20,
		Degradation::Failures5,
		Degradation::NoFailures,
		Degradation::PassingCollapsed,
		Degradation::Minimal,
	];

	/// The step as a lower-case, hyphenated word, for file names and logs.
	pub fn name(self) -> &'static str {
		match self {
			Self::ExtrasToCounts => "extras-to-counts",
			Self::Excerpt8 => "excerpt-8",
			Self::Excerpt3 => "excerpt-3",
			Self::Failures20 => "failures-20",
			Self::Failures5 => "failures-5",
			Self::NoFailures => "no-failures",
			Self::PassingCollapsed => "passing-collapsed",
			Self::Minimal => "minimal",
		}
	}
}

/// What a render shows, cumulatively reduced by the steps taken so far.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
	/// Flaky tests as a count only.
	pub extras_as_counts: bool,
	/// Lines of each failure's body.
	pub excerpt_lines: usize,
	/// Failures listed.
	pub max_failures: usize,
	/// Passing suites as one row.
	pub collapse_passing: bool,
	/// Heading, totals and footer only.
	pub minimal: bool,
}

impl Plan {
	pub fn start(options: &Options) -> Self {
		Self {
			extras_as_counts: false,
			excerpt_lines: options.excerpt_lines,
			max_failures: options.max_failures,
			collapse_passing: false,
			minimal: false,
		}
	}

	fn apply(&mut self, step: Degradation) {
		match step {
			Degradation::ExtrasToCounts => self.extras_as_counts = true,
			Degradation::Excerpt8 => self.excerpt_lines = self.excerpt_lines.min(8),
			Degradation::Excerpt3 => self.excerpt_lines = self.excerpt_lines.min(3),
			Degradation::Failures20 => self.max_failures = self.max_failures.min(20),
			Degradation::Failures5 => self.max_failures = self.max_failures.min(5),
			Degradation::NoFailures => self.max_failures = 0,
			Degradation::PassingCollapsed => self.collapse_passing = true,
			Degradation::Minimal => self.minimal = true,
		}
	}
}

/// Renders with `start`, and while the result is over `limit` characters takes the next step. A
/// step that changes nothing (there was nothing for it to cut) is not recorded. The last step,
/// `Minimal`, is the heading, the totals and the footer, which fit in any limit worth setting;
/// if even that is over, it is returned as it is.
pub(crate) fn fit(
	limit: usize,
	start: Plan,
	render: impl Fn(&Plan) -> Safe,
) -> (Safe, Vec<Degradation>) {
	let mut plan = start;
	let mut out = render(&plan);
	let mut taken = Vec::new();
	for step in Degradation::ALL {
		if measure(&out) <= limit {
			break;
		}
		plan.apply(step);
		let next = render(&plan);
		if next != out {
			taken.push(step);
			out = next;
		}
	}
	(out, taken)
}
