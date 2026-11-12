// `publish`: check the environment, read the settings and the event, then run the publish flow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Publishing runs only from a `workflow_run` workflow on the default branch of the repository
//! the event is for. Anything else is refused with exit status 2 before a request is made, so a
//! misplaced job cannot post with a token it should not have. In-job publishing is planned for
//! v0.2.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use pr_results_core::{Flavour, Glob, Limits as ParseLimits, RepoSlug, ServerUrl};
use pr_results_forge::{
	publish, requests_to_json, ApiRequest, ArchiveLimits, DryRun, GitHub, Host, Method, Plan,
	PublishReport, SectionPlan, Token, WorkflowRun, DEFAULT_BOT_LOGIN,
};
use pr_results_render::Conclusion;
use serde_json::Value;

use crate::args::PublishArgs;
use crate::config::{self, CommentMode, Settings};
use crate::event::{parse_event, Event, MAX_EVENT_BYTES};
use crate::render_cmd::{check_file, write};
use crate::{CliError, ForgeFactory, Streams, VERSION};

/// Where the settings are read from when `--config` is not given. A missing file is fine then.
const DEFAULT_CONFIG: &str = ".github/pr-results.toml";
/// At most this many suites are inferred from artefact names.
const MAX_INFERRED: usize = 20;
const INFERRED_MARK: &str = "-test-results";

type Vars<'a> = &'a dyn Fn(&str) -> Option<String>;

/// What the environment says, checked.
struct Environment {
	repo: RepoSlug,
	server: ServerUrl,
	api_base: String,
	event: Event,
}

pub(crate) fn publish_command<X: ForgeFactory>(
	a: &PublishArgs,
	vars: Vars<'_>,
	factory: &X,
	io: &mut Streams<'_>,
) -> Result<u8, CliError> {
	let get = |name: &str| vars(name).filter(|v| !v.is_empty());
	let env = environment(&get)?;
	let settings = load_settings(a.config.as_deref())?;
	let Some(run) = env.event.run.clone() else {
		io.say("The run's head repository has been deleted, so there is nothing to verify: nothing to publish.");
		return Ok(0);
	};
	if matches!(run.conclusion.as_deref(), Some("cancelled" | "skipped")) {
		io.say("The run was cancelled or skipped: nothing to publish.");
		return Ok(0);
	}

	// GitHub's runner sets one, and so does Forgejo's from 13.2.
	let summary = get("GITHUB_STEP_SUMMARY")
		.or_else(|| get("FORGEJO_STEP_SUMMARY"))
		.map(PathBuf::from);

	let bot = match &a.bot_login {
		Some(login) if valid_login(login) => login.clone(),
		Some(_) => return Err(CliError::usage("--bot-login is not a GitHub login")),
		None => DEFAULT_BOT_LOGIN.to_owned(),
	};
	let token = get(&a.token_env)
		.and_then(|t| Token::new(&t))
		.ok_or_else(|| {
			CliError::usage(format!(
				"the token is not in {}: set it in the step's `env`",
				a.token_env
			))
		})?;
	// Before any request, so the runner hides it from every later line.
	io.command(&token.mask_command());
	let forge = factory
		.make(&env.api_base, token)
		.map_err(|e| CliError::usage(format!("the API address cannot be used: {e}")))?;

	let report = match &a.dry_run {
		Some(dir) => {
			let gh = GitHub::new(DryRun::new(forge), env.repo.clone(), &bot);
			let report = execute(&gh, &run, &env, &settings, summary.as_deref(), io)?;
			write_dry_run(dir, &gh.forge().requests(), io)?;
			report
		}
		None => {
			let gh = GitHub::new(forge, env.repo.clone(), &bot);
			execute(&gh, &run, &env, &settings, summary.as_deref(), io)?
		}
	};
	Ok(exit_code(&report, a.fail_on_failures))
}

/// 1 when something the plan asked for was not written, else 4 when tests failed and the caller
/// asked to hear of it, else 0.
fn exit_code(report: &PublishReport, fail_on_failures: bool) -> u8 {
	if report.failed() {
		1
	} else if fail_on_failures
		&& report
			.checks
			.iter()
			.any(|c| c.conclusion == Conclusion::Failure)
	{
		4
	} else {
		0
	}
}

fn valid_login(login: &str) -> bool {
	!login.is_empty()
		&& login.len() <= 64
		&& login
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'[' | b']'))
}

