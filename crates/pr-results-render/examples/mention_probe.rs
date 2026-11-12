// Writes a Markdown probe that shows whether GitHub turns names into mentions or links.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Run it, post the output as a comment in a throwaway repository, and read the comment's
//! rendered HTML (decision Q9). Every target appears four ways through the renderer's own
//! forms (`text`, `code_span`, `code_block` and the failure layout) and once raw, as a control.
//! The heading's title (which goes through `Safe::text`) carries every target too, and the
//! table cells and test names also carry targets with a pipe, a backtick and an escaped pipe.
//! The targets include a real team (`@OWNER/TEAM`) and a reference to another real repository
//! (`OTHER#1`).
//!
//! With `--check-run` it writes instead the JSON body of a check run, as the renderer builds it
//! from the same failures: the title, summary and text of the check, and its annotations. Each
//! annotation carries a target in its title and message, on a line of the sandbox's `README.md`.
//! Post it with `POST /repos/{owner}/{repo}/check-runs` from a workflow with `checks: write`.
//!
//! In the rendered HTML, a mention, issue link or autolink anchor should appear only for the raw
//! rows. The timeline of the issues and the notifications of an account that watches the
//! sandbox should show only the raw rows too. The workflow that posts both is
//! `docs/probe-workflow.yml`.
//!
//! ```text
//! cargo run -p pr-results-render --example mention_probe -- [OWNER [REPO [TEAM [OTHER]]]] > probe.md
//! cargo run -p pr-results-render --example mention_probe -- --check-run --head-sha SHA \
//!     [OWNER [REPO [TEAM [OTHER]]]] > check-run.json
//! ```
//!
//! The sandbox needs an issue number 1 and a team `TEAM` with a member who is not the poster;
//! `OTHER` is `owner/name` of a second repository with an issue number 1; and an autolink
//! reference `GH-` that points at `https://example.invalid/<num>`.

use std::process::ExitCode;

use pr_results_core::{
	parse_junit, CommentKey, Flavour, Limits, RepoSlug, RunId, Safe, ServerUrl, Sha,
};
use pr_results_render::{
	check, comment, job_summary, Input, Level, Options, RunContext, Section, Target,
};

const HELP: &str = "\
mention_probe: write a Markdown probe for GitHub's mention and link filters

Usage:
  mention_probe [OWNER [REPO [TEAM [OTHER]]]] > probe.md
  mention_probe --check-run --head-sha SHA [OWNER [REPO [TEAM [OTHER]]]] > check-run.json

OWNER, REPO and TEAM name the sandbox: an account, its repository and a team in it. OTHER is
owner/name of a second repository. They default to example-owner, sandbox, example-team and
example-owner/other. With --check-run the output is the JSON body of a check run for the commit
SHA, with annotations, instead of the comment.
";

fn targets(owner: &str, repo: &str, team: &str, other: &str) -> Vec<String> {
	vec![
		format!("@{owner}"),
		format!("@{owner}/{team}"),
		"@everyone".to_owned(),
		"#1".to_owned(),
		format!("{owner}/{repo}#1"),
		format!("{other}#1"),
		"GH-1".to_owned(),
		"https://example.invalid/probe".to_owned(),
		"www.example.invalid".to_owned(),
	]
}

/// Targets that could break out of a table cell or a code span. They are probed in the rendered
/// forms only, because raw they would only break the control table.
fn awkward(owner: &str) -> Vec<String> {
	vec![
		format!("@{owner}|#1"),
		format!("x`@{owner}`#1"),
		format!("@{owner}\\|#1"),
	]
}

/// XML text for an attribute value.
fn attr(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('"', "&quot;")
}

/// The failures of the probe: one case per target, each carrying the target in its class, name,
/// message, body and (a path that can be annotated) file.
fn probe_suites(targets: &[String]) -> Option<Vec<pr_results_core::Suite>> {
	let mut xml = String::from("<testsuites><testsuite name=\"probe\">");
	for (n, target) in targets.iter().enumerate() {
		let t = attr(target);
		xml.push_str(&format!(
			"<testcase classname=\"{t}\" name=\"{t}\" file=\"README.md\" line=\"{}\" time=\"0.1\"><failure message=\"{t}\">{t}\n{t}</failure></testcase>",
			n + 1
		));
	}
	xml.push_str("</testsuite></testsuites>");
	let report = parse_junit(xml.as_bytes(), Flavour::Generic, &Limits::default()).ok()?;
	Some(report.suites)
}

fn probe_options(targets: &[String]) -> Options {
	Options {
		max_failures: targets.len(),
		max_annotations: targets.len(),
		..Options::default()
	}
}

fn failure_layout(ctx: &RunContext, title: &str, targets: &[String]) -> Option<Safe> {
	let suites = probe_suites(targets)?;
	let options = probe_options(targets);
	let sections = [Section {
		title: "Probe",
		check_name: "Probe",
		required: false,
		suites: &suites,
		inputs_skipped: 0,
	}];
	let input = Input {
		ctx,
		title,
		sections: &sections,
		options: &options,
	};
	// Without the marker, so the probe can never be taken for a real comment.
	Some(job_summary(&comment(&input)))
}

