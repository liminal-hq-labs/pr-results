// Writes a Markdown probe that shows whether GitHub turns names into mentions or links.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Run it, post the output as a comment in a throwaway repository, and read the comment's
//! rendered HTML (decision Q9). Every target appears four ways through the renderer's own
//! forms (`text`, `code_span`, `code_block` and the failure layout) and once raw, as a control.
//! The heading's title (which goes through `Safe::text`) carries every target too, and the
//! table cells and test names also carry targets with a pipe, a backtick and an escaped pipe.
//! Annotations and check-run summaries are not probed: annotations go through the REST API as
//! plain text, and a check run's summary and text use the same forms as the comment.
//! In the rendered HTML, a mention, issue link or autolink anchor should appear only for the raw
//! rows. The timeline and the notifications of the account that owns the repository should show
//! only the raw rows too.
//!
//! ```text
//! cargo run -p pr-results-render --example mention_probe -- [OWNER [REPO [TEAM]]] > probe.md
//! ```
//!
//! The sandbox needs an issue number 1, and an autolink reference `GH-` that points at
//! `https://example.invalid/<num>`.

use std::process::ExitCode;

use pr_results_core::{
	parse_junit, CommentKey, Flavour, Limits, RepoSlug, RunId, Safe, ServerUrl, Sha,
};
use pr_results_render::{comment, job_summary, Input, Options, RunContext, Section};

const HELP: &str = "\
mention_probe: write a Markdown probe for GitHub's mention and link filters

Usage:
  mention_probe [OWNER [REPO [TEAM]]] > probe.md

OWNER, REPO and TEAM name the sandbox: an account, its repository and a team in it.
They default to example-owner, sandbox and example-team.
";

fn targets(owner: &str, repo: &str, team: &str) -> Vec<String> {
	vec![
		format!("@{owner}"),
		format!("@{owner}/{team}"),
		"@everyone".to_owned(),
		"#1".to_owned(),
		format!("{owner}/{repo}#1"),
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

fn failure_layout(ctx: &RunContext, title: &str, targets: &[String]) -> Option<Safe> {
	let mut xml = String::from("<testsuites><testsuite name=\"probe\">");
	for target in targets {
		let t = attr(target);
		xml.push_str(&format!(
			"<testcase classname=\"{t}\" name=\"{t}\" time=\"0.1\"><failure message=\"{t}\">{t}\n{t}</failure></testcase>"
		));
	}
	xml.push_str("</testsuite></testsuites>");
	let report = parse_junit(xml.as_bytes(), Flavour::Generic, &Limits::default()).ok()?;
	let options = Options {
		max_failures: targets.len(),
		..Options::default()
	};
	let sections = [Section {
		title: "Probe",
		check_name: "Probe",
		required: false,
		suites: &report.suites,
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

fn main() -> ExitCode {
	let args: Vec<String> = std::env::args().skip(1).collect();
	if args.iter().any(|a| a == "--help" || a == "-h") {
		print!("{HELP}");
		return ExitCode::SUCCESS;
	}
	let arg = |i: usize, default: &str| args.get(i).cloned().unwrap_or_else(|| default.to_owned());
	let (owner, repo, team) = (
		arg(0, "example-owner"),
		arg(1, "sandbox"),
		arg(2, "example-team"),
	);
	let slug = format!("{owner}/{repo}");
	let ctx = match (
		ServerUrl::parse("https://github.com"),
		RepoSlug::parse(&slug),
		Sha::parse(&"0".repeat(40)),
		CommentKey::parse("probe"),
	) {
		(Some(server), Some(repo), Some(head_sha), Some(key)) => RunContext {
			server,
			repo,
			head_sha,
			run_id: RunId(1),
			key,
			tool_version: "probe",
		},
		_ => {
			eprintln!("mention_probe: {slug:?} is not an owner and repository name");
			return ExitCode::from(2);
		}
	};
	let controls = targets(&owner, &repo, &team);
	let mut targets = controls.clone();
	targets.extend(awkward(&owner));
	let title = targets.join(" ");

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
