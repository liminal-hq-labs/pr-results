// `publish`: check the environment, read the settings and the event, then run the publish flow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A `workflow_run` publisher runs on the default branch. With `--from`, publishing uses the
//! current pull request or push and bounded local inputs; fork pull requests are skipped.
//!
//! Before any request the forge is settled: its kind (`--forge`, else `[forge] kind`, and a
//! Forgejo runner on a `github` kind is refused), its API address (on the runner's own host and
//! `https`, so the token cannot be sent elsewhere) and its CA file.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use pr_results_core::{Flavour, Glob, Limits as ParseLimits, RepoSlug, ServerUrl};
use pr_results_forge::{
	publish, publish_inputs, requests_to_json, ApiRequest, ArchiveLimits, CaBundle, DryRun, Forge,
	ForgeError, Forgejo, GitHub, Host, HostKind, Method, Plan, PublishReport, SectionPlan, Token,
	WorkflowRun, DEFAULT_BOT_LOGIN,
};
use pr_results_render::Conclusion;
use serde_json::Value;

use crate::args::{PublishArgs, DEFAULT_TOKEN_ENV};
use crate::config::{self, valid_login, CommentMode, ForgeSettings, Settings};
use crate::event::{parse_event, parse_in_job, Event, MAX_EVENT_BYTES};
use crate::render_cmd::{check_file, write};
use crate::{CliError, ForgeFactory, ForgeSetup, Streams, VERSION};

/// The token variable a Forgejo runner exports, tried after `GITHUB_TOKEN`.
const FORGEJO_TOKEN_ENV: &str = "FORGEJO_TOKEN";
/// A runner that sets this is a Forgejo's.
const FORGEJO_SERVER_ENV: &str = "FORGEJO_SERVER_URL";

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
	/// `GITHUB_SERVER_URL` as written (without a trailing slash), which `server` was parsed from.
	server_text: String,
	/// `GITHUB_API_URL`, if the runner set one. Whether it is usable is decided with the forge.
	api_env: Option<String>,
	event: Event,
}

pub(crate) fn publish_command<X: ForgeFactory>(
	a: &PublishArgs,
	vars: Vars<'_>,
	factory: &X,
	io: &mut Streams<'_>,
) -> Result<u8, CliError> {
	let get = |name: &str| vars(name).filter(|v| !v.is_empty());
	let forgejo_config = a.forge == Some(HostKind::Forgejo)
		|| (a.forge.is_none() && get(FORGEJO_SERVER_ENV).is_some());
	let settings = load_settings(a.config.as_deref(), forgejo_config)?;
	let kind = forge_kind(a, &settings.forge, &get)?;
	let env = environment(
		&get,
		a.from.is_some(),
		kind == HostKind::Forgejo && settings.forge.allow_http,
	)?;
	let setup = forge_setup(kind, &settings.forge, &env)?;
	let Some(run) = env.event.run.clone() else {
		io.say("The run's head repository has been deleted, so there is nothing to verify: nothing to publish.");
		return Ok(0);
	};
	if a.from.is_some()
		&& run.event == "pull_request"
		&& !run
			.head_repo
			.to_string()
			.eq_ignore_ascii_case(&env.repo.to_string())
	{
		io.say("A fork pull request has a read-only token: nothing to publish.");
		return Ok(0);
	}
	if matches!(run.conclusion.as_deref(), Some("cancelled" | "skipped")) {
		io.say("The run was cancelled or skipped: nothing to publish.");
		return Ok(0);
	}

	// GitHub's runner sets one, and so does Forgejo's from 13.2.
	let summary = get("GITHUB_STEP_SUMMARY")
		.or_else(|| get("FORGEJO_STEP_SUMMARY"))
		.map(PathBuf::from);

	let bot = match a
		.bot_login
		.clone()
		.or_else(|| settings.forge.bot_login.clone())
	{
		Some(login) if valid_login(&login) => Some(login),
		Some(_) => return Err(CliError::usage("--bot-login is not a login")),
		None => None,
	};
	let token = token(a, kind, &get)?;
	// Before any request, so the runner hides it from every later line.
	io.command(&token.mask_command());
	let forge = factory
		.make(&setup, token)
		.map_err(|e| CliError::usage(format!("the API address cannot be used: {e}")))?;

	let report = match (kind, &a.dry_run) {
		(HostKind::GitHub, Some(dir)) => {
			let bot = bot.as_deref().unwrap_or(DEFAULT_BOT_LOGIN);
			let gh = GitHub::new(DryRun::new(forge), env.repo.clone(), bot);
			let report = execute(
				&gh,
				&run,
				&env,
				&settings,
				summary.as_deref(),
				a.from.as_deref(),
				io,
			)?;
			write_dry_run(dir, &gh.forge().requests(), io)?;
			report
		}
		(HostKind::GitHub, None) => {
			let bot = bot.as_deref().unwrap_or(DEFAULT_BOT_LOGIN);
			let gh = GitHub::new(forge, env.repo.clone(), bot);
			execute(
				&gh,
				&run,
				&env,
				&settings,
				summary.as_deref(),
				a.from.as_deref(),
				io,
			)?
		}
		(HostKind::Forgejo, Some(dir)) => {
			let host = forgejo(DryRun::new(forge), &env, bot.as_deref())?;
			let report = execute(
				&host,
				&run,
				&env,
				&settings,
				summary.as_deref(),
				a.from.as_deref(),
				io,
			)?;
			write_dry_run(dir, &host.forge().requests(), io)?;
			report
		}
		(HostKind::Forgejo, None) => {
			let host = forgejo(forge, &env, bot.as_deref())?;
			execute(
				&host,
				&run,
				&env,
				&settings,
				summary.as_deref(),
				a.from.as_deref(),
				io,
			)?
		}
	};
	Ok(exit_code(&report, a.fail_on_failures))
}

