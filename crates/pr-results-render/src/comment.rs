// The sticky pull request comment, the no-results comment and the job summary.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use pr_results_core::{Safe, Status};

use crate::blocks::{
	failure_lines, flaky_lines, footer, marker_line, section_label, section_title, table_head,
	table_row, TITLE_CHARS,
};
use crate::budget::{fit, Plan};
use crate::text::{big, cat, count, forgejo_inert, noun, num, t, Doc};
use crate::view::{entries, is_failure, Counts, Entry, SectionInfo, Verdict};
use crate::wording;
use crate::{Comment, Input, Options, RunContext, Symbols, Target};

/// The heading line: `### Title: outcome`. The title is text from the settings file, so it goes
/// through `Safe::text`, and the literal colon after it keeps it from joining the outcome. On
/// Forgejo, which links a bare commit hash, `!12`, mathematics and `:emoji:` in ordinary text, a
/// title that holds any of them goes in a code span instead.
fn heading(target: Target, title: &str, outcome: Safe) -> Safe {
	let shown = if title.trim().is_empty() {
		t("Test results")
	} else if target == Target::Forgejo && !forgejo_inert(title) {
		Safe::code_span(title, TITLE_CHARS)
	} else {
		Safe::text_max(title, TITLE_CHARS)
	};
	cat(&[t("### "), shown, t(": "), outcome])
}

