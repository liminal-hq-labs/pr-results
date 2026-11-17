// The pieces both the comment and the check runs are made of: tables, failures, flaky lists, footer.
//
// Untrusted text reaches these lines only through `code_span` and `code_block`. The HTML lines
// (`<details>`, `<summary>`, `</details>`) hold literals and numbers and nothing else, because
// Markdown inside a raw HTML line is not rendered.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use pr_results_core::{Case, Marker, Safe, Status, Suite, TrustedUrl};

use crate::text::{big, cat, count, join, noun, num, t, Piece};
use crate::view::{excerpt, group_of, location_piece, suite_label, Counts, Entry, Verdict};
use crate::{CheckState, RunContext, Symbols, Target};

/// Budgets for the names shown on a line, in characters after escaping.
const SUITE_CHARS: usize = 120;
const GROUP_CHARS: usize = 120;
const NAME_CHARS: usize = 200;
const MESSAGE_CHARS: usize = 300;
pub(crate) const TITLE_CHARS: usize = 120;
const CHECK_CHARS: usize = 120;
const COMMAND_CHARS: usize = 400;

/// The table cell and failure prefix for a section: its title, in a code span.
pub(crate) fn section_label(title: &str) -> Safe {
	Safe::code_span(title, SUITE_CHARS)
}

pub(crate) fn marker_line(ctx: &RunContext) -> Safe {
	Marker {
		key: ctx.key.clone(),
		sha: ctx.head_sha.clone(),
		run: ctx.run_id,
	}
	.render()
}

/// `Commit abc · workflow run · artefacts · pr-results 0.1.0`, all links built from the context.
/// Forgejo has no artefacts section on a run's page, and names the page by the run's number.
pub(crate) fn footer(ctx: &RunContext) -> Safe {
	let commit = TrustedUrl::commit(&ctx.server, &ctx.repo, &ctx.head_sha);
	let commit = cat(&[
		t("Commit "),
		Safe::link(&commit, &Safe::sha_short(&ctx.head_sha)),
	]);
	let version = cat(&[t("pr-results "), Safe::trusted(ctx.tool_version)]);
	let separator = t(" \u{b7} ");
	match ctx.target {
		Target::GitHub => {
			let run = TrustedUrl::run(&ctx.server, &ctx.repo, ctx.run_id);
			let artefacts = TrustedUrl::run_artifacts(&ctx.server, &ctx.repo, ctx.run_id);
			join(
				&separator,
				&[
					commit,
					Safe::link(&run, &t("workflow run")),
					Safe::link(&artefacts, &t("artefacts")),
					version,
				],
			)
		}
		Target::Forgejo => {
			let run = match ctx.run_number {
				Some(number) => TrustedUrl::forgejo_run(&ctx.server, &ctx.repo, number),
				None => TrustedUrl::run(&ctx.server, &ctx.repo, ctx.run_id),
			};
			join(
				&separator,
				&[commit, Safe::link(&run, &t("workflow run")), version],
			)
		}
	}
}

/// What a mark in front of a status word says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mark {
	Pass,
	Fail,
	/// Passed only after a retry.
	Flaky,
	/// Skipped, or no results.
	Skip,
	Cancelled,
	Running,
	/// Needs a person's attention but is not a failure.
	Warn,
}

/// The mark and the space after it, or nothing at all for plain text.
pub(crate) fn mark(symbols: Symbols, kind: Mark) -> Safe {
	match symbols {
		Symbols::Text => Safe::default(),
		Symbols::Glyphs => t(match kind {
			Mark::Pass => "\u{2713} ",
			Mark::Fail | Mark::Warn => "\u{2717} ",
			Mark::Flaky | Mark::Running => "\u{21bb} ",
			Mark::Skip | Mark::Cancelled => "\u{2013} ",
		}),
		Symbols::Emoji => t(match kind {
			Mark::Pass => "\u{2705} ",
			Mark::Fail => "\u{274c} ",
			Mark::Flaky => "\u{1f3b2} ",
			Mark::Skip => "\u{23ed}\u{fe0f} ",
			Mark::Cancelled => "\u{1f6ab} ",
			Mark::Running => "\u{1f504} ",
			Mark::Warn => "\u{26a0}\u{fe0f} ",
		}),
	}
}

