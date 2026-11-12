// The publish flow: read the run's artefacts, post the check runs, then the sticky comment.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The order is deliberate. Artefacts are read and rendered first, then the check runs are
//! written, because they hang on the commit and need no pull request, and the comment comes last
//! so a refusal there (a read-only token, a locked conversation) still leaves the checks posted.
//! Nothing here prints a name from a report: what it reports are counts and fixed words, and the
//! few names that reach a log go through [`Safe::log_line`].

use std::cell::Cell;
use std::fmt;

use pr_results_core::{
	parse_junit, CommentKey, Flavour, Glob, Limits, Marker, RepoPath, Report, RunId, Safe,
	ServerUrl, Sha, Status, Suite,
};
use pr_results_render::{
	check_with_paths, comment, job_summary, Comment, Conclusion, Input, Options, RunContext,
	Section, Target,
};
use serde_json::Value;

use crate::api::ForgeError;
use crate::archive::{read_zip, read_zip_filtered, ArchiveError, ArchiveLimits, Entry};
use crate::host::{Artifact, CheckSpec, CommentAction, CommentSpec, Host, HostKind, Pull, Tree};

/// A pull request is commented on this many times at most when several verify.
const MAX_PULLS: usize = 5;
/// The event file is read only if it is no larger than this.
const EVENT_FILE_BYTES: u64 = 1024 * 1024;
const EVENT_FILE_NAME: &str = "event.json";

/// The triggering run, read from the trusted `workflow_run` event.
#[derive(Debug, Clone)]
pub struct WorkflowRun {
	pub id: RunId,
	pub head_sha: Sha,
	pub head_branch: String,
	pub head_repo: pr_results_core::RepoSlug,
	/// `pull_request`, `push` and so on.
	pub event: String,
	/// `success`, `failure`, `cancelled`, `skipped` and so on.
	pub conclusion: Option<String>,
	/// The numbers in `workflow_run.pull_requests[]`, which are re-read and verified.
	pub pull_requests: Vec<u64>,
}

/// One configured suite: which artefacts and files feed it and what its check run is called.
#[derive(Debug, Clone)]
pub struct SectionPlan {
	pub title: String,
	pub check_name: String,
	pub required: bool,
	pub artifact: Glob,
	pub files: Vec<Glob>,
	pub flavour: Flavour,
}

#[derive(Debug, Clone)]
pub struct Plan {
	/// The comment's heading.
	pub title: String,
	pub key: CommentKey,
	/// Whether to write the sticky comment at all (`comment.mode`).
	pub post_comment: bool,
	pub sections: Vec<SectionPlan>,
	pub options: Options,
	pub archive: ArchiveLimits,
	pub parse: Limits,
	/// All artefacts downloaded for one run, in bytes.
	pub max_download_bytes: u64,
	/// The artefact that holds the pull request's `event.json`.
	pub event_artifact: String,
	/// Shown in the footer.
	pub tool_version: &'static str,
}

impl Plan {
	/// A plan with the settings' defaults for everything but the sections.
	pub fn new(
		title: &str,
		key: CommentKey,
		sections: Vec<SectionPlan>,
		tool_version: &'static str,
	) -> Self {
		Self {
			title: title.to_owned(),
			key,
			post_comment: true,
			sections,
			options: Options::default(),
			archive: ArchiveLimits::default(),
			parse: Limits::default(),
			max_download_bytes: 100 * 1024 * 1024,
			event_artifact: "event-file".to_owned(),
			tool_version,
		}
	}
}

/// Something worth saying about a run. Every one is a fixed phrase and numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
	/// A push gets check runs and no comment.
	PushNoComment,
	CommentsOff,
	/// No pull request verified, so there is no comment.
	NoPullRequest,
	/// Several pull requests verified and the event file did not choose one.
	SeveralPullRequests {
		found: usize,
		commented: usize,
	},
	PullClosed {
		pr: u64,
	},
	PullMoved {
		pr: u64,
	},
	PullGone {
		pr: u64,
	},
	CommentSkippedNewer {
		pr: u64,
	},
	/// The comment went over GitHub's limit and was written in its minimal form.
	CommentMinimal {
		pr: u64,
	},
	CheckMinimal {
		section: usize,
	},
	DuplicatesRemoved {
		pr: u64,
		count: usize,
	},
	ArtifactExpired {
		section: usize,
	},
	ArtifactRefused {
		section: usize,
		why: ArchiveError,
	},
	ArtifactTooLarge {
		section: usize,
	},
	InputUnreadable {
		section: usize,
	},
	TreeUnavailable,
	TreeTruncated,
	/// Waiting to retry used up its time, so later failures were reported, not retried.
	RetryBudgetSpent,
	AnnotationsOutsideTree {
		section: usize,
		count: usize,
	},
	AnnotationsRejected {
		section: usize,
		count: usize,
	},
}

