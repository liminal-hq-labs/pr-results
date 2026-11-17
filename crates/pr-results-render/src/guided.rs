// The guided style's blocks: a summary in fixed words, what needs attention, and the extras.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Everything here is a literal, a number, or a name in a code span. The summary sentence, the
//! mascot's line and the alert hold no name at all, so nothing a report or another workflow wrote
//! can reach them; the names are in the list under them, where they are always code.

use pr_results_core::Safe;

use crate::blocks::{check_label, mark, section_label, Mark};
use crate::text::{big, cat, count, noun, t};
use crate::view::{Counts, SectionInfo, Verdict};
use crate::{CheckState, Options, OtherCheck, Section, Symbols};

/// Bullets of the needs-attention list before the rest are counted.
const MAX_BULLETS: usize = 10;

/// How things stand, which chooses the words and the colour of the alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mood {
	/// Everything passed.
	Clear,
	/// Everything passed, but some tests needed a retry.
	Flaky,
	/// The tests are fine; another check failed, or a required suite sent nothing.
	Attention,
	/// A test failed.
	Failed,
	/// No test ran.
	NoTests,
}

/// The numbers the guided blocks are written from.
pub(crate) struct Tally {
	pub total: Counts,
	pub suites: usize,
	/// Required suites that sent nothing.
	pub missing: u64,
	/// Other checks that failed or need action.
	pub other_failed: u64,
	pub other_running: u64,
	pub other_passed: u64,
	/// Other checks that were skipped or cancelled.
	pub other_idle: u64,
}

