// `render` and `validate-config`: the commands that need no network.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `render` turns local JUnit files into the comment and the check runs' Markdown. It has no run
//! to describe, so the footer links point at a placeholder repository, and the output depends on
//! the files and the settings alone.

use std::fs;
use std::io::Read;
use std::path::Path;

use pr_results_core::{
	parse_junit, Flavour, Glob, Limits as ParseLimits, RepoSlug, Report, RunId, ServerUrl, Sha,
	Suite,
};
use pr_results_render::{check, comment, Conclusion, Input, RunContext, Section};

use crate::args::RenderArgs;
use crate::config::{self, Settings};
use crate::{CliError, Streams, VERSION};

/// The heading when neither `--title` nor the settings give one.
const DEFAULT_TITLE: &str = "Test results";

/// Where the footer links point: nothing real, because `render` has no run.
const PLACEHOLDER_REPO: &str = "example/app";
const PLACEHOLDER_SHA: &str = "3f9c2e1d4b5a69788796a5b4c3d2e1f001122334";
const PLACEHOLDER_RUN: u64 = 19_283_746;

pub(crate) fn validate(path: &Path, io: &mut Streams<'_>) -> Result<u8, CliError> {
	let settings = config::read(Some(path)).map_err(CliError::usage)?;
	let count = settings.suites.len();
	io.say(&format!(
		"{}: valid, {count} {}",
		path.display(),
		if count == 1 { "suite" } else { "suites" }
	));
	Ok(0)
}

struct Owned {
	title: String,
	check_name: String,
	required: bool,
	suites: Vec<Suite>,
}

pub(crate) fn render(a: &RenderArgs, io: &mut Streams<'_>) -> Result<u8, CliError> {
	let settings = config::read(a.config.as_deref()).map_err(CliError::usage)?;
	let title = a
		.title
		.clone()
		.or_else(|| settings.title.clone())
		.unwrap_or_else(|| DEFAULT_TITLE.to_owned());
	let sections = sections(a, &settings, &title)?;

	let ctx = placeholder_context(&settings)?;
	let options = settings.options();
	let borrowed: Vec<Section<'_>> = sections
		.iter()
		.map(|s| Section {
			title: &s.title,
			check_name: &s.check_name,
			required: s.required,
			suites: &s.suites,
			inputs_skipped: 0,
		})
		.collect();
	let input = Input {
		ctx: &ctx,
		title: &title,
		sections: &borrowed,
		options: &options,
	};
	let body = comment(&input).body.to_string();

	let Some(dir) = &a.out else {
		io.block(&body);
		return Ok(0);
	};
	let checks_dir = dir.join("checks");
	fs::create_dir_all(&checks_dir)
		.map_err(|_| CliError::usage(format!("{}: cannot be created", checks_dir.display())))?;
	write(&dir.join("comment.md"), &body)?;
	for index in 0..borrowed.len() {
		if let Some(output) = check(&input, index) {
			write(
				&checks_dir.join(format!("{index}.md")),
				&check_file(
					conclusion_word(output.conclusion),
					output.title.as_str(),
					output.summary.as_str(),
					output.text.as_str(),
				),
			)?;
		}
	}
	io.say(&format!(
		"Wrote the comment and {} check {} to {}.",
		borrowed.len(),
		if borrowed.len() == 1 { "run" } else { "runs" },
		dir.display()
	));
	Ok(0)
}

pub(crate) fn conclusion_word(c: Conclusion) -> &'static str {
	match c {
		Conclusion::Success => "success",
		Conclusion::Failure => "failure",
		Conclusion::Neutral => "neutral",
	}
}

/// The text of a check run's file, in the layout of the goldens under `tests/golden/`.
pub(crate) fn check_file(conclusion: &str, title: &str, summary: &str, text: &str) -> String {
	format!("conclusion: {conclusion}\ntitle: {title}\n\n--- summary ---\n{summary}\n--- text ---\n{text}")
}

pub(crate) fn write(path: &Path, text: &str) -> Result<(), CliError> {
	fs::write(path, text)
		.map_err(|_| CliError::usage(format!("{}: cannot be written", path.display())))
}