/// Why the exit status is not zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
	Check { section: usize, status: Option<u16> },
	Comment { pr: u64, status: Option<u16> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
	pub section: usize,
	pub id: u64,
	pub created: bool,
	pub conclusion: Conclusion,
	pub annotations_sent: usize,
	pub annotation_batches: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentReport {
	pub pr: u64,
	pub id: u64,
	pub action: CommentAction,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublishReport {
	/// The run was cancelled or skipped, so nothing was read or written.
	pub nothing_to_do: bool,
	pub checks: Vec<CheckReport>,
	pub comments: Vec<CommentReport>,
	/// Section titles in plan order, for the log lines.
	pub titles: Vec<String>,
	pub notes: Vec<Note>,
	pub failures: Vec<Failure>,
	/// The comment without its marker, within the job summary's size cap: what the command line
	/// writes to the run's own page before its diagnostics. Nothing when the run was cancelled or
	/// skipped.
	pub job_summary: Option<String>,
}

impl PublishReport {
	/// Whether the exit status should be non-zero: something the plan asked for was not written.
	pub fn failed(&self) -> bool {
		!self.failures.is_empty()
	}

	/// The lines for the runner's log. They hold counts, fixed words and, for a section, its
	/// title through [`Safe::log_line`], so a workflow command cannot be written by a report, and
	/// the log needs no `::stop-commands::` wrapper.
	pub fn log_lines(&self) -> Vec<String> {
		let mut out = Vec::new();
		if self.nothing_to_do {
			out.push("The run was cancelled or skipped: nothing to publish.".to_owned());
			return out;
		}
		let title = |i: usize| {
			self.titles
				.get(i)
				.map(|t| Safe::log_line(t.as_str(), 80).as_str().to_owned())
				.unwrap_or_default()
		};
		for c in &self.checks {
			out.push(format!(
				"Check run {} for {}: {} annotations sent in {} batches.",
				if c.created { "created" } else { "updated" },
				title(c.section),
				c.annotations_sent,
				c.annotation_batches
			));
		}
		for c in &self.comments {
			let verb = match c.action {
				CommentAction::Created => "created",
				CommentAction::Updated => "updated",
				CommentAction::SkippedNewer => "left alone (a later run wrote it)",
			};
			out.push(format!("Comment on pull request {} {verb}.", c.pr));
		}
		for n in &self.notes {
			out.push(match n {
				Note::PushNoComment => "A push gets check runs and no comment.".to_owned(),
				Note::CommentsOff => "Comments are switched off.".to_owned(),
				Note::NoPullRequest => {
					"No open pull request matches this run, so there is no comment.".to_owned()
				}
				Note::SeveralPullRequests { found, commented } => format!(
					"{found} pull requests match this run and the event file chose none: commenting on {commented}."
				),
				Note::PullClosed { pr } => format!("Pull request {pr} is closed: no comment."),
				Note::PullMoved { pr } => {
					format!("Pull request {pr} has a newer commit: no comment.")
				}
				Note::PullGone { pr } => format!("Pull request {pr} could not be read: no comment."),
				Note::CommentSkippedNewer { pr } => {
					format!("Pull request {pr} already has a comment from a later run.")
				}
				Note::CommentMinimal { pr } => {
					format!("The comment for pull request {pr} was too large and was shortened.")
				}
				Note::CheckMinimal { section } => format!(
					"The check run for {} was too large and was shortened.",
					title(*section)
				),
				Note::DuplicatesRemoved { pr, count } => {
					format!("Removed {count} duplicate comments on pull request {pr}.")
				}
				Note::ArtifactExpired { section } => {
					format!("An artefact for {} has expired.", title(*section))
				}
				Note::ArtifactRefused { section, why } => {
					format!("An artefact for {} was refused: {why}.", title(*section))
				}
				Note::ArtifactTooLarge { section } => format!(
					"An artefact for {} went over the download limit.",
					title(*section)
				),
				Note::InputUnreadable { section } => {
					format!("A file for {} could not be read.", title(*section))
				}
				Note::RetryBudgetSpent => {
					"Retries stopped because the time allowed for waiting was used up.".to_owned()
				}
				Note::TreeUnavailable => {
					"The commit's tree could not be read: annotations were not checked.".to_owned()
				}
				Note::TreeTruncated => {
					"The commit's tree is too large to list: annotations were not checked."
						.to_owned()
				}
				Note::AnnotationsOutsideTree { section, count } => format!(
					"{count} annotations for {} named files that are not in the commit.",
					title(*section)
				),
				Note::AnnotationsRejected { section, count } => format!(
					"GitHub refused {count} annotations for {}.",
					title(*section)
				),
			});
		}
		for f in &self.failures {
			out.push(match f {
				Failure::Check { section, status } => format!(
					"The check run for {} was not written{}.",
					title(*section),
					status.map(|s| format!(" (status {s})")).unwrap_or_default()
				),
				Failure::Comment { pr, status } => format!(
					"The comment on pull request {pr} was not written{}.",
					status.map(|s| format!(" (status {s})")).unwrap_or_default()
				),
			});
		}
		out
	}
}

/// Publishing could not start: the run's artefacts could not be listed or downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishError {
	pub stage: &'static str,
	pub source: ForgeError,
}

impl fmt::Display for PublishError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}: {}", self.stage, self.source)
	}
}