/// The Forgejo host, with the token's account checked when a bot login names one: a token that
/// belongs to another account is a usage error, found before anything is written.
fn forgejo<F: Forge>(
	forge: F,
	env: &Environment,
	bot: Option<&str>,
) -> Result<Forgejo<F>, CliError> {
	let host = Forgejo::new(forge, env.repo.clone());
	let host = match bot {
		Some(login) => host.with_account(login),
		None => host,
	};
	match host.identity() {
		Ok(_) => Ok(host),
		Err(
			e @ (ForgeError::Refused(_)
			| ForgeError::Status {
				status: 401 | 403, ..
			}),
		) => Err(CliError::usage(format!(
			"the bot login cannot be used: {e}"
		))),
		Err(e) => Err(CliError::failed(format!(
			"publishing stopped: reading the token's account: {e}"
		))),
	}
}

/// Which forge this is: `--forge`, else `[forge] kind`, else GitHub. A Forgejo runner exports
/// `FORGEJO_SERVER_URL`, and publishing as GitHub from one would send its token to the wrong API,
/// so that is refused rather than guessed at.
fn forge_kind(
	a: &PublishArgs,
	f: &ForgeSettings,
	get: &dyn Fn(&str) -> Option<String>,
) -> Result<HostKind, CliError> {
	let kind = a.forge.or(f.kind).unwrap_or(HostKind::GitHub);
	if kind == HostKind::GitHub && get(FORGEJO_SERVER_ENV).is_some() {
		return Err(CliError::usage(
			"this is a Forgejo runner (FORGEJO_SERVER_URL is set) and the forge kind is github: set `kind = \"forgejo\"` under `[forge]`, or pass --forge forgejo",
		));
	}
	Ok(kind)
}

/// The token: `--token-env`'s variable, which is `GITHUB_TOKEN` unless named, and on Forgejo
/// `FORGEJO_TOKEN` after it.
fn token(
	a: &PublishArgs,
	kind: HostKind,
	get: &dyn Fn(&str) -> Option<String>,
) -> Result<Token, CliError> {
	let value = get(&a.token_env).or_else(|| {
		(kind == HostKind::Forgejo && a.token_env == DEFAULT_TOKEN_ENV)
			.then(|| get(FORGEJO_TOKEN_ENV))
			.flatten()
	});
	value.and_then(|t| Token::new(&t)).ok_or_else(|| {
		CliError::usage(format!(
			"the token is not in {}: set it in the step's `env`",
			a.token_env
		))
	})
}

/// `host:port` of an `https://` URL, lower-cased, with the default port dropped.
fn authority(url: &str) -> Option<String> {
	let (rest, default_port) = if let Some(rest) = url.strip_prefix("https://") {
		(rest, ":443")
	} else {
		(url.strip_prefix("http://")?, ":80")
	};
	let authority = rest.split(['/', '?', '#']).next()?.to_ascii_lowercase();
	Some(
		authority
			.strip_suffix(default_port)
			.unwrap_or(&authority)
			.to_owned(),
	)
}