/// `1 failure in 2 suites`, `All 696 passed`, `No results from 1 required suite`.
fn outcome(infos: &[SectionInfo<'_>]) -> Safe {
	let mut total = Counts::default();
	let mut missing = 0u64;
	for info in infos {
		total.add(&info.counts);
		if info.missing_required() {
			missing += 1;
		}
	}
	let suites = big(infos.len());
	if total.failed > 0 {
		let mut parts = vec![
			count(total.failed),
			noun(total.failed, " failure in ", " failures in "),
			count(suites),
			noun(suites, " suite", " suites"),
		];
		if missing > 0 {
			parts.push(t(", "));
			parts.push(count(missing));
			parts.push(t(" required without results"));
		}
		return cat(&parts);
	}
	if missing > 0 {
		return cat(&[
			t("No results from "),
			count(missing),
			noun(missing, " required suite", " required suites"),
		]);
	}
	let passed = total.passed.saturating_add(total.flaky);
	if passed == 0 && total.skipped == 0 {
		return t("No tests ran");
	}
	let mut parts = vec![t("All "), count(passed), t(" passed")];
	if total.skipped > 0 {
		parts.push(t(", "));
		parts.push(count(total.skipped));
		parts.push(t(" skipped"));
	}
	if total.flaky > 0 {
		parts.push(t(", "));
		parts.push(count(total.flaky));
		parts.push(t(" flaky"));
	}
	cat(&parts)
}

fn totals_line(total: &Counts, suites: usize) -> Safe {
	cat(&[
		t("Totals: "),
		count(total.passed),
		t(" passed, "),
		count(total.failed),
		t(" failed, "),
		count(total.skipped),
		t(" skipped, "),
		count(total.flaky),
		t(" flaky across "),
		count(big(suites)),
		noun(big(suites), " suite in ", " suites in "),
		Safe::duration_ms(total.time_ms),
		t("."),
	])
}

/// The table: one row per section, with passing sections folded into one row when the plan says.
fn table(infos: &[SectionInfo<'_>], plan: &Plan, symbols: Symbols, doc: &mut Doc) -> Option<u64> {
	for line in table_head("Suite") {
		doc.line(line);
	}
	let passing = infos
		.iter()
		.filter(|i| i.verdict == Verdict::Passed)
		.count();
	let fold = plan.collapse_passing && passing >= 2;
	let mut folded = Counts::default();
	for info in infos {
		if fold && info.verdict == Verdict::Passed {
			folded.add(&info.counts);
			continue;
		}
		doc.line(table_row(
			&section_label(info.section.title),
			info.verdict,
			info.section.required,
			&info.counts,
			symbols,
		));
	}
	if fold {
		let label = cat(&[
			count(big(passing)),
			noun(big(passing), " other suite", " other suites"),
		]);
		doc.line(table_row(&label, Verdict::Passed, false, &folded, symbols));
		return Some(big(passing));
	}
	None
}

fn render(
	i: &Input<'_>,
	infos: &[SectionInfo<'_>],
	failures: &[Entry<'_>],
	flaky: &[Entry<'_>],
	plan: &Plan,
) -> Safe {
	let o: &Options = i.options;
	let target = i.ctx.target;
	let mut doc = Doc::new();
	doc.line(marker_line(i.ctx));
	doc.line(heading(i.ctx.target, i.title, outcome(infos)));
	let mut total = Counts::default();
	for info in infos {
		total.add(&info.counts);
	}
	if plan.minimal {
		doc.block(totals_line(&total, infos.len()));
		doc.block(t(wording::minimal_note(target)));
		doc.block(footer(i.ctx));
		return doc.build();
	}

	doc.blank();
	let folded = table(infos, plan, o.symbols, &mut doc);

	for info in infos.iter().filter(|i| i.verdict == Verdict::NoResults) {
		doc.block(cat(&[
			t("No results were uploaded for "),
			section_label(info.section.title),
			t("."),
		]));
	}

	let titles: Vec<&str> = i.sections.iter().map(|s| s.title).collect();
	let multi = (titles.len() > 1).then_some(titles.as_slice());

	let m = failures.len();
	let shown = m.min(plan.max_failures);
	let mut excerpts_cut = false;
	if shown > 0 {
		doc.block(t("#### Failures"));
		for (k, e) in failures.iter().take(shown).enumerate() {
			doc.blank();
			let (lines, body) = failure_lines(
				k + 1,
				m,
				e,
				section_title(multi, e),
				plan.excerpt_lines,
				target,
			);
			excerpts_cut |= plan.excerpt_lines < o.excerpt_lines && body > plan.excerpt_lines;
			doc.lines(lines);
		}
	}
	if m > shown {
		let left = big(m - shown);
		doc.block(if shown == 0 {
			let (one, many) = wording::none_listed(target);
			cat(&[count(left), noun(left, one, many)])
		} else {
			let (one, many) = wording::more_listed(target);
			cat(&[t("\u{2026}and "), count(left), noun(left, one, many)])
		});
	}

	let mut notes: Vec<Safe> = Vec::new();
	if !flaky.is_empty() {
		if plan.extras_as_counts {
			let k = big(flaky.len());
			notes.push(cat(&[
				t("The list of "),
				count(k),
				noun(k, " flaky test", " flaky tests"),
				t(wording::flaky_left_out(target)),
			]));
		} else {
			doc.blank();
			doc.lines(flaky_lines(flaky, multi, o.max_failures, o.symbols));
		}
	}

	let skipped: usize = i.sections.iter().map(|s| s.inputs_skipped).sum();
	let mut early = Vec::new();
	if skipped > 0 {
		early.push(cat(&[
			count(big(skipped)),
			noun(
				big(skipped),
				" input file was skipped because it could not be read or went over a limit.",
				" input files were skipped because they could not be read or went over a limit.",
			),
		]));
	}
	early.append(&mut notes);
	if excerpts_cut {
		early.push(cat(&[
			t("Failure excerpts were shortened to "),
			num(plan.excerpt_lines),
			t(wording::excerpts_cut(target)),
		]));
	}
	if let Some(k) = folded {
		early.push(cat(&[count(k), t(wording::passing_combined(target))]));
	}
	for note in early {
		doc.block(note);
	}

	doc.block(footer(i.ctx));
	doc.build()
}

pub(crate) fn comment(i: &Input<'_>) -> Comment {
	let infos: Vec<SectionInfo<'_>> = i.sections.iter().map(SectionInfo::new).collect();
	let nothing = infos.iter().all(|s| s.verdict == Verdict::NoResults);
	if nothing && !infos.iter().any(|s| s.section.required) {
		return no_results(i.ctx, i.title);
	}
	let failures = entries(i.sections, is_failure);
	let flaky = entries(i.sections, |s| s == Status::Flaky);
	let (body, steps_taken) = fit(i.options.comment_limit(), Plan::start(i.options), |plan| {
		render(i, &infos, &failures, &flaky, plan)
	});
	Comment { body, steps_taken }
}

pub(crate) fn no_results(ctx: &RunContext, title: &str) -> Comment {
	let mut doc = Doc::new();
	doc.line(marker_line(ctx));
	doc.line(heading(ctx.target, title, t("No results")));
	doc.block(t("No results were uploaded for this run."));
	doc.block(footer(ctx));
	Comment {
		body: doc.build(),
		steps_taken: Vec::new(),
	}
}

/// Bytes kept free for what closes a cut summary and the line that says it was cut.
const CUT_RESERVE: usize = 256;

/// The line that closes a `<details>` block, with its line break.
const CLOSE_BLOCK_BYTES: usize = 11;

/// What a cut has to close: a code fence (its length, or nought) and the `<details>` blocks.
#[derive(Clone, Copy, Default)]
struct Open {
	fence: usize,
	details: usize,
}

impl Open {
	/// The state after `line`.
	fn after(self, line: &str) -> Self {
		let ticks = line.chars().take_while(|c| *c == '`').count();
		if self.fence > 0 {
			let closes = ticks >= self.fence && line.trim() == "`".repeat(ticks);
			return Self {
				fence: if closes { 0 } else { self.fence },
				..self
			};
		}
		if ticks >= 3 && !line.chars().skip(ticks).any(|c| c == '`') {
			return Self {
				fence: ticks,
				..self
			};
		}
		// The only lines outside code that start with `<` are the renderer's own: `<d…` opens a
		// block, `</d…` closes one and `<s…` is a summary.
		if line.starts_with("</d") {
			Self {
				details: self.details.saturating_sub(1),
				..self
			}
		} else if line.starts_with("<d") {
			Self {
				details: self.details.saturating_add(1),
				..self
			}
		} else {
			self
		}
	}

	/// Bytes of the lines that close what is open.
	fn closing_len(self) -> usize {
		let fence = if self.fence > 0 { self.fence + 1 } else { 0 };
		fence.saturating_add(self.details.saturating_mul(CLOSE_BLOCK_BYTES))
	}
}

pub(crate) fn job_summary(c: &Comment, cap: usize) -> Safe {
	let body = c.body.after_first_line();
	if body.as_str().len() <= cap {
		return body;
	}
	if cap < CUT_RESERVE {
		return Safe::default();
	}
	let mut used = 0usize;
	let mut open = Open::default();
	let mut kept = 0usize;
	let mut total = 0usize;
	for line in body.as_str().lines() {
		total += 1;
		let next = open.after(line);
		let cost = line.len() + 1;
		if kept + 1 == total
			&& used
				.saturating_add(cost)
				.saturating_add(next.closing_len())
				.saturating_add(CUT_RESERVE)
				<= cap
		{
			used += cost;
			open = next;
			kept += 1;
		}
	}
	let mut parts = vec![body.first_lines(kept)];
	if open.fence > 0 {
		parts.push(Safe::backticks(open.fence));
		parts.push(t("\n"));
	}
	for _ in 0..open.details {
		parts.push(t("</details>\n"));
	}
	let left = big(total - kept);
	parts.push(t("\n"));
	parts.push(cat(&[
		t("The last "),
		count(left),
		noun(
			left,
			" line of this summary was",
			" lines of this summary were",
		),
		t(" left out to fit the size limit for a job summary.\n"),
	]));
	cat(&parts)
}
