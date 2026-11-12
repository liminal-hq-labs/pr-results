// A check run: its title, summary, text and annotations.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use pr_results_core::{Case, RepoPath, Safe, Status};

use crate::blocks::{
	failure_lines, flaky_lines, footer, message_line, path_line, section_label, slowest,
	table_head, table_row,
};
use crate::budget::{fit, Plan};
use crate::text::{big, cat, count, join, measure, noun, num, t, Doc, Piece};
use crate::view::{entries, is_failure, suite_label, Counts, Entry, SectionInfo, Verdict};
use crate::{
	Annotation, CheckOutput, Conclusion, Input, Level, Section, StatusOutput, StatusState, Target,
	STATUS_DESCRIPTION_CHARS,
};

/// GitHub allows at most this many annotations on one check run.
const ANNOTATION_HARD_CAP: usize = 500;
/// An annotation's title is at most 255 characters.
const ANNOTATION_TITLE_CHARS: usize = 255;
/// An annotation's message is at most 64 KiB.
const ANNOTATION_MESSAGE_BYTES: usize = 65_535;
/// Where the search for a message that fits starts, in characters.
const ANNOTATION_MESSAGE_CHARS: usize = 16_000;
/// Rows of the per-suite table.
const SUITE_ROWS: usize = 50;
/// Rows of the slowest-tests table.
const SLOWEST: usize = 10;
/// The longest body of one failure in the text, in characters.
const DETAIL_CHARS: usize = 20_000;

pub(crate) fn check(
	i: &Input<'_>,
	index: usize,
	keep: &dyn Fn(&RepoPath) -> bool,
) -> Option<CheckOutput> {
	if i.ctx.target == Target::Forgejo {
		return None;
	}
	let section = i.sections.get(index)?;
	let info = SectionInfo::new(section);
	let one = std::slice::from_ref(section);
	let failures = entries(one, is_failure);
	let flaky = entries(one, |s| s == Status::Flaky);
	let (annotations, capped) = annotations(i, section, keep);

	let conclusion = match info.verdict {
		Verdict::Failed => Conclusion::Failure,
		Verdict::Passed => Conclusion::Success,
		Verdict::NoResults if section.required => Conclusion::Failure,
		Verdict::NoResults => Conclusion::Neutral,
	};
	let (summary, _) = fit(i.options.summary_limit(), Plan::start(i.options), |plan| {
		summary(i, &info, &failures, &flaky, plan, annotations.len(), capped)
	});
	Some(CheckOutput {
		conclusion,
		title: title(&info),
		summary,
		text: text(i, section, &failures),
		annotations,
		annotations_capped: capped,
	})
}

pub(crate) fn status(i: &Input<'_>, index: usize) -> Option<StatusOutput> {
	let section = i.sections.get(index)?;
	let info = SectionInfo::new(section);
	let state = match info.verdict {
		Verdict::Failed => StatusState::Failure,
		Verdict::Passed => StatusState::Success,
		Verdict::NoResults if section.required => StatusState::Failure,
		Verdict::NoResults => StatusState::Success,
	};
	// No thin spaces between digit groups: plain text spells them out.
	let title = title_with(&info, Safe::number);
	let description = bounded(
		STATUS_DESCRIPTION_CHARS,
		STATUS_DESCRIPTION_CHARS,
		measure,
		|budget| Safe::plain_text(title.as_str(), budget),
	);
	Some(StatusOutput { state, description })
}

/// `1 failed, 23 passed, 4 skipped`, or `672 passed, 1 flaky`.
fn title(info: &SectionInfo<'_>) -> Safe {
	title_with(info, count)
}