pub(crate) fn table_head(first: &'static str) -> [Safe; 2] {
	[
		cat(&[
			t("| "),
			t(first),
			t(" | Result | Passed | Failed | Skipped | Flaky | Time |"),
		]),
		t("| --- | --- | ---: | ---: | ---: | ---: | ---: |"),
	]
}

fn result_cell(verdict: Verdict, required: bool, symbols: Symbols) -> Safe {
	match verdict {
		Verdict::Failed => cat(&[mark(symbols, Mark::Fail), t("**Failed**")]),
		Verdict::Passed => cat(&[mark(symbols, Mark::Pass), t("Passed")]),
		Verdict::NoResults if required => cat(&[mark(symbols, Mark::Skip), t("**No results**")]),
		Verdict::NoResults => cat(&[mark(symbols, Mark::Skip), t("No results")]),
	}
}

pub(crate) fn other_head() -> [Safe; 2] {
	[t("| Check | Result |"), t("| --- | --- |")]
}

fn state_cell(state: CheckState, symbols: Symbols) -> Safe {
	match state {
		CheckState::Failed => cat(&[mark(symbols, Mark::Fail), t("**Failed**")]),
		CheckState::ActionRequired => cat(&[mark(symbols, Mark::Warn), t("**Action required**")]),
		CheckState::Cancelled => cat(&[mark(symbols, Mark::Cancelled), t("Cancelled")]),
		CheckState::Running => cat(&[mark(symbols, Mark::Running), t("Running")]),
		CheckState::Skipped => cat(&[mark(symbols, Mark::Skip), t("Skipped")]),
		CheckState::Passed => cat(&[mark(symbols, Mark::Pass), t("Passed")]),
	}
}

/// A check's name. It was chosen by whoever wrote that workflow, so it goes in a code span.
pub(crate) fn check_label(name: &str) -> Safe {
	Safe::code_span(name, CHECK_CHARS)
}

/// A command from the settings, in a code span.
pub(crate) fn command_span(command: &str) -> Safe {
	Safe::code_span(command, COMMAND_CHARS)
}

/// One row of the other checks' table.
pub(crate) fn other_row(name: &str, state: CheckState, symbols: Symbols) -> Safe {
	other_row_labelled(&check_label(name), state, symbols)
}

/// The same with a label that is already `Safe`, such as the row that stands for several checks.
pub(crate) fn other_row_labelled(label: &Safe, state: CheckState, symbols: Symbols) -> Safe {
	cat(&[
		t("| "),
		label.clone(),
		t(" | "),
		state_cell(state, symbols),
		t(" |"),
	])
}

/// One table row. `label` is already `Safe`: a code span of the suite's title, or a literal.
pub(crate) fn table_row(
	label: &Safe,
	verdict: Verdict,
	required: bool,
	c: &Counts,
	symbols: Symbols,
) -> Safe {
	join(
		&t(" | "),
		&[
			cat(&[t("| "), label.clone()]),
			result_cell(verdict, required, symbols),
			count(c.passed),
			count(c.failed),
			count(c.skipped),
			count(c.flaky),
			cat(&[Safe::duration_ms(c.time_ms), t(" |")]),
		],
	)
}

/// `suite › group › name`, each in a code span, with the section's title in front when the
/// comment holds more than one section.
pub(crate) fn path_line(e: &Entry<'_>, section_title: Option<&str>) -> Safe {
	let mut parts = Vec::with_capacity(4);
	if let Some(title) = section_title {
		parts.push(Safe::code_span(title, SUITE_CHARS));
	}
	parts.push(Safe::code_span(&suite_label(e.suite), SUITE_CHARS));
	if let Some(group) = group_of(e.suite, e.case) {
		parts.push(Safe::code_span(&group, GROUP_CHARS));
	}
	parts.push(Safe::code_span(&e.case.name, NAME_CHARS));
	join(&t(" \u{203a} "), &parts)
}