impl std::error::Error for PublishError {}

fn at(stage: &'static str) -> impl Fn(ForgeError) -> PublishError {
	move |source| PublishError { stage, source }
}

/// What was read for each section.
struct Loaded {
	suites: Vec<Vec<Suite>>,
	skipped: Vec<usize>,
}

/// Posts the results of `run`: a check run per section, then the sticky comment on each pull
/// request that verifies. See the specification's "Finding and verifying the pull request".
pub fn publish<H: Host>(
	gh: &H,
	run: &WorkflowRun,
	server: &ServerUrl,
	plan: &Plan,
) -> Result<PublishReport, PublishError> {
	let mut report = PublishReport {
		titles: plan.sections.iter().map(|s| s.title.clone()).collect(),
		..PublishReport::default()
	};
	if matches!(run.conclusion.as_deref(), Some("cancelled" | "skipped")) {
		report.nothing_to_do = true;
		return Ok(report);
	}

	gh.begin_run();
	let artifacts = gh.artifacts(run.id).map_err(at("listing artefacts"))?;
	let loaded = load(gh, &artifacts, plan, &mut report)?;

	let ctx = RunContext {
		server: server.clone(),
		repo: gh.repo().clone(),
		head_sha: run.head_sha.clone(),
		run_id: run.id,
		run_number: None,
		key: plan.key.clone(),
		tool_version: plan.tool_version,
		target: match gh.kind() {
			HostKind::GitHub => Target::GitHub,
			HostKind::Forgejo => Target::Forgejo,
		},
	};
	let sections: Vec<Section<'_>> = plan
		.sections
		.iter()
		.zip(&loaded.suites)
		.zip(&loaded.skipped)
		.map(|((p, suites), skipped)| Section {
			title: &p.title,
			check_name: &p.check_name,
			required: p.required,
			suites,
			inputs_skipped: *skipped,
		})
		.collect();
	let input = Input {
		ctx: &ctx,
		title: &plan.title,
		sections: &sections,
		options: &plan.options,
	};

	let full: Comment = comment(&input);
	report.job_summary = Some(job_summary(&full).as_str().to_owned());

	let tree = read_tree(gh, run, &loaded, &mut report);
	post_checks(gh, run, plan, &input, tree.as_ref(), &mut report);
	post_comments(gh, run, plan, &input, &full, &artifacts, &mut report);
	if gh.retry_budget_spent() {
		report.notes.push(Note::RetryBudgetSpent);
	}
	Ok(report)
}

// ---- reading -------------------------------------------------------------------------------