impl Tally {
	pub fn of(infos: &[SectionInfo<'_>], others: &[OtherCheck]) -> Self {
		let mut total = Counts::default();
		let mut missing = 0;
		for info in infos {
			total.add(&info.counts);
			if info.missing_required() {
				missing += 1;
			}
		}
		let of =
			|keep: fn(CheckState) -> bool| big(others.iter().filter(|c| keep(c.state)).count());
		Self {
			total,
			suites: infos.len(),
			missing,
			other_failed: of(|s| matches!(s, CheckState::Failed | CheckState::ActionRequired)),
			other_running: of(|s| s == CheckState::Running),
			other_passed: of(|s| s == CheckState::Passed),
			other_idle: of(|s| matches!(s, CheckState::Skipped | CheckState::Cancelled)),
		}
	}

	pub fn mood(&self) -> Mood {
		if self.total.failed > 0 {
			Mood::Failed
		} else if self.other_failed > 0 || self.missing > 0 {
			Mood::Attention
		} else if self.total.passed.saturating_add(self.total.flaky) == 0 && self.total.skipped == 0
		{
			Mood::NoTests
		} else if self.total.flaky > 0 {
			Mood::Flaky
		} else {
			Mood::Clear
		}
	}

	fn passed(&self) -> u64 {
		self.total.passed.saturating_add(self.total.flaky)
	}
}

/// The mark in the heading, which only emoji have.
pub(crate) fn heading_mark(symbols: Symbols, mood: Mood) -> Safe {
	if symbols != Symbols::Emoji {
		return Safe::default();
	}
	mark(
		symbols,
		match mood {
			Mood::Clear | Mood::Flaky => Mark::Pass,
			Mood::Failed => Mark::Fail,
			Mood::Attention => Mark::Warn,
			Mood::NoTests => Mark::Skip,
		},
	)
}

fn tests(n: u64) -> Safe {
	cat(&[count(n), noun(n, " test", " tests")])
}

fn other_checks(n: u64) -> Safe {
	cat(&[count(n), noun(n, " other check", " other checks")])
}

/// `a`, `a and b`, `a, b and c`.
fn list(parts: &[Safe]) -> Safe {
	match parts {
		[] => Safe::default(),
		[one] => one.clone(),
		[first @ .., last] => {
			let mut out = Vec::new();
			for (i, p) in first.iter().enumerate() {
				if i > 0 {
					out.push(t(", "));
				}
				out.push(p.clone());
			}
			out.push(t(" and "));
			out.push(last.clone());
			cat(&out)
		}
	}
}

/// The summary: what needs attention, then what went well, in a fixed set of sentences.
pub(crate) fn sentence(tally: &Tally, mood: Mood) -> Safe {
	let mut parts: Vec<Safe> = Vec::new();
	match mood {
		Mood::Failed | Mood::Attention => {
			let mut needs = Vec::new();
			let failed = tally.total.failed;
			if failed > 0 {
				needs.push(cat(&[
					count(failed),
					noun(failed, " failed test", " failed tests"),
				]));
			}
			if tally.other_failed > 0 {
				needs.push(other_checks(tally.other_failed));
			}
			if tally.missing > 0 {
				needs.push(cat(&[
					count(tally.missing),
					noun(
						tally.missing,
						" required suite without results",
						" required suites without results",
					),
				]));
			}
			let one = needs.len() == 1 && failed + tally.other_failed + tally.missing == 1;
			parts.push(list(&needs));
			parts.push(t(if one {
				" needs attention."
			} else {
				" need attention."
			}));
			if tally.passed() > 0 {
				parts.push(t(" "));
				parts.push(count(tally.passed()));
				parts.push(noun(tally.passed(), " test passed", " tests passed"));
				if tally.total.flaky > 0 {
					parts.push(t(", "));
					parts.push(count(tally.total.flaky));
					parts.push(t(" of them only after a retry"));
				}
				parts.push(t("."));
			}
		}
		Mood::Clear | Mood::Flaky => {
			let passed = tally.passed();
			let suites = big(tally.suites);
			parts.push(t("Everything passed: "));
			parts.push(tests(passed));
			if tally.total.skipped > 0 {
				parts.push(t(" ("));
				parts.push(count(tally.total.skipped));
				parts.push(t(" skipped)"));
			}
			parts.push(t(" in "));
			parts.push(count(suites));
			parts.push(noun(suites, " suite", " suites"));
			if tally.other_passed > 0 {
				parts.push(t(" and "));
				parts.push(other_checks(tally.other_passed));
			}
			if tally.total.time_ms > 0 {
				parts.push(t(", in "));
				parts.push(Safe::duration_ms(tally.total.time_ms));
				parts.push(t(" of test time"));
			}
			parts.push(t("."));
			if tally.total.flaky > 0 {
				parts.push(t(" "));
				parts.push(tests(tally.total.flaky));
				parts.push(t(" only passed after a retry."));
			}
		}
		Mood::NoTests => parts.push(t("No tests ran.")),
	}
	if tally.other_running > 0 {
		parts.push(t(" "));
		parts.push(other_checks(tally.other_running));
		parts.push(noun(tally.other_running, " is", " are"));
		parts.push(t(" still running."));
	}
	cat(&parts)
}

/// What needs a person, one bullet each, with the names in code. Empty when there is nothing.
pub(crate) fn attention_lines(
	infos: &[SectionInfo<'_>],
	others: &[OtherCheck],
	symbols: Symbols,
) -> Vec<Safe> {
	let mut bullets: Vec<Safe> = Vec::new();
	let bullet = |kind: Mark, rest: Vec<Safe>| {
		let mut parts = vec![t("- "), mark(symbols, kind)];
		parts.extend(rest);
		cat(&parts)
	};
	for info in infos.iter().filter(|i| i.counts.failed > 0) {
		let n = info.counts.failed;
		bullets.push(bullet(
			Mark::Fail,
			vec![
				count(n),
				noun(n, " test failed in ", " tests failed in "),
				section_label(info.section.title),
				t("."),
			],
		));
	}
	for info in infos.iter().filter(|i| i.missing_required()) {
		bullets.push(bullet(
			Mark::Skip,
			vec![
				t("No results from the required suite "),
				section_label(info.section.title),
				t("."),
			],
		));
	}
	let mut failing: Vec<&OtherCheck> = others
		.iter()
		.filter(|c| matches!(c.state, CheckState::Failed | CheckState::ActionRequired))
		.collect();
	failing.sort_by(|a, b| a.state.cmp(&b.state).then_with(|| a.name.cmp(&b.name)));
	for c in failing {
		let (kind, said) = if c.state == CheckState::Failed {
			(Mark::Fail, " failed.")
		} else {
			(Mark::Warn, " needs action.")
		};
		let mut rest = vec![check_label(&c.name), t(said)];
		if let Some(command) = &c.command {
			rest.push(t(" To run it locally: "));
			rest.push(crate::blocks::command_span(command));
		}
		bullets.push(bullet(kind, rest));
	}
	for info in infos.iter().filter(|i| i.counts.flaky > 0) {
		let n = info.counts.flaky;
		bullets.push(bullet(
			Mark::Flaky,
			vec![
				count(n),
				noun(n, " test in ", " tests in "),
				section_label(info.section.title),
				noun(
					n,
					" passed only after a retry, so it may be flaky.",
					" passed only after a retry, so they may be flaky.",
				),
			],
		));
	}
	if bullets.len() > MAX_BULLETS {
		let left = big(bullets.len() - MAX_BULLETS);
		bullets.truncate(MAX_BULLETS);
		bullets.push(cat(&[t("\u{2026}and "), count(left), t(" more.")]));
	}
	bullets
}

/// Whole blocks of a bar, so a screen reader reads the sentence before the shapes.
const BARS: [&str; 11] = [
	"\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}",
	"\u{25b0}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}",
	"\u{25b0}\u{25b0}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}",
	"\u{25b0}\u{25b0}\u{25b0}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}",
	"\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}",
	"\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}",
	"\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b1}\u{25b1}\u{25b1}\u{25b1}",
	"\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b1}\u{25b1}\u{25b1}",
	"\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b1}\u{25b1}",
	"\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b1}",
	"\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}",
];

/// `737 of 738 tests passed ▰▰▰▰▰▰▰▰▰▱`: the share that passed, in tenths, rounded down, and
/// never a full bar while anything failed. Skipped tests are not counted.
pub(crate) fn progress_line(total: &Counts) -> Option<Safe> {
	let passed = total.passed.saturating_add(total.flaky);
	let ran = passed.saturating_add(total.failed);
	if ran == 0 {
		return None;
	}
	let tenths = usize::try_from(u128::from(passed) * 10 / u128::from(ran)).unwrap_or(10);
	let filled = if total.failed > 0 {
		tenths.min(9)
	} else {
		tenths.min(10)
	};
	Some(cat(&[
		count(passed),
		t(" of "),
		count(ran),
		t(" tests passed "),
		t(BARS.get(filled).copied().unwrap_or(BARS[10])),
	]))
}

/// One line from Jax, in a fixed set of sentences, so a re-run with the same results writes the
/// same words.
/// The two sentences of the note that the comment is edited in place. `job_summary` drops them, since a
/// run's job summary is written once and is not updated.
pub(crate) const UPDATED_NOTE: &str = "This comment is updated in place with the latest results.";
pub(crate) const UPDATED_NOTE_EMOJI: &str =
	"\u{267b}\u{fe0f} This comment is updated in place with the latest results.";

pub(crate) fn updated_note(symbols: Symbols) -> Safe {
	t(if symbols == Symbols::Emoji {
		UPDATED_NOTE_EMOJI
	} else {
		UPDATED_NOTE
	})
}

pub(crate) fn mascot(mood: Mood) -> Safe {
	t(match mood {
		Mood::Clear => "\u{1f9a6} **Jax:** All clear. Every test passed.",
		Mood::Flaky => {
			"\u{1f9a6} **Jax:** Everything passed, though a test needed a retry. Worth a look."
		}
		Mood::Failed => "\u{1f9a6} **Jax:** Something broke. The details are below.",
		Mood::Attention => {
			"\u{1f9a6} **Jax:** The tests are fine, but something else needs a look."
		}
		Mood::NoTests => "\u{1f9a6} **Jax:** I could not find any test results to show.",
	})
}

/// A GitHub alert block around `paragraphs`. Each is a fixed sentence: an alert's lines are
/// quoted, and nothing from a report is ever put in one.
pub(crate) fn alert(mood: Mood, paragraphs: &[Safe]) -> Vec<Safe> {
	let mut lines = vec![t(match mood {
		Mood::Clear => "> [!TIP]",
		Mood::Flaky | Mood::NoTests => "> [!NOTE]",
		Mood::Attention => "> [!WARNING]",
		Mood::Failed => "> [!CAUTION]",
	})];
	for (i, p) in paragraphs.iter().enumerate() {
		if i > 0 {
			lines.push(t(">"));
		}
		lines.push(cat(&[t("> "), p.clone()]));
	}
	lines
}

/// The blocks that open a guided comment, in order: the alert or the plain paragraphs, then the
/// list, then the bar.
pub(crate) fn opening(
	tally: &Tally,
	infos: &[SectionInfo<'_>],
	others: &[OtherCheck],
	o: &Options,
	in_alert: bool,
) -> Vec<Vec<Safe>> {
	let mood = tally.mood();
	let mut paragraphs = Vec::new();
	if o.mascot {
		paragraphs.push(mascot(mood));
	}
	paragraphs.push(sentence(tally, mood));
	let mut blocks: Vec<Vec<Safe>> = Vec::new();
	if o.alerts && in_alert {
		blocks.push(alert(mood, &paragraphs));
	} else {
		blocks.extend(paragraphs.into_iter().map(|p| vec![p]));
	}
	let bullets = attention_lines(infos, others, o.symbols);
	if !bullets.is_empty() {
		blocks.push(bullets);
	}
	if o.progress {
		blocks.extend(progress_line(&tally.total).map(|l| vec![l]));
	}
	blocks
}

/// `Other checks: 1 failing, 5 passed.` for the smallest comment, where the
/// table does not fit.
pub(crate) fn other_checks_line(others: &[OtherCheck]) -> Option<Safe> {
	if others.is_empty() {
		return None;
	}
	let tally = Tally::of(&[], others);
	let mut parts: Vec<Safe> = Vec::new();
	for (n, words) in [
		(tally.other_failed, " failing"),
		(tally.other_running, " running"),
		(tally.other_passed, " passed"),
		(tally.other_idle, " skipped or cancelled"),
	] {
		if n > 0 {
			parts.push(cat(&[count(n), t(words)]));
		}
	}
	Some(cat(&[t("Other checks: "), list_commas(&parts), t(".")]))
}

fn list_commas(parts: &[Safe]) -> Safe {
	let mut out = Vec::new();
	for (i, p) in parts.iter().enumerate() {
		if i > 0 {
			out.push(t(", "));
		}
		out.push(p.clone());
	}
	cat(&out)
}

/// Whether a job's row and its suite's row say the same thing, so one of them is enough.
fn agree(state: CheckState, verdict: Verdict) -> bool {
	matches!(
		(state, verdict),
		(CheckState::Passed, Verdict::Passed) | (CheckState::Failed, Verdict::Failed)
	)
}

/// The other checks that tell the reader something the suites do not. A suite that names its job
/// replaces that job's row only while the two agree: a job that failed with no results uploaded, or
/// one that passed while its tests failed, is exactly what the table is for.
pub(crate) fn visible_others(sections: &[Section<'_>], others: &[OtherCheck]) -> Vec<OtherCheck> {
	others
		.iter()
		.filter(|c| {
			!sections.iter().any(|s| {
				s.job == Some(c.name.as_str()) && agree(c.state, SectionInfo::new(s).verdict)
			})
		})
		.cloned()
		.collect()
}