/// ``at `path:line` ``, for a case that reports a whole repository path. Forgejo has no
/// annotations to carry it, so the comment does.
fn location(case: &Case) -> Option<Safe> {
	let piece = location_piece(case)?;
	Some(cat(&[t("at "), Safe::code_span(&piece, MESSAGE_CHARS)]))
}

pub(crate) fn message_line(case: &Case, target: Target) -> Safe {
	let kind = match case.status {
		Status::Errored => t("Errored: "),
		_ => Safe::default(),
	};
	let message = case
		.message
		.as_ref()
		.map_or_else(|| t("no message"), |m| Safe::code_span(m, MESSAGE_CHARS));
	let mut parts = vec![cat(&[kind, message])];
	if target == Target::Forgejo {
		parts.extend(location(case));
	}
	parts.push(Safe::duration_ms(case.time_ms));
	join(&t(" \u{b7} "), &parts)
}

/// How many earlier attempts there were, and how many of them the report could not keep.
fn attempts_line(case: &Case) -> Option<Safe> {
	let kept = case.attempts.len();
	if kept == 0 && case.attempts_omitted == 0 {
		return None;
	}
	let mut parts = vec![t("Earlier attempts: "), num(kept)];
	if case.attempts_omitted > 0 {
		parts.push(t(" shown, "));
		parts.push(num(case.attempts_omitted));
		parts.push(t(" more not kept"));
	}
	Some(cat(&parts))
}

/// A failure as a `<details open>` block. Returns its lines (empty ones are blank lines) and how
/// many lines the failure's body has, so a caller can tell whether the excerpt cut anything.
pub(crate) fn failure_lines(
	n: usize,
	total: usize,
	e: &Entry<'_>,
	section_title: Option<&str>,
	excerpt_lines: usize,
	target: Target,
	command: Option<&Piece>,
) -> (Vec<Safe>, usize) {
	let blank = Safe::default;
	let mut lines = vec![
		t("<details open>"),
		cat(&[
			t("<summary>Failure "),
			num(n),
			t(" of "),
			num(total),
			t("</summary>"),
		]),
		blank(),
		path_line(e, section_title),
		blank(),
		message_line(e.case, target),
	];
	if let Some(attempts) = attempts_line(e.case) {
		lines.push(blank());
		lines.push(attempts);
	}
	let mut body_lines = 0;
	if let Some(detail) = &e.case.detail {
		let from_end = e.suite.flavour == pr_results_core::Flavour::Steps;
		if let Some(x) = excerpt(detail, excerpt_lines, from_end) {
			body_lines = x.total_lines;
			lines.push(blank());
			if x.shown_lines < x.total_lines {
				lines.push(cat(&[
					t(if x.from_end { "Last " } else { "First " }),
					num(x.shown_lines),
					t(" of "),
					num(x.total_lines),
					t(" lines:"),
				]));
				lines.push(blank());
			}
			lines.push(Safe::code_block(&x.piece, usize::MAX));
		}
	}
	if let Some(command) = command {
		lines.push(blank());
		lines.push(t("Run it locally:"));
		lines.push(blank());
		lines.push(Safe::code_block(command, usize::MAX));
	}
	lines.push(blank());
	lines.push(t("</details>"));
	(lines, body_lines)
}