/// Reads the run's artefacts one at a time. An artefact is downloaded once even when several
/// sections want it, only the entries some section asked for are inflated, and each archive is
/// parsed and dropped before the next is fetched, so memory holds one archive and what it yields.
/// The inflated bytes of everything kept are capped across the whole run by
/// `plan.archive.max_total_bytes`.
fn load<H: Host>(
	gh: &H,
	artifacts: &[Artifact],
	plan: &Plan,
	report: &mut PublishReport,
) -> Result<Loaded, PublishError> {
	let count = plan.sections.len();
	let mut merged: Vec<Report> = (0..count).map(|_| Report::default()).collect();
	let mut skipped = vec![0usize; count];
	let mut downloaded: u64 = 0;
	let mut inflated: u64 = 0;

	let mut wanted: Vec<&Artifact> = artifacts
		.iter()
		.filter(|a| plan.sections.iter().any(|s| s.artifact.is_match(&a.name)))
		.collect();
	wanted.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));

	for artifact in wanted {
		let feeds: Vec<usize> = plan
			.sections
			.iter()
			.enumerate()
			.filter(|(_, s)| s.artifact.is_match(&artifact.name))
			.map(|(i, _)| i)
			.collect();
		let refuse =
			|report: &mut PublishReport, skipped: &mut [usize], note: &dyn Fn(usize) -> Note| {
				for i in &feeds {
					report.notes.push(note(*i));
					if let Some(n) = skipped.get_mut(*i) {
						*n += 1;
					}
				}
			};
		if artifact.expired {
			refuse(report, &mut skipped, &|section| Note::ArtifactExpired {
				section,
			});
			continue;
		}
		let keep = |name: &str| {
			feeds
				.iter()
				.filter_map(|i| plan.sections.get(*i))
				.any(|s| s.files.iter().any(|g| g.is_match(name)))
		};
		let entries = match fetch(gh, artifact, plan, &mut downloaded, inflated, &keep)? {
			Fetched::Entries(e) => e,
			Fetched::Expired => {
				refuse(report, &mut skipped, &|section| Note::ArtifactExpired {
					section,
				});
				continue;
			}
			Fetched::TooLarge => {
				refuse(report, &mut skipped, &|section| Note::ArtifactTooLarge {
					section,
				});
				continue;
			}
			Fetched::Refused(why) => {
				refuse(report, &mut skipped, &|section| Note::ArtifactRefused {
					section,
					why,
				});
				continue;
			}
		};
		inflated =
			inflated.saturating_add(entries.iter().map(|e| e.data.len() as u64).sum::<u64>());
		for i in &feeds {
			let (Some(section), Some(out), Some(skip)) = (
				plan.sections.get(*i),
				merged.get_mut(*i),
				skipped.get_mut(*i),
			) else {
				continue;
			};
			for entry in entries
				.iter()
				.filter(|e| section.files.iter().any(|g| g.is_match(&e.name)))
			{
				match parse_junit(&entry.data, section.flavour, &plan.parse) {
					Ok(parsed) => out.suites.extend(parsed.suites),
					Err(_) => {
						report.notes.push(Note::InputUnreadable { section: *i });
						*skip += 1;
					}
				}
			}
		}
		// `entries` is dropped here, before the next archive is downloaded.
	}

	let mut loaded = Loaded {
		suites: Vec::new(),
		skipped,
	};
	for mut r in merged {
		r.merge_same_named();
		loaded.suites.push(r.suites);
	}
	Ok(loaded)
}

enum Fetched {
	Entries(Vec<Entry>),
	Expired,
	TooLarge,
	Refused(ArchiveError),
}

/// One artefact's wanted entries, or why it cannot be used. `inflated` is what the run has
/// already kept, and the archive may add only what is left of the cap.
fn fetch<H: Host>(
	gh: &H,
	artifact: &Artifact,
	plan: &Plan,
	downloaded: &mut u64,
	inflated: u64,
	keep: &dyn Fn(&str) -> bool,
) -> Result<Fetched, PublishError> {
	let room = plan.max_download_bytes.saturating_sub(*downloaded);
	if artifact.size_in_bytes > room {
		return Ok(Fetched::TooLarge);
	}
	let bytes = match gh.download(artifact, room) {
		Ok(b) => b,
		Err(ForgeError::TooLarge { .. }) => return Ok(Fetched::TooLarge),
		Err(ForgeError::Status {
			status: 404 | 410, ..
		}) => return Ok(Fetched::Expired),
		Err(e) => return Err(at("downloading an artefact")(e)),
	};
	*downloaded = downloaded.saturating_add(bytes.len() as u64);
	let limits = ArchiveLimits {
		max_total_bytes: plan.archive.max_total_bytes.saturating_sub(inflated),
		..plan.archive
	};
	match read_zip_filtered(&bytes, &limits, keep) {
		Ok(entries) => Ok(Fetched::Entries(entries)),
		Err(ArchiveError::TotalTooLarge) => Ok(Fetched::TooLarge),
		Err(why) => Ok(Fetched::Refused(why)),
	}
}

