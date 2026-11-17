// The readers for cargo's own JSON: `cargo clippy --message-format=json` and `cargo audit --json`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Rust projects can feed clippy and the audit to the comment with no extra tool and no converter.
//! Both are read as findings, the way SARIF is: a finding at or above the suite's threshold fails and
//! one below it is skipped, a clean run is one passing case, and a location is used only as a
//! relative path inside the repository. As there, only the fields that are used are kept.

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::ansi::strip_ansi;
use crate::error::ParseError;
use crate::junit::Limits;
use crate::model::{Case, Flavour, Report, Severity, Status, Suite};
use crate::sarif::{blank, capped, clean_scan, plain_relative, CASE_OVERHEAD};
use crate::trusted::RepoPath;

/// The path for a file a tool names, with the suite's prefix in front, or nothing when it is not a
/// path inside the repository.
fn locate(raw: &str, prefix: Option<&RepoPath>) -> Option<RepoPath> {
	let relative = plain_relative(raw)?;
	match prefix {
		Some(p) => p.join(&relative),
		None => RepoPath::parse(&relative),
	}
}

/// Checks a document is within the limits that apply before it is read.
fn start<'a>(input: &'a [u8], limits: &Limits) -> Result<&'a [u8], ParseError> {
	if input.len() > limits.max_bytes {
		return Err(ParseError::TooLarge {
			len: input.len(),
			limit: limits.max_bytes,
		});
	}
	let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
	if input.iter().all(u8::is_ascii_whitespace) {
		return Err(ParseError::Empty);
	}
	Ok(input)
}

fn status_for(severity: Severity, fail_on: Severity) -> Status {
	if severity >= fail_on {
		Status::Failed
	} else {
		Status::Skipped
	}
}

// ---- clippy -------------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct Line {
	reason: Option<String>,
	message: Option<Diagnostic>,
}

#[derive(Deserialize, Default)]
struct Diagnostic {
	level: Option<String>,
	message: Option<String>,
	code: Option<Code>,
	#[serde(default)]
	spans: Vec<Span>,
	rendered: Option<String>,
}

#[derive(Deserialize, Default)]
struct Code {
	code: Option<String>,
}

#[derive(Deserialize, Default)]
struct Span {
	file_name: Option<String>,
	line_start: Option<u64>,
	#[serde(default)]
	is_primary: bool,
}

/// Parses `cargo clippy --message-format=json` (or `cargo build`'s): one JSON object per line, and
/// a `build-finished` line at the end, which is how a file that was cut short is told from a whole
/// one. A diagnostic with no span (`1 warning emitted`) is a summary, not a finding. The same
/// diagnostic from building a crate for more than one target is counted once.
pub fn parse_clippy_json(
	input: &[u8],
	limits: &Limits,
	path_prefix: Option<&RepoPath>,
	fail_on: Severity,
) -> Result<Report, ParseError> {
	let input = start(input, limits)?;
	let mut finished = false;
	let mut seen: BTreeSet<(String, String, u64, String)> = BTreeSet::new();
	let mut cases: Vec<Case> = Vec::new();
	let mut text = 0usize;

	for (n, raw) in input.split(|b| *b == b'\n').enumerate() {
		let raw = raw.trim_ascii();
		if raw.is_empty() {
			continue;
		}
		let line: Line = serde_json::from_slice(raw).map_err(|e| {
			if e.classify() == serde_json::error::Category::Eof {
				ParseError::Truncated
			} else {
				ParseError::NotJson {
					line: n + 1,
					column: e.column(),
				}
			}
		})?;
		match line.reason.as_deref() {
			Some("build-finished") => finished = true,
			Some("compiler-message") => {
				let Some(d) = line.message else { continue };
				if let Some(case) =
					diagnostic(d, limits, path_prefix, fail_on, &mut seen, &mut text)
				{
					cases.push(case);
					if cases.len() > limits.max_cases {
						return Err(ParseError::TooManyCases {
							limit: limits.max_cases,
						});
					}
					if text > limits.max_text_bytes {
						return Err(ParseError::TooMuchText {
							limit: limits.max_text_bytes,
						});
					}
				}
			}
			_ => {}
		}
	}
	if !finished {
		return Err(ParseError::Truncated);
	}
	if cases.is_empty() {
		let mut count = 0;
		cases.push(clean_scan(&mut count, &mut text, limits)?);
	}
	Ok(Report {
		suites: vec![Suite {
			name: capped("clippy", limits.max_name_bytes, &mut text),
			project: None,
			flavour: Flavour::Generic,
			cases,
			time_ms: 0,
		}],
	})
}