/// The preconditions. Every failure is exit status 2.
fn environment(get: &dyn Fn(&str) -> Option<String>) -> Result<Environment, CliError> {
	let need = |name: &str| {
		get(name).ok_or_else(|| {
			CliError::usage(format!("{name} is not set: this is not a workflow run"))
		})
	};
	let event_name = need("GITHUB_EVENT_NAME")?;
	if event_name != "workflow_run" {
		return Err(CliError::usage(format!(
			"publish runs only in a workflow_run workflow, not on {event_name}: see docs/adopting.md"
		)));
	}
	let repo = RepoSlug::parse(&need("GITHUB_REPOSITORY")?)
		.ok_or_else(|| CliError::usage("GITHUB_REPOSITORY is not owner/name"))?;
	let git_ref = need("GITHUB_REF")?;
	let api_base = need("GITHUB_API_URL")?;
	if !api_base.starts_with("https://") {
		return Err(CliError::usage("GITHUB_API_URL is not an https:// address"));
	}
	let server = ServerUrl::parse(need("GITHUB_SERVER_URL")?.trim_end_matches('/'))
		.ok_or_else(|| CliError::usage("GITHUB_SERVER_URL is not an https:// address"))?;
	let path = need("GITHUB_EVENT_PATH")?;
	let bytes = read_event(Path::new(&path))?;
	let event = parse_event(&bytes).map_err(CliError::usage)?;

	if git_ref != format!("refs/heads/{}", event.default_branch) {
		return Err(CliError::usage(
			"publish runs only on the default branch: GITHUB_REF is not the default branch",
		));
	}
	let same = |a: &RepoSlug, b: &RepoSlug| a.to_string().eq_ignore_ascii_case(&b.to_string());
	if !same(&event.repository, &repo) || !same(&event.run_repository, &repo) {
		return Err(CliError::usage(
			"the event is for another repository than GITHUB_REPOSITORY",
		));
	}
	Ok(Environment {
		repo,
		server,
		api_base,
		event,
	})
}

fn read_event(path: &Path) -> Result<Vec<u8>, CliError> {
	let file =
		fs::File::open(path).map_err(|_| CliError::usage("GITHUB_EVENT_PATH cannot be opened"))?;
	let mut bytes = Vec::new();
	file.take(MAX_EVENT_BYTES + 1)
		.read_to_end(&mut bytes)
		.map_err(|_| CliError::usage("GITHUB_EVENT_PATH cannot be read"))?;
	if bytes.len() as u64 > MAX_EVENT_BYTES {
		return Err(CliError::usage("the event file is larger than 8 MiB"));
	}
	Ok(bytes)
}

/// `--config` must exist. Without it the default path is read if it is there.
fn load_settings(explicit: Option<&Path>) -> Result<Settings, CliError> {
	let default = Path::new(DEFAULT_CONFIG);
	let path = match explicit {
		Some(p) => Some(p),
		None if default.is_file() => Some(default),
		None => None,
	};
	config::read(path).map_err(CliError::usage)
}

fn execute<H: Host>(
	gh: &H,
	run: &WorkflowRun,
	env: &Environment,
	settings: &Settings,
	summary: Option<&Path>,
	io: &mut Streams<'_>,
) -> Result<PublishReport, CliError> {
	let title = settings
		.title
		.clone()
		.unwrap_or_else(|| config::title_from_repo(env.repo.name()));
	let sections = if settings.suites.is_empty() {
		inferred_sections(gh, run, &title, io)?
	} else {
		configured_sections(settings, &title)?
	};
	let plan = plan(settings, &title, sections);
	let report = publish(gh, run, &env.server, &plan)
		.map_err(|e| CliError::failed(format!("publishing stopped: {e}")))?;
	let lines = report.log_lines();
	for line in &lines {
		io.say(line);
	}
	if let Some(path) = summary {
		summarize_to(path, report.job_summary.as_deref(), &lines, io);
	}
	Ok(report)
}

fn plan(settings: &Settings, title: &str, sections: Vec<SectionPlan>) -> Plan {
	let mut plan = Plan::new(title, settings.key.clone(), sections, VERSION);
	plan.post_comment = settings.mode == CommentMode::Always;
	plan.options = settings.options();
	let l = &settings.limits;
	plan.archive = ArchiveLimits {
		max_files: l.max_files,
		max_entry_bytes: l.max_file_bytes,
		max_total_bytes: l.max_artifact_bytes,
	};
	plan.parse = ParseLimits {
		max_bytes: usize::try_from(l.max_file_bytes).unwrap_or(usize::MAX),
		max_cases: l.max_cases,
		..ParseLimits::default()
	};
	plan.max_download_bytes = l.max_artifact_bytes;
	plan
}

fn configured_sections(settings: &Settings, title: &str) -> Result<Vec<SectionPlan>, CliError> {
	let mut seen = BTreeSet::new();
	let mut out = Vec::new();
	for s in &settings.suites {
		let check_name = s
			.check
			.clone()
			.unwrap_or_else(|| config::default_check_name(title, &s.name));
		if check_name.chars().count() > 100 {
			return Err(CliError::usage(
				"a check name is longer than 100 characters: shorten the `title` or set a `check`",
			));
		}
		if !seen.insert(check_name.clone()) {
			return Err(CliError::usage(
				"two suites have the same check name: give one a different `check`",
			));
		}
		out.push(SectionPlan {
			title: s.name.clone(),
			check_name,
			required: s.required,
			artifact: s.artifact.clone(),
			files: s.files.clone(),
			flavour: s.flavour,
		});
	}
	Ok(out)
}