fn title_with(info: &SectionInfo<'_>, number: fn(u64) -> Safe) -> Safe {
	if info.verdict == Verdict::NoResults {
		return t("No results were uploaded");
	}
	let c = &info.counts;
	let mut parts: Vec<Safe> = Vec::new();
	for (n, word) in [
		(c.failed, " failed"),
		(c.passed, " passed"),
		(c.flaky, " flaky"),
		(c.skipped, " skipped"),
	] {
		if n > 0 {
			parts.push(cat(&[number(n), t(word)]));
		}
	}
	if parts.is_empty() {
		return t("No tests ran");
	}
	join(&t(", "), &parts)
}

fn summary(
	i: &Input<'_>,
	info: &SectionInfo<'_>,
	failures: &[Entry<'_>],
	flaky: &[Entry<'_>],
	plan: &Plan,
	annotations: usize,
	capped: usize,
) -> Safe {
	let o = i.options;
	let section = info.section;
	let mut doc = Doc::new();
	for line in table_head("Suite") {
		doc.line(line);
	}
	doc.line(table_row(
		&section_label(section.title),
		info.verdict,
		section.required,
		&info.counts,
		o.symbols,
	));

	if info.verdict == Verdict::NoResults {
		doc.block(t(if section.required {
			"No results were uploaded for this run, and this suite is required."
		} else {
			"No results were uploaded for this run."
		}));
	}
	if section.inputs_skipped > 0 {
		let n = big(section.inputs_skipped);
		doc.block(cat(&[
			count(n),
			noun(
				n,
				" input file was skipped because it could not be read or went over a limit.",
				" input files were skipped because they could not be read or went over a limit.",
			),
		]));
	}
	if plan.minimal {
		doc.block(t(
			"Most of this summary was left out to fit GitHub's size limit.",
		));
		doc.block(footer(i.ctx));
		return doc.build();
	}

	if !section.suites.is_empty() {
		doc.blank();
		for line in suite_table(section.suites) {
			doc.line(line);
		}
	}

	let m = failures.len();
	let shown = m.min(plan.max_failures);
	let mut excerpts_cut = false;
	if shown > 0 {
		doc.block(t("#### Failures"));
		for (k, e) in failures.iter().take(shown).enumerate() {
			doc.blank();
			let (lines, body) =
				failure_lines(k + 1, m, e, None, plan.excerpt_lines, Target::GitHub);
			excerpts_cut |= plan.excerpt_lines < o.excerpt_lines && body > plan.excerpt_lines;
			doc.lines(lines);
		}
	}
	if m > shown {
		let left = big(m - shown);
		doc.block(if shown == 0 {
			cat(&[
				count(left),
				noun(
					left,
					" failure is not listed in this summary.",
					" failures are not listed in this summary.",
				),
			])
		} else {
			cat(&[
				t("\u{2026}and "),
				count(left),
				noun(
					left,
					" more failure, not listed in this summary.",
					" more failures, not listed in this summary.",
				),
			])
		});
	}
	if !flaky.is_empty() {
		if plan.extras_as_counts {
			let k = big(flaky.len());
			doc.block(cat(&[
				t("The list of "),
				count(k),
				noun(k, " flaky test", " flaky tests"),
				t(" was left out to fit GitHub's size limit. The table counts them."),
			]));
		} else {
			doc.blank();
			doc.lines(flaky_lines(flaky, None, o.max_failures, o.symbols));
		}
	}
	if excerpts_cut {
		doc.block(cat(&[
			t("Failure excerpts were shortened to "),
			num(plan.excerpt_lines),
			t(" lines each to fit GitHub's size limit."),
		]));
	}
	if annotations > 0 || capped > 0 {
		doc.block(annotation_line(annotations, capped, o.max_annotations));
	}
	doc.block(footer(i.ctx));
	doc.build()
}

fn annotation_line(shown: usize, capped: usize, cap: usize) -> Safe {
	if capped == 0 {
		return cat(&[t("Annotations: "), count(big(shown)), t(".")]);
	}
	cat(&[
		t("Annotations: "),
		count(big(shown)),
		t(" shown, "),
		count(big(capped)),
		t(" not shown (the cap is "),
		count(big(cap.min(ANNOTATION_HARD_CAP))),
		t(")."),
	])
}

/// One row per JUnit suite, so step durations read at a glance.
fn suite_table(suites: &[pr_results_core::Suite]) -> Vec<Safe> {
	let mut lines = vec![
		t("| Group | Passed | Failed | Skipped | Flaky | Time |"),
		t("| --- | ---: | ---: | ---: | ---: | ---: |"),
	];
	for suite in suites.iter().take(SUITE_ROWS) {
		let c = Counts::of(std::slice::from_ref(suite));
		lines.push(join(
			&t(" | "),
			&[
				cat(&[t("| "), Safe::code_span(&suite_label(suite), 120)]),
				count(c.passed),
				count(c.failed),
				count(c.skipped),
				count(c.flaky),
				cat(&[Safe::duration_ms(c.time_ms), t(" |")]),
			],
		));
	}
	if suites.len() > SUITE_ROWS {
		let left = big(suites.len() - SUITE_ROWS);
		lines.push(Safe::default());
		lines.push(cat(&[
			t("\u{2026}and "),
			count(left),
			noun(left, " more suite.", " more suites."),
		]));
	}
	lines
}

fn slowest_lines(section: &Section<'_>, rows: usize) -> Vec<Safe> {
	let slow = slowest(section.suites, rows);
	if slow.is_empty() {
		return Vec::new();
	}
	let mut lines = vec![
		t("#### Slowest tests"),
		Safe::default(),
		t("| Test | Time |"),
		t("| --- | ---: |"),
	];
	for (suite, case) in slow {
		let e = Entry {
			section: 0,
			suite,
			case,
		};
		lines.push(join(
			&t(" | "),
			&[
				cat(&[t("| "), path_line(&e, None)]),
				cat(&[Safe::duration_ms(case.time_ms), t(" |")]),
			],
		));
	}
	lines
}

fn lines_len(lines: &[Safe]) -> usize {
	lines.iter().map(|l| measure(l).saturating_add(1)).sum()
}

/// One failure with its whole body, in at most `cap` characters of it.
fn detail_lines(n: usize, e: &Entry<'_>, cap: usize) -> Vec<Safe> {
	let mut lines = vec![
		cat(&[t("##### "), num(n), t(". "), path_line(e, None)]),
		Safe::default(),
		message_line(e.case, Target::GitHub),
	];
	if let Some(detail) = &e.case.detail {
		if !detail.is_empty() {
			lines.push(Safe::default());
			lines.push(Safe::code_block(detail, cap));
		}
	}
	lines
}

/// The details of every failure that fits, then the slowest ten cases.
fn text(i: &Input<'_>, section: &Section<'_>, failures: &[Entry<'_>]) -> Safe {
	let limit = i.options.text_limit();
	let slow = slowest_lines(section, SLOWEST);
	let mut shown: Vec<Vec<Safe>> = Vec::new();
	let mut used = 0usize;
	let reserve = lines_len(&slow).saturating_add(300);
	'failures: for (k, e) in failures.iter().enumerate() {
		let room = limit.saturating_sub(reserve).saturating_sub(used);
		if room < 400 {
			break;
		}
		let mut cap = room.saturating_sub(300).min(DETAIL_CHARS);
		loop {
			let lines = detail_lines(k + 1, e, cap);
			let len = lines_len(&lines).saturating_add(1);
			if len <= room {
				used = used.saturating_add(len);
				shown.push(lines);
				break;
			}
			if cap <= 64 {
				break 'failures;
			}
			cap /= 2;
		}
	}
	loop {
		let built = assemble_text(&shown, failures.len(), &slow);
		if measure(&built) <= limit {
			return built;
		}
		if shown.pop().is_none() {
			// Even the slowest tests do not fit a limit this small.
			return assemble_text(&[], failures.len(), &[]);
		}
	}
}

fn assemble_text(shown: &[Vec<Safe>], total: usize, slow: &[Safe]) -> Safe {
	let mut doc = Doc::new();
	if !shown.is_empty() {
		doc.block(t("#### Failure details"));
		for lines in shown {
			doc.blank();
			doc.lines(lines.clone());
		}
	}
	if total > shown.len() && !shown.is_empty() {
		let left = big(total - shown.len());
		doc.block(cat(&[
			t("\u{2026}and "),
			count(left),
			noun(
				left,
				" more failure, not shown here to fit GitHub's size limit.",
				" more failures, not shown here to fit GitHub's size limit.",
			),
		]));
	}
	if !slow.is_empty() {
		doc.blank();
		doc.lines(slow.to_vec());
	}
	let built = doc.build();
	if built.as_str().trim().is_empty() {
		Safe::default()
	} else {
		built
	}
}

/// An annotation for each failed or errored case with a path that `keep` accepts, then (if
/// asked) each flaky one.
fn annotations(
	i: &Input<'_>,
	section: &Section<'_>,
	keep: &dyn Fn(&RepoPath) -> bool,
) -> (Vec<Annotation>, usize) {
	let o = i.options;
	let mut found = Vec::new();
	for (wanted, level) in [
		(true, Level::Failure),
		(o.flaky_annotations, Level::Warning),
	] {
		if !wanted {
			continue;
		}
		for suite in section.suites {
			for case in &suite.cases {
				let matches = match level {
					Level::Failure => is_failure(case.status),
					_ => case.status == Status::Flaky,
				};
				if !matches {
					continue;
				}
				if let Some(a) = annotation(case, level).filter(|a| keep(&a.path)) {
					found.push(a);
				}
			}
		}
	}
	let cap = o.max_annotations.min(ANNOTATION_HARD_CAP);
	let capped = found.len().saturating_sub(cap);
	found.truncate(cap);
	(found, capped)
}

fn annotation(case: &Case, level: Level) -> Option<Annotation> {
	let path = RepoPath::from_case(case.file.as_ref()?)?;
	let line = case.line.filter(|l| *l > 0).unwrap_or(1);
	let message = Piece {
		text: match (&case.message, &case.detail) {
			(Some(m), Some(d)) => format!("{}\n{}", m.as_str(), d.as_str()),
			(Some(m), None) => m.as_str().to_owned(),
			(None, Some(d)) => d.as_str().to_owned(),
			(None, None) => "No message was reported.".to_owned(),
		},
		omitted: case
			.message
			.as_ref()
			.map_or(0, pr_results_core::Untrusted::omitted)
			.saturating_add(
				case.detail
					.as_ref()
					.map_or(0, pr_results_core::Untrusted::omitted),
			),
	};
	Some(Annotation {
		path,
		start_line: line,
		end_line: line,
		level,
		title: bounded(
			ANNOTATION_TITLE_CHARS,
			ANNOTATION_TITLE_CHARS,
			measure,
			|budget| Safe::plain_text(&case.name, budget),
		),
		message: bounded(
			ANNOTATION_MESSAGE_BYTES,
			ANNOTATION_MESSAGE_CHARS,
			|s| s.as_str().len(),
			|budget| Safe::plain_block(&message, budget),
		),
	})
}

/// The text `build` makes from the largest budget, starting at `start` characters, whose `size`
/// is within `limit`. A cut appends a marker after the budget, so the budget alone is not the
/// size; this takes the excess off the budget until the whole, marker included, fits.
fn bounded(
	limit: usize,
	start: usize,
	size: impl Fn(&Safe) -> usize,
	build: impl Fn(usize) -> Safe,
) -> Safe {
	let mut budget = start.min(limit);
	loop {
		let built = build(budget);
		let excess = size(&built).saturating_sub(limit);
		if excess == 0 || budget == 0 {
			return built;
		}
		budget = budget.saturating_sub(excess);
	}
}