/// Settles where the token may go and how its certificate is trusted, before any request.
///
/// The API address is `[forge] api_url`, else `GITHUB_API_URL`. It must be `https`, and for a
/// Forgejo or whenever `api_url` is set it must be on the host of `GITHUB_SERVER_URL`: the setting
/// can confirm the runner's own API, and cannot send the token anywhere else.
fn forge_setup(
	kind: HostKind,
	f: &ForgeSettings,
	env: &Environment,
) -> Result<ForgeSetup, CliError> {
	if kind == HostKind::Forgejo && authority(&env.server_text).as_deref() == Some("github.com") {
		return Err(CliError::usage(
			"the forge kind is forgejo but GITHUB_SERVER_URL is github.com",
		));
	}
	let api_base = match (&f.api_url, &env.api_env) {
		(Some(url), _) => url.clone(),
		(None, Some(url)) => url.clone(),
		(None, None) => {
			return Err(CliError::usage(
				"GITHUB_API_URL is not set: this is not a workflow run",
			))
		}
	};
	if !api_base.starts_with("https://")
		&& !(kind == HostKind::Forgejo
			&& f.allow_http
			&& api_base.starts_with("http://")
			&& env.server_text.starts_with("http://"))
	{
		return Err(CliError::usage(if api_base.starts_with("http://") {
			"the API address is plain http, which is refused: the token would cross the network in clear"
		} else {
			"GITHUB_API_URL is not an https:// address"
		}));
	}
	if kind == HostKind::Forgejo || f.api_url.is_some() {
		let server = authority(&env.server_text);
		let api = authority(&api_base);
		if server.is_none()
			|| server != api
			|| (kind == HostKind::Forgejo
				&& env.server_text.starts_with("https://") != api_base.starts_with("https://"))
		{
			return Err(CliError::usage(
				"the API address is not on the runner's own host (GITHUB_SERVER_URL): the token is sent nowhere else",
			));
		}
	}
	if kind == HostKind::Forgejo && !f.blob_hosts.is_empty() {
		return Err(CliError::usage(
			"`forge.blob_hosts` is for GitHub Enterprise Server and is refused for forgejo",
		));
	}
	let ca = f.ca_file.as_deref().map(read_ca).transpose()?;
	Ok(ForgeSetup {
		allow_http: kind == HostKind::Forgejo && f.allow_http,
		kind,
		api_base,
		ca,
		blob_hosts: (!f.blob_hosts.is_empty()).then(|| f.blob_hosts.clone()),
	})
}