/// The commit's tree, read once and only when some case could be annotated.
fn read_tree<H: Host>(
	gh: &H,
	run: &WorkflowRun,
	loaded: &Loaded,
	report: &mut PublishReport,
) -> Option<Tree> {
	let wanted = loaded.suites.iter().flatten().any(|s| {
		s.cases.iter().any(|c| {
			c.file.is_some() && matches!(c.status, Status::Failed | Status::Errored | Status::Flaky)
		})
	});
	if !wanted {
		return None;
	}
	match gh.tree_paths(&run.head_sha) {
		Ok(t) if t.truncated => {
			report.notes.push(Note::TreeTruncated);
			None
		}
		Ok(t) => Some(t),
		Err(_) => {
			report.notes.push(Note::TreeUnavailable);
			None
		}
	}
}

// ---- check runs ----------------------------------------------------------------------------

fn post_checks<H: Host>(
	gh: &H,
	run: &WorkflowRun,
	plan: &Plan,
	input: &Input<'_>,
	tree: Option<&Tree>,
	report: &mut PublishReport,
) {
	for (index, section) in plan.sections.iter().enumerate() {
		let outside = Cell::new(0usize);
		let keep = |p: &RepoPath| {
			let known = tree.is_none_or(|t| t.paths.contains(p.as_str()));
			if !known {
				outside.set(outside.get() + 1);
			}
			known
		};
		let Some(output) = check_with_paths(input, index, &keep) else {
			continue;
		};
		if outside.get() > 0 {
			report.notes.push(Note::AnnotationsOutsideTree {
				section: index,
				count: outside.get(),
			});
		}
		let external_id = format!("pr-results:1:{}:{}", plan.key.as_str(), section.check_name);
		let mut spec = CheckSpec {
			name: &section.check_name,
			head_sha: &run.head_sha,
			external_id: &external_id,
			output: &output,
		};
		let mut result = gh.upsert_result(&spec);
		let shortened;
		if matches!(result, Err(ForgeError::Status { status: 422, .. })) {
			// Over the size limit, or something GitHub will not take: say less.
			let small = Options {
				summary_limit: 0,
				text_limit: 0,
				max_annotations: 0,
				..plan.options.clone()
			};
			let again = Input {
				options: &small,
				..input.clone()
			};
			if let Some(minimal) = check_with_paths(&again, index, &keep) {
				shortened = minimal;
				spec.output = &shortened;
				report.notes.push(Note::CheckMinimal { section: index });
				result = gh.upsert_result(&spec);
			}
		}
		match result {
			Ok(r) => {
				if r.annotations_rejected > 0 {
					report.notes.push(Note::AnnotationsRejected {
						section: index,
						count: r.annotations_rejected,
					});
				}
				report.checks.push(CheckReport {
					section: index,
					id: r.id,
					created: r.created,
					conclusion: spec.output.conclusion,
					annotations_sent: r.annotations_sent,
					annotation_batches: r.annotation_batches,
				});
			}
			Err(e) => report.failures.push(Failure::Check {
				section: index,
				status: e.status(),
			}),
		}
	}
}

// ---- the comment ---------------------------------------------------------------------------

fn verifies<H: Host>(gh: &H, run: &WorkflowRun, p: &Pull) -> bool {
	p.open
		&& p.head_sha == run.head_sha.as_str()
		&& same_repo(p.head_repo.as_deref(), &run.head_repo.to_string())
		&& same_repo(p.base_repo.as_deref(), &gh.repo().to_string())
}

/// Repository names are case-insensitive: `Owner/Repo` and `owner/repo` are one repository.
fn same_repo(found: Option<&str>, want: &str) -> bool {
	found.is_some_and(|f| f.eq_ignore_ascii_case(want))
}

/// The pull requests this run belongs to, from the trusted event and the API, never from the
/// uploaded event file alone.
fn find_pulls<H: Host>(
	gh: &H,
	run: &WorkflowRun,
	hint: impl FnOnce() -> Option<u64>,
) -> Result<(Vec<Pull>, Option<Note>), ForgeError> {
	let mut found: Vec<Pull> = Vec::new();
	fn push(p: Pull, found: &mut Vec<Pull>) {
		if !found.iter().any(|f| f.number == p.number) {
			found.push(p);
		}
	}
	if run.pull_requests.is_empty() {
		for p in gh.pulls_for_commit(&run.head_sha)? {
			if verifies(gh, run, &p) {
				push(p, &mut found);
			}
		}
		if found.is_empty() {
			for p in gh.pulls_by_head(run.head_repo.owner(), &run.head_branch)? {
				if verifies(gh, run, &p) {
					push(p, &mut found);
				}
			}
		}
	} else {
		for n in &run.pull_requests {
			if let Some(p) = gh.pull(*n)? {
				if verifies(gh, run, &p) {
					push(p, &mut found);
				}
			}
		}
	}
	found.sort_by_key(|p| p.number);
	if found.len() <= 1 {
		return Ok((found, None));
	}
	let total = found.len();
	if let Some(n) = hint() {
		if let Some(p) = found.iter().find(|p| p.number == n) {
			return Ok((vec![p.clone()], None));
		}
	}
	found.truncate(MAX_PULLS);
	let note = Note::SeveralPullRequests {
		found: total,
		commented: found.len(),
	};
	Ok((found, Some(note)))
}