fn diagnostic(
	d: Diagnostic,
	limits: &Limits,
	path_prefix: Option<&RepoPath>,
	fail_on: Severity,
	seen: &mut BTreeSet<(String, String, u64, String)>,
	text: &mut usize,
) -> Option<Case> {
	let level = d.level.as_deref()?;
	let severity = if level.starts_with("error") {
		Severity::Error
	} else if level == "warning" {
		Severity::Warning
	} else {
		return None;
	};
	let span = d
		.spans
		.iter()
		.find(|s| s.is_primary)
		.or_else(|| d.spans.first())?;
	let message = d.message.unwrap_or_default();
	let first = message.lines().next().unwrap_or("").trim().to_owned();
	let name = d
		.code
		.and_then(|c| c.code)
		.filter(|c| !c.trim().is_empty())
		.unwrap_or_else(|| "rustc".to_owned());
	let file = span
		.file_name
		.as_deref()
		.and_then(|f| locate(f, path_prefix));
	let line = span.line_start.filter(|n| *n > 0);

	let key = (
		name.clone(),
		file.as_ref()
			.map(|f| f.as_str().to_owned())
			.unwrap_or_default(),
		line.unwrap_or(0),
		first.clone(),
	);
	if !seen.insert(key) {
		return None;
	}

	let mut case = blank(
		capped(&name, limits.max_name_bytes, text),
		status_for(severity, fail_on),
	);
	if !first.is_empty() {
		case.message = Some(capped(&first, limits.max_message_bytes, text));
	}
	if let Some(rendered) = d.rendered {
		let rendered = strip_ansi(&rendered);
		let rendered = rendered.trim();
		if rendered.lines().nth(1).is_some() {
			case.detail = Some(capped(rendered, limits.max_detail_bytes, text));
		}
	}
	if let Some(file) = file {
		case.classname = capped(file.as_str(), limits.max_name_bytes, text);
		case.file = Some(capped(file.as_str(), limits.max_name_bytes, text));
		case.line = line.and_then(|n| u32::try_from(n).ok());
	}
	*text = text.saturating_add(CASE_OVERHEAD);
	Some(case)
}

// ---- cargo audit --------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct Audit {
	vulnerabilities: Option<Vulnerabilities>,
	#[serde(default)]
	warnings: std::collections::BTreeMap<String, Vec<Finding>>,
}

#[derive(Deserialize, Default)]
struct Vulnerabilities {
	#[serde(default)]
	list: Vec<Finding>,
}

#[derive(Deserialize, Default)]
struct Finding {
	advisory: Option<Advisory>,
	package: Option<Package>,
	versions: Option<Versions>,
}

#[derive(Deserialize, Default)]
struct Advisory {
	id: Option<String>,
	title: Option<String>,
	description: Option<String>,
	url: Option<String>,
}

#[derive(Deserialize, Default)]
struct Package {
	name: Option<String>,
	version: Option<String>,
}

#[derive(Deserialize, Default)]
struct Versions {
	#[serde(default)]
	patched: Vec<String>,
}