/// Reads a `ca_file`: a PEM of at most 1 MiB that holds certificates and nothing else.
fn read_ca(path: &Path) -> Result<CaBundle, CliError> {
	let shown = path.display();
	let file = fs::File::open(path)
		.map_err(|_| CliError::usage(format!("ca_file {shown}: cannot be opened")))?;
	let mut bytes = Vec::new();
	file.take(CaBundle::MAX_BYTES as u64 + 1)
		.read_to_end(&mut bytes)
		.map_err(|_| CliError::usage(format!("ca_file {shown}: cannot be read")))?;
	CaBundle::from_pem(&bytes).map_err(|e| CliError::usage(format!("ca_file {shown}: {e}")))
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

/// The preconditions. Every failure is exit status 2.
fn environment(
	get: &dyn Fn(&str) -> Option<String>,
	in_job: bool,
	allow_http: bool,
) -> Result<Environment, CliError> {
	let need = |name: &str| {
		get(name).ok_or_else(|| {
			CliError::usage(format!("{name} is not set: this is not a workflow run"))
		})
	};
	let event_name = need("GITHUB_EVENT_NAME")?;
	if (in_job && !matches!(event_name.as_str(), "pull_request" | "push"))
		|| (!in_job && event_name != "workflow_run")
	{
		return Err(CliError::usage(format!("publish requires workflow_run, or pull_request or push with --from; received {event_name}")));
	}
	let repo = RepoSlug::parse(&need("GITHUB_REPOSITORY")?)
		.ok_or_else(|| CliError::usage("GITHUB_REPOSITORY is not owner/name"))?;
	let git_ref = get("GITHUB_REF").unwrap_or_default();
	let api_env = get("GITHUB_API_URL");
	let server_text = need("GITHUB_SERVER_URL")?.trim_end_matches('/').to_owned();
	let server = ServerUrl::parse_with_http(&server_text, allow_http)
		.ok_or_else(|| CliError::usage("GITHUB_SERVER_URL is not an https:// address"))?;
	let path = need("GITHUB_EVENT_PATH")?;
	let bytes = read_event(Path::new(&path))?;
	let event = if in_job {
		let id = need("GITHUB_RUN_ID")?
			.parse::<u64>()
			.ok()
			.filter(|n| *n > 0)
			.ok_or_else(|| CliError::usage("GITHUB_RUN_ID is not a positive number"))?;
		let number = get("GITHUB_RUN_NUMBER")
			.map(|s| {
				s.parse::<u64>()
					.ok()
					.filter(|n| *n > 0)
					.ok_or_else(|| CliError::usage("GITHUB_RUN_NUMBER is not a positive number"))
			})
			.transpose()?;
		parse_in_job(&bytes, &event_name, id, number).map_err(CliError::usage)?
	} else {
		parse_event(&bytes).map_err(CliError::usage)?
	};

	if !in_job && git_ref != format!("refs/heads/{}", event.default_branch) {
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
		server_text,
		api_env,
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
fn load_settings(explicit: Option<&Path>, forgejo: bool) -> Result<Settings, CliError> {
	let default = Path::new(DEFAULT_CONFIG);
	let path = match explicit {
		Some(p) => Some(p),
		None if (forgejo
			|| config::read(Some(Path::new(".forgejo/pr-results.toml")))
				.is_ok_and(|s| s.forge.kind == Some(HostKind::Forgejo)))
			&& Path::new(".forgejo/pr-results.toml").is_file() =>
		{
			Some(Path::new(".forgejo/pr-results.toml"))
		}
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
	from: Option<&Path>,
	io: &mut Streams<'_>,
) -> Result<PublishReport, CliError> {
	let title = settings
		.title
		.clone()
		.unwrap_or_else(|| config::title_from_repo(env.repo.name()));
	let seed = plan(settings, &title, Vec::new());
	let local = from
		.map(|dir| crate::local::files(dir, &seed))
		.transpose()?;
	let sections = if settings.suites.is_empty() {
		if let Some((files, _)) = &local {
			inferred_names(files.iter().map(|(a, _, _)| a.as_str()), &title, io)?
		} else {
			inferred_sections(gh, run, &title, io)?
		}
	} else {
		configured_sections(settings, &title)?
	};
	let plan = plan(settings, &title, sections);
	let report = if let Some((files, skipped)) = local {
		if skipped > 0 {
			io.say(&format!("Skipped {skipped} links or non-regular inputs."));
		}
		let inputs = crate::local::read(&files, &plan);
		publish_inputs(gh, run, &env.server, &plan, &inputs)
	} else {
		publish(gh, run, &env.server, &plan)
			.map_err(|e| CliError::failed(format!("publishing stopped: {e}")))?
	};
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
			path_prefix: s.path_prefix.clone(),
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
	inferred_names(artifacts.iter().map(|a| a.name.as_str()), title, io)
}

fn inferred_names<'a>(
	names: impl Iterator<Item = &'a str>,
	title: &str,
	io: &mut Streams<'_>,
) -> Result<Vec<SectionPlan>, CliError> {
	let mut stems = BTreeSet::new();
	for name in names {
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
			path_prefix: None,
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
	// A commit status (Forgejo) is one request for each write: the file says its state, the
	// description and the link, under the context that names it.
	for r in requests {
		if r.method != Method::Post || !r.path.path().contains("/statuses/") {
			continue;
		}
		let Some(body) = r
			.body
			.as_deref()
			.and_then(|b| serde_json::from_slice::<Value>(b).ok())
		else {
			continue;
		};
		let text = |key: &str| {
			body.get(key)
				.and_then(Value::as_str)
				.unwrap_or_default()
				.to_owned()
		};
		let context = text("context");
		if seen.contains(&context) {
			continue;
		}
		seen.push(context.clone());
		write(
			&checks_dir.join(format!("{}.md", seen.len() - 1)),
			&check_file(
				&text("state"),
				&text("description"),
				&format!("context: {context}\nlink: {}\n", text("target_url")),
				"",
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

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

	use pr_results_forge::ApiPath;
	use serde_json::json;

	use super::*;

	#[test]
	fn a_dry_run_of_commit_statuses_writes_a_file_for_each_context() {
		let dir =
			std::env::temp_dir().join(format!("pr-results-dry-statuses-{}", std::process::id()));
		let _ = fs::remove_dir_all(&dir);
		let status = |context: &str, state: &str| {
			ApiRequest::json(
				Method::Post,
				ApiPath::parse(
					"/repos/example/app/statuses/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334",
				)
				.unwrap(),
				&json!({
					"context": context,
					"state": state,
					"description": "1 failed, 2 passed",
					"target_url": "https://git.lan/example/app/pulls/7#issuecomment-5",
				}),
			)
		};
		// The second status for a context supersedes the first and has no file of its own.
		let requests = [
			status("App Rust tests", "failure"),
			status("App E2E", "success"),
			status("App Rust tests", "success"),
		];
		let (mut out, mut err) = (Vec::new(), Vec::new());
		let mut io = Streams {
			out: &mut out,
			err: &mut err,
		};
		write_dry_run(&dir, &requests, &mut io).unwrap();

		let first = fs::read_to_string(dir.join("checks/0.md")).unwrap();
		assert!(
			first.starts_with("conclusion: failure\ntitle: 1 failed, 2 passed\n"),
			"{first}"
		);
		assert!(first.contains(
			"context: App Rust tests\nlink: https://git.lan/example/app/pulls/7#issuecomment-5"
		));
		assert!(fs::read_to_string(dir.join("checks/1.md"))
			.unwrap()
			.contains("context: App E2E"));
		assert!(!dir.join("checks/2.md").exists());
		assert!(String::from_utf8(out)
			.unwrap()
			.contains("Wrote requests.json and 2 check files"));
	}
}