/// A JSON string literal.
fn json(s: &str) -> String {
	let mut out = String::from("\"");
	for c in s.chars() {
		match c {
			'"' => out.push_str("\\\""),
			'\\' => out.push_str("\\\\"),
			'\n' => out.push_str("\\n"),
			'\r' => out.push_str("\\r"),
			'\t' => out.push_str("\\t"),
			c if c.is_control() => out.push_str(&format!("\\u{:04x}", u32::from(c))),
			c => out.push(c),
		}
	}
	out.push('"');
	out
}

/// The body of `POST /repos/{owner}/{repo}/check-runs`: the renderer's check for the probe
/// failures, with their annotations. The check is named by its heading's title, so the title
/// of the check run carries every target as well.
fn check_run_body(ctx: &RunContext, title: &str, targets: &[String]) -> Option<String> {
	let suites = probe_suites(targets)?;
	let options = probe_options(targets);
	let sections = [Section {
		title: title.trim(),
		check_name: "mention probe",
		required: false,
		suites: &suites,
		inputs_skipped: 0,
	}];
	let input = Input {
		ctx,
		title,
		sections: &sections,
		options: &options,
	};
	let out = check(&input, 0)?;
	let annotations: Vec<String> = out
		.annotations
		.iter()
		.map(|a| {
			let level = match a.level {
				Level::Notice => "notice",
				Level::Warning => "warning",
				Level::Failure => "failure",
			};
			format!(
				"{{\"path\":{},\"start_line\":{},\"end_line\":{},\"annotation_level\":\"{level}\",\"title\":{},\"message\":{}}}",
				json(a.path.as_str()),
				a.start_line,
				a.end_line,
				json(a.title.as_str()),
				json(a.message.as_str())
			)
		})
		.collect();
	Some(format!(
		"{{\"name\":\"mention probe\",\"head_sha\":{},\"status\":\"completed\",\"conclusion\":\"neutral\",\"output\":{{\"title\":{},\"summary\":{},\"text\":{},\"annotations\":[{}]}}}}\n",
		json(ctx.head_sha.as_str()),
		json(out.title.as_str()),
		json(out.summary.as_str()),
		json(out.text.as_str()),
		annotations.join(",")
	))
}

fn main() -> ExitCode {
	let mut check_run = false;
	let mut head_sha: Option<String> = None;
	let mut positional: Vec<String> = Vec::new();
	let mut args = std::env::args().skip(1);
	while let Some(a) = args.next() {
		match a.as_str() {
			"--help" | "-h" => {
				print!("{HELP}");
				return ExitCode::SUCCESS;
			}
			"--check-run" => check_run = true,
			"--head-sha" => head_sha = args.next(),
			_ => positional.push(a),
		}
	}
	let arg = |i: usize, default: &str| {
		positional
			.get(i)
			.cloned()
			.unwrap_or_else(|| default.to_owned())
	};
	let (owner, repo, team, other) = (
		arg(0, "example-owner"),
		arg(1, "sandbox"),
		arg(2, "example-team"),
		arg(3, "example-owner/other"),
	);
	let slug = format!("{owner}/{repo}");
	let sha = head_sha.unwrap_or_else(|| "0".repeat(40));
	let ctx = match (
		ServerUrl::parse("https://github.com"),
		RepoSlug::parse(&slug),
		Sha::parse(&sha),
		CommentKey::parse("probe"),
	) {
		(Some(server), Some(repo), Some(head_sha), Some(key)) => RunContext {
			server,
			repo,
			head_sha,
			run_id: RunId(1),
			run_number: None,
			target: Target::GitHub,
			key,
			tool_version: "probe",
		},
		_ => {
			eprintln!(
				"mention_probe: {slug:?} is not an owner and repository name, or {sha:?} is not a commit"
			);
			return ExitCode::from(2);
		}
	};
	let controls = targets(&owner, &repo, &team, &other);
	let mut targets = controls.clone();
	targets.extend(awkward(&owner));
	let title = targets.join(" ");

	if check_run {
		return match check_run_body(&ctx, &title, &targets) {
			Some(body) => {
				print!("{body}");
				ExitCode::SUCCESS
			}
			None => {
				eprintln!("mention_probe: the probe check run could not be built");
				ExitCode::from(2)
			}
		};
	}

	println!("# Mention probe\n");
	println!("Only the raw control rows should link or mention anyone.\n");

	println!("## Raw control\n");
	println!("| Form | Target |\n| --- | --- |");
	for target in &controls {
		println!("| raw | {target} |");
	}

	println!("\n## text\n");
	println!("| Form | Target |\n| --- | --- |");
	for target in &targets {
		println!("| text | {} |", Safe::text(target.as_str()));
	}

	println!("\n## code_span\n");
	println!("| Form | Target |\n| --- | --- |");
	for target in &targets {
		println!("| code_span | {} |", Safe::code_span(target.as_str(), 200));
	}

	println!("\n## code_block\n");
	for target in &targets {
		println!("{}\n", Safe::code_block(target.as_str(), 200));
	}

	println!("## Failure layout\n");
	println!("The heading carries every target as its title.\n");
	match failure_layout(&ctx, &title, &targets) {
		Some(layout) => print!("{layout}"),
		None => {
			eprintln!("mention_probe: the probe report could not be built");
			return ExitCode::from(2);
		}
	}
	ExitCode::SUCCESS
}