/// Parses `cargo audit --json`. A vulnerability is an error; a warning (unmaintained, unsound,
/// yanked) is a warning, so with the default threshold both fail, and `fail_on = "error"` is what
/// `cargo audit` itself does. A finding is reported at `Cargo.lock`, which is where it is fixed.
pub fn parse_audit_json(
	input: &[u8],
	limits: &Limits,
	path_prefix: Option<&RepoPath>,
	fail_on: Severity,
) -> Result<Report, ParseError> {
	let input = start(input, limits)?;
	let audit: Audit = serde_json::from_slice(input).map_err(|e| {
		if e.classify() == serde_json::error::Category::Eof {
			ParseError::Truncated
		} else {
			ParseError::NotJson {
				line: e.line(),
				column: e.column(),
			}
		}
	})?;
	let Some(vulnerabilities) = audit.vulnerabilities else {
		return Err(ParseError::Structure(
			"not the output of cargo audit --json",
		));
	};
	let lock = locate("Cargo.lock", path_prefix);

	let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
	let mut cases: Vec<Case> = Vec::new();
	let mut text = 0usize;
	let all = vulnerabilities
		.list
		.into_iter()
		.map(|f| (Severity::Error, "vulnerability".to_owned(), f))
		.chain(audit.warnings.into_iter().flat_map(|(kind, list)| {
			list.into_iter()
				.map(move |f| (Severity::Warning, kind.clone(), f))
		}));
	for (severity, kind, f) in all {
		let advisory = f.advisory.unwrap_or_default();
		let package = f.package.unwrap_or_default();
		let id = advisory
			.id
			.filter(|i| !i.trim().is_empty())
			.unwrap_or_else(|| kind.clone());
		let crate_name = package.name.unwrap_or_default();
		let version = package.version.unwrap_or_default();
		if !seen.insert((id.clone(), crate_name.clone(), version.clone())) {
			continue;
		}
		let mut case = blank(
			capped(&id, limits.max_name_bytes, &mut text),
			status_for(severity, fail_on),
		);
		let group = format!("{crate_name} {version}");
		case.classname = capped(group.trim(), limits.max_name_bytes, &mut text);
		if let Some(title) = advisory
			.title
			.as_deref()
			.map(str::trim)
			.filter(|t| !t.is_empty())
		{
			case.message = Some(capped(title, limits.max_message_bytes, &mut text));
		}
		let mut detail = String::new();
		if let Some(d) = advisory
			.description
			.as_deref()
			.map(str::trim)
			.filter(|d| !d.is_empty())
		{
			detail.push_str(d);
		}
		if let Some(patched) = f.versions.map(|v| v.patched).filter(|p| !p.is_empty()) {
			if !detail.is_empty() {
				detail.push('\n');
			}
			detail.push_str(&format!("Patched versions: {}", patched.join(", ")));
		}
		if let Some(url) = advisory.url.as_deref().filter(|u| !u.trim().is_empty()) {
			if !detail.is_empty() {
				detail.push('\n');
			}
			detail.push_str(url);
		}
		if detail.lines().nth(1).is_some() {
			case.detail = Some(capped(&detail, limits.max_detail_bytes, &mut text));
		}
		if let Some(lock) = &lock {
			case.file = Some(capped(lock.as_str(), limits.max_name_bytes, &mut text));
		}
		text = text.saturating_add(CASE_OVERHEAD);
		cases.push(case);
		if cases.len() > limits.max_cases {
			return Err(ParseError::TooManyCases {
				limit: limits.max_cases,
			});
		}
		if text > limits.max_text_bytes {
			return Err(ParseError::TooMuchText {
				limit: limits.max_text_bytes,
			});
		}
	}
	if cases.is_empty() {
		let mut count = 0;
		cases.push(clean_scan(&mut count, &mut text, limits)?);
	}
	Ok(Report {
		suites: vec![Suite {
			name: capped("cargo audit", limits.max_name_bytes, &mut text),
			project: None,
			flavour: Flavour::Generic,
			cases,
			time_ms: 0,
		}],
	})
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

	use super::*;

	#[test]
	fn a_file_is_a_repository_path_or_nothing() {
		assert!(locate("src/lib.rs", None).is_some());
		assert_eq!(locate("./src/lib.rs", None).unwrap().as_str(), "src/lib.rs");
		assert_eq!(locate("100%/a.rs", None).unwrap().as_str(), "100%/a.rs");
		for not in [
			"/home/runner/.cargo/registry/src/x/lib.rs",
			"../x.rs",
			"C:/x.rs",
			"",
		] {
			assert!(locate(not, None).is_none(), "{not}");
		}
	}
}
