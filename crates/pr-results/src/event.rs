// The trusted `workflow_run` event, read from the file the runner wrote.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! This is the event of the publisher's own run, not the pull request's uploaded `event.json`,
//! which is untrusted and read by the publish flow as a hint only. Errors are fixed phrases.

use pr_results_core::{RepoSlug, RunId, Sha};
use pr_results_forge::WorkflowRun;
use serde_json::Value;

/// A `workflow_run` event is read to at most this many bytes.
pub const MAX_EVENT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Event {
	/// The repository the event is for, which must be the one the job runs in.
	pub repository: RepoSlug,
	/// The repository that ran the triggering workflow.
	pub run_repository: RepoSlug,
	pub default_branch: String,
	/// `None` when the fork the run came from has been deleted, so nothing can be verified.
	pub run: Option<WorkflowRun>,
}

fn text<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
	let mut at = v;
	for key in path {
		at = at.get(key)?;
	}
	at.as_str()
}

fn slug(v: &Value, path: &[&str]) -> Option<RepoSlug> {
	RepoSlug::parse(text(v, path)?)
}

pub fn parse_event(bytes: &[u8]) -> Result<Event, &'static str> {
	let doc: Value =
		serde_json::from_slice(bytes).map_err(|_| "the event file is not valid JSON")?;
	let run = doc
		.get("workflow_run")
		.filter(|r| r.is_object())
		.ok_or("the event file is not a workflow_run event")?;
	let repository = slug(&doc, &["repository", "full_name"])
		.ok_or("the event has no repository name it can use")?;
	let run_repository = slug(run, &["repository", "full_name"])
		.ok_or("the event's run has no repository name it can use")?;
	let default_branch = text(&doc, &["repository", "default_branch"])
		.filter(|b| !b.is_empty())
		.ok_or("the event does not name the default branch")?
		.to_owned();
	let id = run
		.get("id")
		.and_then(Value::as_u64)
		.ok_or("the event's run has no id")?;
	let head_sha = text(run, &["head_sha"])
		.and_then(Sha::parse)
		.ok_or("the event's run has no commit it can use")?;
	let run_event = text(run, &["event"])
		.ok_or("the event's run does not say what started it")?
		.to_owned();
	let pull_requests: Vec<u64> = run
		.get("pull_requests")
		.and_then(Value::as_array)
		.map(|list| {
			list.iter()
				.filter_map(|p| p.get("number").and_then(Value::as_u64))
				.collect()
		})
		.unwrap_or_default();
	let workflow_run = slug(run, &["head_repository", "full_name"]).map(|head_repo| WorkflowRun {
		id: RunId(id),
		run_number: run.get("run_number").and_then(Value::as_u64),
		head_sha,
		head_branch: text(run, &["head_branch"]).unwrap_or_default().to_owned(),
		head_repo,
		event: run_event,
		conclusion: text(run, &["conclusion"]).map(str::to_owned),
		pull_requests,
	});
	Ok(Event {
		repository,
		run_repository,
		default_branch,
		run: workflow_run,
	})
}

/// Reads the runner's own pull request or push event, never an uploaded event.
pub fn parse_in_job(
	bytes: &[u8],
	kind: &str,
	id: u64,
	number: Option<u64>,
) -> Result<Event, &'static str> {
	let doc: Value =
		serde_json::from_slice(bytes).map_err(|_| "the event file is not valid JSON")?;
	let repository = slug(&doc, &["repository", "full_name"])
		.ok_or("the event has no repository name it can use")?;
	let (head_repo, head_sha, head_branch, pulls) = match kind {
		"pull_request" => {
			let pr = doc
				.get("pull_request")
				.ok_or("the event has no pull request")?;
			let base = slug(pr, &["base", "repo", "full_name"])
				.ok_or("the pull request has no base repository")?;
			if !base
				.to_string()
				.eq_ignore_ascii_case(&repository.to_string())
			{
				return Err("the pull request has another base repository");
			}
			let head = slug(pr, &["head", "repo", "full_name"])
				.ok_or("the pull request has no head repository")?;
			let sha = text(pr, &["head", "sha"])
				.and_then(Sha::parse)
				.ok_or("the pull request has no commit it can use")?;
			let n = pr
				.get("number")
				.or_else(|| doc.get("number"))
				.and_then(Value::as_u64)
				.filter(|n| *n > 0)
				.ok_or("the pull request has no number")?;
			(
				head,
				sha,
				text(pr, &["head", "ref"]).unwrap_or_default().to_owned(),
				vec![n],
			)
		}
		"push" => {
			let sha = text(&doc, &["after"])
				.and_then(Sha::parse)
				.ok_or("the push has no commit it can use")?;
			(
				repository.clone(),
				sha,
				text(&doc, &["ref"])
					.unwrap_or_default()
					.strip_prefix("refs/heads/")
					.unwrap_or_default()
					.to_owned(),
				Vec::new(),
			)
		}
		_ => return Err("in-job publishing requires a pull_request or push event"),
	};
	Ok(Event {
		run_repository: repository.clone(),
		repository,
		default_branch: String::new(),
		run: Some(WorkflowRun {
			id: RunId(id),
			run_number: number,
			head_sha,
			head_branch,
			head_repo,
			event: kind.to_owned(),
			conclusion: None,
			pull_requests: pulls,
		}),
	})
}