/// With no `[[suite]]`, one suite for each `<stem>-test-results*` artefact. Artefact names are
/// chosen by whoever wrote the workflow, a fork included, and become check names, so only plain
/// names are used: letters, digits, `.`, `_` and `-`.
fn inferred_sections<H: Host>(
	gh: &H,
	run: &WorkflowRun,
	title: &str,
	io: &mut Streams<'_>,
) -> Result<Vec<SectionPlan>, CliError> {
	let artifacts = gh
		.artifacts(run.id)
		.map_err(|e| CliError::failed(format!("publishing stopped: listing artefacts: {e}")))?;
	let mut stems = BTreeSet::new();
	for artifact in &artifacts {
		let name = artifact.name.as_str();
		let plain = !name.is_empty()
			&& name.len() <= 100
			&& name
				.bytes()
				.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
		if let (true, Some(at)) = (plain, name.find(INFERRED_MARK)) {
			if let Some(stem) = name.get(..at).filter(|s| !s.is_empty()) {
				stems.insert(stem.to_owned());
			}
		}
	}
	let mut seen = BTreeSet::new();
	let mut out = Vec::new();
	for stem in stems {
		let suite = config::suite_name_from_stem(&stem);
		let check_name = config::default_check_name(title, &suite);
		if out.len() >= MAX_INFERRED {
			io.say(&format!(
				"More than {MAX_INFERRED} result artefacts: the rest are not read. Name the suites in the settings file."
			));
			break;
		}
		if check_name.chars().count() > 100 || !seen.insert(check_name.clone()) {
			continue;
		}
		let artifact = Glob::new(&format!("{stem}{INFERRED_MARK}*"))
			.map_err(|e| CliError::usage(format!("an artefact name cannot be matched: {e}")))?;
		let files = Glob::new("**/*.xml")
			.map_err(|e| CliError::usage(format!("a file pattern cannot be used: {e}")))?;
		out.push(SectionPlan {
			title: suite,
			check_name,
			required: false,
			artifact,
			files: vec![files],
			flavour: Flavour::Auto,
		});
	}
	Ok(out)
}

/// Appends the comment (without its marker), then the diagnostics, to the job summary. Failing
/// to write it never fails the run.
pub(crate) fn summarize_to(
	path: &Path,
	comment: Option<&str>,
	lines: &[String],
	io: &mut Streams<'_>,
) {
	let mut text = String::new();
	if let Some(comment) = comment.filter(|c| !c.is_empty()) {
		text.push_str(comment);
		text.push('\n');
	}
	text.push_str("### pr-results\n\n");
	for line in lines {
		text.push_str("- ");
		text.push_str(line);
		text.push('\n');
	}
	let written = fs::OpenOptions::new()
		.append(true)
		.create(true)
		.open(path)
		.and_then(|mut f| f.write_all(text.as_bytes()));
	if written.is_err() {
		io.say("The job summary could not be written.");
	}
}

// ---- --dry-run -----------------------------------------------------------------------------

fn write_dry_run(
	dir: &Path,
	requests: &[ApiRequest],
	io: &mut Streams<'_>,
) -> Result<(), CliError> {
	let checks_dir = dir.join("checks");
	fs::create_dir_all(&checks_dir)
		.map_err(|_| CliError::usage(format!("{}: cannot be created", checks_dir.display())))?;
	write(&dir.join("requests.json"), &requests_to_json(requests))?;

	let comment = requests.iter().find_map(|r| {
		let is_comment = r.path.path().contains("/issues/") && r.path.path().contains("comments");
		let writes = matches!(r.method, Method::Post | Method::Patch);
		if !(is_comment && writes) {
			return None;
		}
		let body: Value = serde_json::from_slice(r.body.as_deref()?).ok()?;
		Some(body.get("body")?.as_str()?.to_owned())
	});
	match &comment {
		Some(text) => {
			write(&dir.join("comment.md"), text)?;
			io.block(text);
		}
		None => io.say("No comment would be written."),
	}

	// A check run is written by one request, and again for each further batch of annotations,
	// every one carrying its `external_id`: the first of each is the file.
	let mut seen: Vec<String> = Vec::new();
	for r in requests {
		if !r.path.path().contains("/check-runs")
			|| !matches!(r.method, Method::Post | Method::Patch)
		{
			continue;
		}
		let Some(body) = r
			.body
			.as_deref()
			.and_then(|b| serde_json::from_slice::<Value>(b).ok())
		else {
			continue;
		};
		let id = body
			.get("external_id")
			.and_then(Value::as_str)
			.unwrap_or_default();
		if seen.iter().any(|s| s == id) {
			continue;
		}
		seen.push(id.to_owned());
		let text = |path: &[&str]| {
			let mut at = &body;
			for key in path {
				at = at.get(key).unwrap_or(&Value::Null);
			}
			at.as_str().unwrap_or_default().to_owned()
		};
		write(
			&checks_dir.join(format!("{}.md", seen.len() - 1)),
			&check_file(
				&text(&["conclusion"]),
				&text(&["output", "title"]),
				&text(&["output", "summary"]),
				&text(&["output", "text"]),
			),
		)?;
	}
	io.say(&format!(
		"Wrote requests.json and {} check {} to {}.",
		seen.len(),
		if seen.len() == 1 { "file" } else { "files" },
		dir.display()
	));
	Ok(())
}
