// The readers for `cargo clippy --message-format=json` and `cargo audit --json`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

mod common;

use common::fixture;
use pr_results_core::{
	parse_audit_json, parse_clippy_json, Case, Limits, ParseError, RepoPath, Report, Severity,
	Status,
};

fn clippy(path: &str, fail_on: Severity) -> Report {
	parse_clippy_json(&fixture(path), &Limits::default(), None, fail_on)
		.unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn audit(path: &str, fail_on: Severity) -> Report {
	parse_audit_json(&fixture(path), &Limits::default(), None, fail_on)
		.unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn names(cases: &[Case]) -> Vec<(&str, Status)> {
	cases.iter().map(|c| (c.name.as_str(), c.status)).collect()
}

// ---- clippy -------------------------------------------------------------------------------

#[test]
fn a_diagnostic_is_a_case_at_its_primary_span() {
	use Status::Failed;
	let r = clippy("cargo/clippy.json", Severity::Warning);
	assert_eq!(r.suites.len(), 1);
	let suite = &r.suites[0];
	assert_eq!(suite.name.as_str(), "clippy");
	assert_eq!(
		names(&suite.cases),
		[
			("clippy::too_many_arguments", Failed),
			("clippy::needless_return", Failed),
			("unused_imports", Failed),
			("E0308", Failed),
		],
		"the duplicate from a second target and the summary line are not findings"
	);
	let first = &suite.cases[0];
	assert_eq!(
		first.file.as_ref().unwrap().as_str(),
		"crates/pr-results-forge/src/publish.rs"
	);
	assert_eq!(first.line, Some(454));
	assert_eq!(
		first.classname.as_str(),
		first.file.as_ref().unwrap().as_str()
	);
	assert_eq!(
		first.message.as_ref().unwrap().as_str(),
		"this function has too many arguments (8/7)"
	);
	// With two spans, the primary one is where the finding is.
	assert_eq!(suite.cases[3].line, Some(8));
}

#[test]
fn the_rendered_text_is_the_detail_without_terminal_escapes() {
	let r = clippy("cargo/clippy.json", Severity::Warning);
	let plain = r.suites[0].cases[0].detail.as_ref().unwrap().as_str();
	assert!(plain.starts_with("error: this function has too many arguments (8/7)\n"));
	assert!(plain.contains("= help: for further information"));
	let coloured = r.suites[0].cases[1].detail.as_ref().unwrap().as_str();
	assert!(!coloured.contains('\u{1b}'), "{coloured:?}");
	assert!(coloured.contains("warning: unneeded `return` statement"));
}

#[test]
fn a_file_that_is_not_in_the_repository_loses_its_location_and_keeps_the_finding() {
	let r = clippy("cargo/clippy.json", Severity::Warning);
	let registry = &r.suites[0].cases[2];
	assert_eq!(registry.name.as_str(), "unused_imports");
	assert!(registry.file.is_none() && registry.line.is_none());
	assert_eq!(registry.status, Status::Failed);
}

#[test]
fn the_threshold_decides_what_fails() {
	use Status::{Failed, Skipped};
	let r = clippy("cargo/clippy.json", Severity::Error);
	assert_eq!(
		names(&r.suites[0].cases),
		[
			("clippy::too_many_arguments", Failed),
			("clippy::needless_return", Skipped),
			("unused_imports", Skipped),
			("E0308", Failed),
		]
	);
}

#[test]
fn a_clean_build_is_one_passing_case() {
	let r = clippy("cargo/clippy-clean.json", Severity::Warning);
	assert_eq!(names(&r.suites[0].cases), [("no findings", Status::Passed)]);
}

#[test]
fn the_path_prefix_goes_in_front_of_a_workspace_path() {
	let prefix = RepoPath::parse("backend/").unwrap();
	let r = parse_clippy_json(
		&fixture("cargo/clippy.json"),
		&Limits::default(),
		Some(&prefix),
		Severity::Warning,
	)
	.unwrap();
	assert_eq!(
		r.suites[0].cases[1].file.as_ref().unwrap().as_str(),
		"backend/src/lib.rs"
	);
}

#[test]
fn a_file_without_the_end_of_the_build_was_cut_short() {
	let text = String::from_utf8(fixture("cargo/clippy.json")).unwrap();
	let without: String = text
		.lines()
		.filter(|l| !l.contains("build-finished"))
		.map(|l| format!("{l}\n"))
		.collect();
	assert_eq!(
		parse_clippy_json(
			without.as_bytes(),
			&Limits::default(),
			None,
			Severity::Warning
		)
		.unwrap_err(),
		ParseError::Truncated
	);
}

fn clippy_err(bytes: &[u8]) -> ParseError {
	parse_clippy_json(bytes, &Limits::default(), None, Severity::Warning).unwrap_err()
}

#[test]
fn a_line_that_is_not_json_is_refused_and_named_by_its_number() {
	assert_eq!(clippy_err(b""), ParseError::Empty);
	let e = clippy_err(b"{\"reason\":\"build-finished\",\"success\":true}\nwarning: not json\n");
	assert!(matches!(e, ParseError::NotJson { line: 2, .. }), "{e:?}");
	assert!(matches!(
		clippy_err(
			b"{\"reason\":\"compiler-message\",\"message\":{\"spans\":[{\"line_start\":\"x\"}]}}\n"
		),
		ParseError::NotJson { line: 1, .. }
	));
}

#[test]
fn every_strict_prefix_of_either_document_is_refused() {
	for name in ["clippy", "clippy-clean"] {
		let bytes = fixture(&format!("cargo/{name}.json"));
		let end = bytes
			.iter()
			.rposition(|b| !b.is_ascii_whitespace())
			.unwrap() + 1;
		assert!(
			parse_clippy_json(&bytes[..end], &Limits::default(), None, Severity::Warning).is_ok()
		);
		for cut in 0..end {
			let result =
				parse_clippy_json(&bytes[..cut], &Limits::default(), None, Severity::Warning);
			assert!(result.is_err(), "{name} cut at {cut} of {end} was accepted");
		}
	}
	for name in ["audit", "audit-clean"] {
		let bytes = fixture(&format!("cargo/{name}.json"));
		let end = bytes
			.iter()
			.rposition(|b| !b.is_ascii_whitespace())
			.unwrap() + 1;
		assert!(
			parse_audit_json(&bytes[..end], &Limits::default(), None, Severity::Warning).is_ok()
		);
		for cut in 0..end {
			let result =
				parse_audit_json(&bytes[..cut], &Limits::default(), None, Severity::Warning);
			assert!(result.is_err(), "{name} cut at {cut} of {end} was accepted");
		}
	}
}

#[test]
fn text_from_a_diagnostic_is_kept_raw_for_the_escaping_layer() {
	let line = serde_like(
		"error",
		"@everyone GH-123 <img src=x onerror=alert(1)> `x` | pipe",
		"@owner/team",
		"../../etc/passwd",
	);
	let json = format!("{line}\n{{\"reason\":\"build-finished\",\"success\":false}}\n");
	let r =
		parse_clippy_json(json.as_bytes(), &Limits::default(), None, Severity::Warning).unwrap();
	let case = &r.suites[0].cases[0];
	assert_eq!(case.name.as_str(), "@owner/team");
	assert!(case.message.as_ref().unwrap().as_str().contains("<img"));
	assert!(case.file.is_none(), "a path above the root is no location");
	assert!(
		!format!("{r:?}").contains("@everyone"),
		"Debug never prints report text"
	);
}

fn serde_like(level: &str, message: &str, code: &str, file: &str) -> String {
	format!(
		"{{\"reason\":\"compiler-message\",\"message\":{{\"level\":\"{level}\",\"message\":\"{message}\",\"code\":{{\"code\":\"{code}\"}},\"spans\":[{{\"file_name\":\"{file}\",\"line_start\":3,\"is_primary\":true}}],\"rendered\":\"{level}: {message}\\n --> {file}\\n\"}}}}"
	)
}

#[test]
fn the_limits_apply_to_diagnostics_too() {
	let bytes = fixture("cargo/clippy.json");
	let with = |l: Limits| parse_clippy_json(&bytes, &l, None, Severity::Warning).unwrap_err();
	assert!(matches!(
		with(Limits {
			max_bytes: 10,
			..Limits::default()
		}),
		ParseError::TooLarge { .. }
	));
	assert_eq!(
		with(Limits {
			max_cases: 2,
			..Limits::default()
		}),
		ParseError::TooManyCases { limit: 2 }
	);
	assert_eq!(
		with(Limits {
			max_text_bytes: 100,
			..Limits::default()
		}),
		ParseError::TooMuchText { limit: 100 }
	);
}

// ---- cargo audit --------------------------------------------------------------------------

#[test]
fn a_vulnerability_is_an_error_and_a_warning_is_a_warning_at_the_lockfile() {
	use Status::Failed;
	let r = audit("cargo/audit.json", Severity::Warning);
	let suite = &r.suites[0];
	assert_eq!(suite.name.as_str(), "cargo audit");
	assert_eq!(
		names(&suite.cases),
		[
			("RUSTSEC-2020-0071", Failed),
			("RUSTSEC-2022-0013", Failed),
			("RUSTSEC-2021-0145", Failed),
			("yanked", Failed),
		],
		"the same advisory for the same crate is one finding"
	);
	let first = &suite.cases[0];
	assert_eq!(first.classname.as_str(), "time 0.1.45");
	assert_eq!(first.file.as_ref().unwrap().as_str(), "Cargo.lock");
	assert_eq!(first.line, None);
	assert_eq!(
		first.message.as_ref().unwrap().as_str(),
		"Potential segfault in the time crate"
	);
	let detail = first.detail.as_ref().unwrap().as_str();
	assert!(detail.contains("segfault due to dereferencing"), "{detail}");
	assert!(detail.contains("Patched versions: >=0.2.23"), "{detail}");
	assert!(
		detail.ends_with("https://github.com/time-rs/time/issues/293"),
		"{detail}"
	);
}

#[test]
fn error_as_the_threshold_is_what_cargo_audit_itself_does() {
	use Status::{Failed, Skipped};
	let r = audit("cargo/audit.json", Severity::Error);
	assert_eq!(
		names(&r.suites[0].cases),
		[
			("RUSTSEC-2020-0071", Failed),
			("RUSTSEC-2022-0013", Failed),
			("RUSTSEC-2021-0145", Skipped),
			("yanked", Skipped),
		]
	);
}

#[test]
fn a_clean_audit_is_one_passing_case_and_the_lockfile_follows_the_prefix() {
	let r = audit("cargo/audit-clean.json", Severity::Warning);
	assert_eq!(names(&r.suites[0].cases), [("no findings", Status::Passed)]);

	let prefix = RepoPath::parse("backend/").unwrap();
	let r = parse_audit_json(
		&fixture("cargo/audit.json"),
		&Limits::default(),
		Some(&prefix),
		Severity::Warning,
	)
	.unwrap();
	assert_eq!(
		r.suites[0].cases[0].file.as_ref().unwrap().as_str(),
		"backend/Cargo.lock"
	);
}

#[test]
fn output_that_is_not_the_audits_is_refused() {
	let err =
		|b: &[u8]| parse_audit_json(b, &Limits::default(), None, Severity::Warning).unwrap_err();
	assert_eq!(err(b""), ParseError::Empty);
	assert_eq!(
		err(b"{}"),
		ParseError::Structure("not the output of cargo audit --json")
	);
	assert_eq!(
		err(br#"{"version":"2.1.0","runs":[]}"#),
		ParseError::Structure("not the output of cargo audit --json")
	);
	assert!(matches!(err(b"[]"), ParseError::NotJson { .. }));
	assert!(matches!(
		err(br#"{"vulnerabilities":{"list":"x"}}"#),
		ParseError::NotJson { .. }
	));
}

#[test]
fn audit_text_is_kept_raw_and_the_limits_apply() {
	let json = br#"{"vulnerabilities":{"list":[{"advisory":{"id":"@everyone <b>","title":"GH-123 `x` | y","description":"[a](https://example.invalid)"},"package":{"name":"p","version":"1"}}]}}"#;
	let r = parse_audit_json(json, &Limits::default(), None, Severity::Warning).unwrap();
	assert_eq!(r.suites[0].cases[0].name.as_str(), "@everyone <b>");
	let limits = Limits {
		max_cases: 0,
		..Limits::default()
	};
	assert_eq!(
		parse_audit_json(
			&fixture("cargo/audit.json"),
			&limits,
			None,
			Severity::Warning
		)
		.unwrap_err(),
		ParseError::TooManyCases { limit: 0 }
	);
}
