// The SARIF reader: static-analysis results as the model the renderer already knows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SARIF 2.1.0 is what many scanners write (CodeQL, Semgrep, Trivy, clippy through a converter).
//! A run is a [`Suite`]; a result is a [`Case`] whose name is its rule, whose class name is the
//! file it is in (so a comment shows `suite › file › rule`) and whose file and line carry the
//! annotation. A finding at or above the suite's threshold is a failure; one below it is skipped
//! and does not fail the job. A run with no findings is one passing case, so a clean scan counts.
//!
//! The input is read into types that hold only what is used: every other field is skipped as it is
//! read, so memory follows what is kept and not the size of the document. Everything that is kept
//! is untrusted text, capped by the same limits as a JUnit report.

use serde::Deserialize;

use crate::error::ParseError;
use crate::junit::Limits;
use crate::model::{Case, Flavour, Report, Severity, Status, Suite, Untrusted};
use crate::trusted::RepoPath;

/// What a stored piece of text costs besides its bytes, as for a JUnit report.
const TEXT_OVERHEAD: usize = 40;
pub(crate) const CASE_OVERHEAD: usize = 64;

#[derive(Deserialize, Default)]
struct Doc {
	version: Option<String>,
	#[serde(default)]
	runs: Vec<Run>,
}

#[derive(Deserialize, Default)]
struct Run {
	#[serde(default)]
	tool: Tool,
	#[serde(default)]
	results: Vec<Finding>,
}

#[derive(Deserialize, Default)]
struct Tool {
	#[serde(default)]
	driver: Driver,
}