/// The pull request number in the uploaded event file, if there is one. It is a hint to choose
/// among pull requests that already verified, and is believed for nothing else.
fn event_hint<H: Host>(gh: &H, artifacts: &[Artifact], name: &str) -> Option<u64> {
	let artifact = artifacts
		.iter()
		.find(|a| a.name == name && !a.expired && a.size_in_bytes <= 2 * EVENT_FILE_BYTES)?;
	let bytes = gh.download(artifact, 2 * EVENT_FILE_BYTES).ok()?;
	let limits = ArchiveLimits {
		max_files: 8,
		max_entry_bytes: EVENT_FILE_BYTES,
		max_total_bytes: EVENT_FILE_BYTES,
	};
	let entries = read_zip(&bytes, &limits).ok()?;
	let file = entries.iter().find(|e| e.name == EVENT_FILE_NAME)?;
	let doc: Value = serde_json::from_slice(&file.data).ok()?;
	doc.get("pull_request")?.get("number")?.as_u64()
}

fn post_comments<H: Host>(
	gh: &H,
	run: &WorkflowRun,
	plan: &Plan,
	input: &Input<'_>,
	full: &Comment,
	artifacts: &[Artifact],
	report: &mut PublishReport,
) {
	if !plan.post_comment {
		report.notes.push(Note::CommentsOff);
		return;
	}
	if run.event == "push" {
		report.notes.push(Note::PushNoComment);
		return;
	}
	let (pulls, note) =
		match find_pulls(gh, run, || event_hint(gh, artifacts, &plan.event_artifact)) {
			Ok(found) => found,
			Err(_) => {
				report.failures.push(Failure::Comment {
					pr: 0,
					status: None,
				});
				return;
			}
		};
	if pulls.is_empty() {
		report.notes.push(Note::NoPullRequest);
		return;
	}
	report.notes.extend(note);

	let Some(marker) = Marker::parse(full.body.as_str()) else {
		// The renderer always writes a marker first; without one there is nothing to find again.
		report.failures.push(Failure::Comment {
			pr: 0,
			status: None,
		});
		return;
	};
	let minimal: std::cell::OnceCell<Comment> = std::cell::OnceCell::new();
	for pull in pulls {
		let pr = pull.number;
		match gh.pull(pr) {
			Ok(Some(now)) if verifies(gh, run, &now) => {}
			Ok(Some(now)) if !now.open => {
				report.notes.push(Note::PullClosed { pr });
				continue;
			}
			Ok(Some(_)) => {
				report.notes.push(Note::PullMoved { pr });
				continue;
			}
			Ok(None) | Err(_) => {
				report.notes.push(Note::PullGone { pr });
				continue;
			}
		}
		let mut body = full.body.as_str();
		let mut result = gh.upsert_comment(&CommentSpec {
			pr,
			marker: &marker,
			body,
		});
		if matches!(result, Err(ForgeError::Status { status: 422, .. })) {
			let small = minimal.get_or_init(|| {
				let options = Options {
					comment_limit: 0,
					..plan.options.clone()
				};
				comment(&Input {
					options: &options,
					..input.clone()
				})
			});
			body = small.body.as_str();
			report.notes.push(Note::CommentMinimal { pr });
			result = gh.upsert_comment(&CommentSpec {
				pr,
				marker: &marker,
				body,
			});
		}
		match result {
			Ok(r) => {
				if r.action == CommentAction::SkippedNewer {
					report.notes.push(Note::CommentSkippedNewer { pr });
				}
				if r.duplicates_removed > 0 {
					report.notes.push(Note::DuplicatesRemoved {
						pr,
						count: r.duplicates_removed,
					});
				}
				report.comments.push(CommentReport {
					pr,
					id: r.id,
					action: r.action,
				});
			}
			Err(e) => report.failures.push(Failure::Comment {
				pr,
				status: e.status(),
			}),
		}
	}
}