fn placeholder_context(settings: &Settings) -> Result<RunContext, CliError> {
	let bad = || CliError::usage("the placeholder run context is invalid");
	Ok(RunContext {
		server: ServerUrl::parse("https://github.com").ok_or_else(bad)?,
		repo: RepoSlug::parse(PLACEHOLDER_REPO).ok_or_else(bad)?,
		head_sha: Sha::parse(PLACEHOLDER_SHA).ok_or_else(bad)?,
		run_id: RunId(PLACEHOLDER_RUN),
		key: settings.key.clone(),
		tool_version: VERSION,
	})
}

/// One section per `[[suite]]` that a file matches, or one section called "Tests" when there are
/// no suites.
fn sections(a: &RenderArgs, settings: &Settings, title: &str) -> Result<Vec<Owned>, CliError> {
	let limits = ParseLimits {
		max_bytes: usize::try_from(settings.limits.max_file_bytes).unwrap_or(usize::MAX),
		max_cases: settings.limits.max_cases,
		..ParseLimits::default()
	};
	let mut sections: Vec<Owned>;
	let mut reports: Vec<Report>;
	let flavours: Vec<Flavour>;
	if settings.suites.is_empty() {
		sections = vec![Owned {
			title: "Tests".to_owned(),
			check_name: config::default_check_name(title, "tests"),
			required: false,
			suites: Vec::new(),
		}];
		reports = vec![Report::default()];
		flavours = vec![a.flavour.unwrap_or(Flavour::Auto)];
	} else {
		sections = settings
			.suites
			.iter()
			.map(|s| Owned {
				title: s.name.clone(),
				check_name: s
					.check
					.clone()
					.unwrap_or_else(|| config::default_check_name(title, &s.name)),
				required: s.required,
				suites: Vec::new(),
			})
			.collect();
		reports = settings.suites.iter().map(|_| Report::default()).collect();
		flavours = settings
			.suites
			.iter()
			.map(|s| a.flavour.unwrap_or(s.flavour))
			.collect();
	}

	for path in &a.files {
		let index = if settings.suites.is_empty() {
			0
		} else {
			let patterns = settings.suites.iter().map(|s| s.files.as_slice());
			which_suite(path, patterns).ok_or_else(|| {
				CliError::usage(format!(
					"{}: matches no suite's `files` in the settings",
					path.display()
				))
			})?
		};
		let bytes = read_file(path, limits.max_bytes)?;
		let flavour = flavours.get(index).copied().unwrap_or(Flavour::Auto);
		let parsed = parse_junit(&bytes, flavour, &limits)
			.map_err(|e| CliError::usage(format!("{}: {e}", path.display())))?;
		if let Some(report) = reports.get_mut(index) {
			report.suites.extend(parsed.suites);
		}
	}
	for (section, mut report) in sections.iter_mut().zip(reports) {
		report.merge_same_named();
		section.suites = report.suites;
	}
	Ok(sections)
}

/// The first suite whose `files` patterns match `path`, or any tail of it, so that
/// `**/junit.xml` finds `target/nextest/ci/junit.xml` and `junit.xml` finds the same file.
fn which_suite<'p>(path: &Path, suites: impl Iterator<Item = &'p [Glob]>) -> Option<usize> {
	let text = path.to_string_lossy().replace('\\', "/");
	let text = text.trim_start_matches("./");
	let mut tails: Vec<&str> = vec![text];
	let mut rest = text;
	while let Some((_, tail)) = rest.split_once('/') {
		tails.push(tail);
		rest = tail;
	}
	suites
		.enumerate()
		.find(|(_, globs)| globs.iter().any(|g| tails.iter().any(|t| g.is_match(t))))
		.map(|(i, _)| i)
}

fn read_file(path: &Path, max: usize) -> Result<Vec<u8>, CliError> {
	let file = fs::File::open(path)
		.map_err(|_| CliError::usage(format!("{}: cannot be opened", path.display())))?;
	let mut bytes = Vec::new();
	file.take(max as u64 + 1)
		.read_to_end(&mut bytes)
		.map_err(|_| CliError::usage(format!("{}: cannot be read", path.display())))?;
	if bytes.len() > max {
		return Err(CliError::usage(format!(
			"{}: is larger than the file limit of {max} bytes",
			path.display()
		)));
	}
	Ok(bytes)
}