#[derive(Deserialize, Default)]
struct Driver {
	name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Finding {
	rule_id: Option<String>,
	level: Option<String>,
	kind: Option<String>,
	message: Option<Message>,
	#[serde(default)]
	locations: Vec<Location>,
	#[serde(default)]
	suppressions: Vec<serde::de::IgnoredAny>,
}

#[derive(Deserialize, Default)]
struct Message {
	text: Option<String>,
	markdown: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Location {
	physical_location: Option<Physical>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Physical {
	artifact_location: Option<Artifact>,
	region: Option<Region>,
}

#[derive(Deserialize, Default)]
struct Artifact {
	uri: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Region {
	start_line: Option<u64>,
}

/// Parses one SARIF file whose paths are relative to `path_prefix`, as for a JUnit report.
/// `fail_on` is the lowest severity that fails: a finding below it is skipped.
pub fn parse_sarif(
	input: &[u8],
	limits: &Limits,
	path_prefix: Option<&RepoPath>,
	fail_on: Severity,
) -> Result<Report, ParseError> {
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
	let doc: Doc = serde_json::from_slice(input).map_err(json_error)?;
	if !doc.version.as_deref().is_some_and(|v| v.starts_with("2.")) {
		return Err(ParseError::Structure("not a SARIF 2 document"));
	}
	if doc.runs.is_empty() {
		return Err(ParseError::Structure("the document has no runs"));
	}
	if doc.runs.len() > limits.max_suites {
		return Err(ParseError::TooManySuites {
			limit: limits.max_suites,
		});
	}
	let mut cases = 0usize;
	let mut text = 0usize;
	let mut suites = Vec::with_capacity(doc.runs.len());
	for run in doc.runs {
		let name = run
			.tool
			.driver
			.name
			.filter(|n| !n.trim().is_empty())
			.unwrap_or_else(|| "SARIF".to_owned());
		let mut suite = Suite {
			name: capped(&name, limits.max_name_bytes, &mut text),
			project: None,
			flavour: Flavour::Generic,
			cases: Vec::new(),
			time_ms: 0,
		};
		if run.results.is_empty() {
			suite.cases.push(clean_scan(&mut cases, &mut text, limits)?);
		}
		for finding in run.results {
			let case = case_of(finding, limits, path_prefix, fail_on, &mut text);
			cases += 1;
			if cases > limits.max_cases {
				return Err(ParseError::TooManyCases {
					limit: limits.max_cases,
				});
			}
			if text > limits.max_text_bytes {
				return Err(ParseError::TooMuchText {
					limit: limits.max_text_bytes,
				});
			}
			suite.cases.push(case);
		}
		suites.push(suite);
	}
	Ok(Report { suites })
}

fn json_error(e: serde_json::Error) -> ParseError {
	use serde_json::error::Category;
	if e.classify() == Category::Eof {
		return ParseError::Truncated;
	}
	ParseError::NotJson {
		line: e.line(),
		column: e.column(),
	}
}

/// Keeps at most `max` bytes of `text` and counts what is stored.
pub(crate) fn capped(text: &str, max: usize, spent: &mut usize) -> Untrusted {
	let kept = Untrusted::capped(text, max);
	*spent = spent
		.saturating_add(kept.len())
		.saturating_add(TEXT_OVERHEAD);
	kept
}

/// What a run with no findings is: one case that passed.
pub(crate) fn clean_scan(
	cases: &mut usize,
	text: &mut usize,
	limits: &Limits,
) -> Result<Case, ParseError> {
	*cases += 1;
	if *cases > limits.max_cases {
		return Err(ParseError::TooManyCases {
			limit: limits.max_cases,
		});
	}
	Ok(blank(
		capped("no findings", limits.max_name_bytes, text),
		Status::Passed,
	))
}

pub(crate) fn blank(name: Untrusted, status: Status) -> Case {
	Case {
		name,
		classname: Untrusted::default(),
		file: None,
		line: None,
		status,
		time_ms: 0,
		attempts: Vec::new(),
		attempts_omitted: 0,
		message: None,
		detail: None,
		system_out: None,
		system_err: None,
	}
}

/// `none`, `note`, `warning` or `error`; a result with no level is a warning, as SARIF says.
fn severity(level: Option<&str>) -> Option<Severity> {
	match level {
		None | Some("warning") => Some(Severity::Warning),
		Some("error") => Some(Severity::Error),
		Some("note") => Some(Severity::Note),
		_ => None,
	}
}

fn case_of(
	f: Finding,
	limits: &Limits,
	path_prefix: Option<&RepoPath>,
	fail_on: Severity,
	text: &mut usize,
) -> Case {
	let status = match f.kind.as_deref() {
		Some("pass") => Status::Passed,
		Some("informational" | "notApplicable") => Status::Skipped,
		_ if !f.suppressions.is_empty() => Status::Skipped,
		_ => match severity(f.level.as_deref()) {
			Some(s) if s >= fail_on => Status::Failed,
			_ => Status::Skipped,
		},
	};
	let rule = f.rule_id.filter(|r| !r.trim().is_empty());
	let mut case = blank(
		capped(
			rule.as_deref().unwrap_or("finding"),
			limits.max_name_bytes,
			text,
		),
		status,
	);

	let message = f
		.message
		.and_then(|m| m.text.or(m.markdown))
		.unwrap_or_default();
	let first = message.lines().next().unwrap_or("").trim();
	if !first.is_empty() {
		case.message = Some(capped(first, limits.max_message_bytes, text));
	}
	if message.trim().lines().nth(1).is_some() {
		case.detail = Some(capped(message.trim(), limits.max_detail_bytes, text));
	}

	let physical = f
		.locations
		.into_iter()
		.next()
		.and_then(|l| l.physical_location);
	let uri = physical
		.as_ref()
		.and_then(|p| p.artifact_location.as_ref())
		.and_then(|a| a.uri.as_deref());
	if let Some(path) = uri.and_then(relative_path).and_then(|p| match path_prefix {
		Some(prefix) => prefix.join(&p),
		None => RepoPath::parse(&p),
	}) {
		let line = physical
			.as_ref()
			.and_then(|p| p.region.as_ref())
			.and_then(|r| r.start_line)
			.and_then(|n| u32::try_from(n).ok())
			.filter(|n| *n > 0);
		case.classname = capped(path.as_str(), limits.max_name_bytes, text);
		case.file = Some(capped(path.as_str(), limits.max_name_bytes, text));
		case.line = line;
	}
	*text = text.saturating_add(CASE_OVERHEAD);
	case
}

/// The path a location names, when it is one inside the repository: relative, percent-decoded and
/// without a scheme. An absolute path or a URL names a place this tool cannot map to the commit.
fn relative_path(uri: &str) -> Option<String> {
	plain_relative(&percent_decode(uri)?)
}

/// The same for a path that is not a URI, so a `%` in it is a `%`.
pub(crate) fn plain_relative(decoded: &str) -> Option<String> {
	let path = decoded.trim_start_matches("./");
	let scheme = path.split_once(':').is_some_and(|(s, _)| {
		!s.is_empty()
			&& s.chars()
				.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
	});
	(!path.is_empty() && !path.starts_with('/') && !scheme).then(|| path.to_owned())
}

fn percent_decode(s: &str) -> Option<String> {
	let bytes = s.as_bytes();
	let mut out = Vec::with_capacity(bytes.len());
	let mut i = 0;
	while let Some(&b) = bytes.get(i) {
		if b == b'%' {
			let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
			out.push(u8::from_str_radix(hex, 16).ok()?);
			i += 3;
		} else {
			out.push(b);
			i += 1;
		}
	}
	String::from_utf8(out)
		.ok()
		.filter(|p| !p.chars().any(char::is_control))
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

	use super::*;

	#[test]
	fn a_uri_is_a_repository_path_or_nothing() {
		assert_eq!(relative_path("src/a.rs").as_deref(), Some("src/a.rs"));
		assert_eq!(relative_path("./src/a.rs").as_deref(), Some("src/a.rs"));
		assert_eq!(relative_path("src/a%20b.rs").as_deref(), Some("src/a b.rs"));
		for not in [
			"",
			"/home/runner/work/x/src/a.rs",
			"file:///home/runner/a.rs",
			"https://example.invalid/a.rs",
			"C:/a.rs",
			"src/%zz.rs",
			"src/%00.rs",
			"src/%ff.rs",
			"src/a%0a.rs",
			"src/a%2",
		] {
			assert_eq!(relative_path(not), None, "{not}");
		}
	}

	#[test]
	fn severity_follows_the_standard_and_a_missing_level_is_a_warning() {
		assert_eq!(severity(None), Some(Severity::Warning));
		assert_eq!(severity(Some("error")), Some(Severity::Error));
		assert_eq!(severity(Some("note")), Some(Severity::Note));
		assert_eq!(severity(Some("none")), None);
		assert_eq!(severity(Some("loud")), None);
	}
}
