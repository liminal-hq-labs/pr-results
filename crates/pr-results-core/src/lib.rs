// The pr-results core: the normalized model, the `Safe` escaping type and the JUnit and SARIF readers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Everything here is pure: no network, no clocks, no environment. Every string that comes from a
//! test report is held as [`Untrusted`] and reaches output only through [`Safe`].

#![forbid(unsafe_code)]

mod anonymize;
mod ansi;
mod cargo;
mod delta;
mod error;
mod flavour;
mod glob;
mod hmac;
mod junit;
mod model;
mod safe;
mod sarif;
mod trusted;

pub use anonymize::anonymize_junit;
pub use ansi::strip_ansi;
pub use cargo::{parse_audit_json, parse_clippy_json};
pub use delta::{test_delta, TestDelta, TestId, MAX_LISTED};
pub use error::ParseError;
pub use glob::{Glob, GlobError, MAX_GLOB_BYTES};
pub use junit::{parse_junit, parse_junit_in, Limits};
pub use model::{
	Attempt, Case, Flavour, InputFormat, Report, Severity, Status, Suite, Untrusted, REPORT_SCHEMA,
};
pub use sarif::parse_sarif;

/// Parses one file of `format`. JUnit is read as `flavour`; a findings format fails at or above
/// `fail_on`. Every place that reads a suite's files goes through here, so a new format is added
/// once.
pub fn parse_report(
	input: &[u8],
	format: InputFormat,
	flavour: Flavour,
	limits: &Limits,
	path_prefix: Option<&RepoPath>,
	fail_on: Severity,
) -> Result<Report, ParseError> {
	match format {
		InputFormat::Junit => parse_junit_in(input, flavour, limits, path_prefix),
		InputFormat::Sarif => parse_sarif(input, limits, path_prefix, fail_on),
		InputFormat::ClippyJson => parse_clippy_json(input, limits, path_prefix, fail_on),
		InputFormat::CargoAuditJson => parse_audit_json(input, limits, path_prefix, fail_on),
	}
}
pub use safe::{Safe, Source, DEFAULT_MAX_CHARS};
pub use trusted::{CommentKey, Marker, RepoPath, RepoSlug, RunId, ServerUrl, Sha, TrustedUrl};