/// A flaky test as one bullet: how often it failed before it passed.
pub(crate) fn flaky_bullet(e: &Entry<'_>, section_title: Option<&str>, symbols: Symbols) -> Safe {
	let failed = e
		.case
		.attempts
		.len()
		.saturating_add(e.case.attempts_omitted);
	let times = if failed == 1 {
		t("once")
	} else {
		cat(&[num(failed), t(" times")])
	};
	let mut parts = vec![
		t("- "),
		mark(symbols, Mark::Flaky),
		path_line(e, section_title),
		t(": failed "),
		times,
		t(", passed on attempt "),
		num(failed.saturating_add(1)),
	];
	if e.case.attempts_omitted > 0 {
		parts.push(t(" ("));
		parts.push(num(e.case.attempts_omitted));
		parts.push(t(" attempts not kept)"));
	}
	cat(&parts)
}

/// The collapsed list of flaky tests, shown up to `max` of them.
pub(crate) fn flaky_lines(
	flaky: &[Entry<'_>],
	titles: Option<&[&str]>,
	max: usize,
	symbols: Symbols,
) -> Vec<Safe> {
	let mut lines = vec![
		t("<details>"),
		cat(&[
			t("<summary>Flaky: passed after a retry ("),
			count(big(flaky.len())),
			t(")</summary>"),
		]),
		Safe::default(),
	];
	for e in flaky.iter().take(max) {
		lines.push(flaky_bullet(e, section_title(titles, e), symbols));
	}
	if flaky.len() > max {
		lines.push(Safe::default());
		lines.push(cat(&[
			t("\u{2026}and "),
			count(big(flaky.len() - max)),
			noun(
				big(flaky.len() - max),
				" more flaky test.",
				" more flaky tests.",
			),
		]));
	}
	lines.push(Safe::default());
	lines.push(t("</details>"));
	lines
}

/// The slowest `n` cases of the suites, longest first; ties keep input order.
pub(crate) fn slowest(suites: &[Suite], n: usize) -> Vec<(&Suite, &Case)> {
	let mut all: Vec<(&Suite, &Case)> = suites
		.iter()
		.flat_map(|s| s.cases.iter().map(move |c| (s, c)))
		.filter(|(_, c)| c.status != Status::Skipped)
		.collect();
	all.sort_by_key(|a| std::cmp::Reverse(a.1.time_ms));
	all.truncate(n);
	all
}

/// The section's title, when the comment holds more than one section and so a failure needs it.
pub(crate) fn section_title<'a>(titles: Option<&[&'a str]>, e: &Entry<'_>) -> Option<&'a str> {
	titles.and_then(|all| all.get(e.section)).copied()
}

/// A test shorter than this is not worth listing as slow: it would be shown as `0.0 s`.
const SLOW_MS: u64 = 100;

/// The slowest `n` cases across all sections that took at least [`SLOW_MS`], longest first, as the
/// entries a failure line is made from.
pub(crate) fn slowest_overall<'a>(sections: &'a [crate::Section<'a>], n: usize) -> Vec<Entry<'a>> {
	let mut rows: Vec<Entry<'a>> = Vec::new();
	for (index, section) in sections.iter().enumerate() {
		for (suite, case) in slowest(section.suites, n) {
			rows.push(Entry {
				section: index,
				suite,
				case,
			});
		}
	}
	rows.sort_by_key(|e| std::cmp::Reverse(e.case.time_ms));
	rows.retain(|e| e.case.time_ms >= SLOW_MS);
	rows.truncate(n);
	rows
}

/// The slowest tests as a collapsed table.
pub(crate) fn slowest_block(rows: &[Entry<'_>], titles: Option<&[&str]>) -> Vec<Safe> {
	let mut lines = vec![
		t("<details>"),
		cat(&[
			t("<summary>Slowest tests ("),
			num(rows.len()),
			t(")</summary>"),
		]),
		Safe::default(),
		t("| Test | Time |"),
		t("| --- | ---: |"),
	];
	for e in rows {
		lines.push(join(
			&t(" | "),
			&[
				cat(&[t("| "), path_line(e, section_title(titles, e))]),
				cat(&[Safe::duration_ms(e.case.time_ms), t(" |")]),
			],
		));
	}
	lines.push(Safe::default());
	lines.push(t("</details>"));
	lines
}
