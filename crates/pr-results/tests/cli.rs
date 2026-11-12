// The built binary: its help, its refusals and `render` on the fixtures.
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

use std::process::{Command, Output};

use common::{fixture, root, scratch, DESK_TOML};

fn bin(args: &[&str]) -> Output {
	// Each call has a directory of its own to run in: tests run side by side.
	static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
	let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
	Command::new(env!("CARGO_BIN_EXE_pr-results"))
		.args(args)
		// A publisher's environment must not leak into a test of the command line.
		.env_remove("GITHUB_EVENT_NAME")
		.env_remove("GITHUB_STEP_SUMMARY")
		.current_dir(scratch(&format!("bin-cwd-{}-{n}", std::process::id())))
		.output()
		.unwrap()
}

fn text(bytes: &[u8]) -> String {
	String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn help_lists_the_commands_and_the_exit_codes() {
	let o = bin(&["--help"]);
	assert_eq!(o.status.code(), Some(0));
	let help = text(&o.stdout);
	for word in [
		"publish",
		"render",
		"validate-config",
		"--dry-run",
		"--fail-on-failures",
		"--version",
	] {
		assert!(help.contains(word), "help lacks {word}");
	}
	assert!(help.contains("4 tests failed"));
	assert_eq!(text(&bin(&[]).stdout), help);
	assert_eq!(text(&bin(&["publish", "--help"]).stdout), help);
}

#[test]
fn help_is_spelled_the_canadian_way() {
	let help = text(&bin(&["--help"]).stdout).to_lowercase();
	for american in [
		"color", "canceled", "behavior", "artifact", "license ", "center", "organiz",
	] {
		assert!(!help.contains(american), "help says {american}");
	}
	assert!(
		!help.contains(" -- "),
		"a double hyphen stands in for a dash"
	);
	assert!(help.is_ascii());
}

#[test]
fn the_version_is_the_crates() {
	let o = bin(&["--version"]);
	assert_eq!(
		text(&o.stdout),
		format!("pr-results {}\n", env!("CARGO_PKG_VERSION"))
	);
}

#[test]
fn a_mistake_is_exit_two_with_a_pointer_to_help() {
	for args in [
		&["post"][..],
		&["publish", "--force"],
		&["render"],
		&["render", "--flavour", "mocha", "x.xml"],
		&["validate-config"],
		&["collect"],
	] {
		let o = bin(args);
		assert_eq!(o.status.code(), Some(2), "{args:?}");
		assert!(
			text(&o.stderr).contains("Try 'pr-results --help'."),
			"{args:?}"
		);
		assert!(o.stdout.is_empty(), "{args:?}");
	}
}

#[test]
fn an_argument_cannot_write_a_workflow_command() {
	let o = bin(&["::set-output name=x::y"]);
	assert_eq!(o.status.code(), Some(2));
	let err = text(&o.stderr);
	for line in err.lines() {
		assert!(!line.trim_start().starts_with("::"), "{line}");
	}
	assert!(err.contains("unknown command"));
}

#[test]
fn publish_refuses_outside_a_workflow_run() {
	let o = bin(&["publish"]);
	assert_eq!(o.status.code(), Some(2));
	assert!(text(&o.stderr).contains("GITHUB_EVENT_NAME is not set"));
}

#[test]
fn validate_config_says_valid_or_why_not() {
	let dir = scratch("validate");
	let good = dir.join("good.toml");
	std::fs::write(&good, DESK_TOML).unwrap();
	let o = bin(&["validate-config", good.to_str().unwrap()]);
	assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
	assert!(text(&o.stdout).contains("valid, 2 suites"));

	let bad = dir.join("bad.toml");
	std::fs::write(&bad, "title = \"x\"\nunknown = 1\n").unwrap();
	let o = bin(&["validate-config", bad.to_str().unwrap()]);
	assert_eq!(o.status.code(), Some(2));
	assert!(text(&o.stderr).contains("line 2: unknown key `unknown`"));

	let o = bin(&["validate-config", dir.join("none.toml").to_str().unwrap()]);
	assert_eq!(o.status.code(), Some(2));
}

#[test]
fn the_repositorys_own_settings_file_is_valid() {
	let path = root().join(".github/pr-results.toml");
	let o = bin(&["validate-config", path.to_str().unwrap()]);
	assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
}

// ---- render --------------------------------------------------------------------------------

/// The footer names the crate's version, where the goldens name the release.
fn as_golden(text: &str) -> String {
	text.replace(env!("CARGO_PKG_VERSION"), "0.1.0")
}

#[test]
fn render_prints_the_comment() {
	let file = root().join("tests/fixtures/nextest/pass.xml");
	let o = bin(&[
		"render",
		"--title",
		"Example checks",
		file.to_str().unwrap(),
	]);
	assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
	let out = text(&o.stdout);
	assert!(out.starts_with("<!-- pr-results:1 key=ci sha="));
	assert!(out.contains("### Example checks: All 3 passed, 1 skipped"));
	assert!(o.stderr.is_empty());
}

#[test]
fn render_writes_what_the_golden_scenarios_hold() {
	// `all-pass` is one nextest fixture under one section called "Rust tests", with the title
	// "Example checks": the same input through the command line must give the same files.
	let dir = scratch("render-golden");
	let config = dir.join("rust.toml");
	std::fs::write(
		&config,
		"title = \"Example checks\"\n\n[[suite]]\nname = \"Rust tests\"\ncheck = \"Example Rust tests\"\nartifact = \"x\"\nflavour = \"nextest\"\n",
	)
	.unwrap();
	let out = dir.join("out");
	let file = root().join("tests/fixtures/nextest/pass.xml");

	let o = bin(&[
		"render",
		"--config",
		config.to_str().unwrap(),
		file.to_str().unwrap(),
		"--out",
		out.to_str().unwrap(),
	]);

	assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
	let golden = root().join("tests/golden/all-pass");
	assert_eq!(
		as_golden(&std::fs::read_to_string(out.join("comment.md")).unwrap()),
		std::fs::read_to_string(golden.join("comment.md")).unwrap()
	);
	assert_eq!(
		as_golden(&std::fs::read_to_string(out.join("checks/0.md")).unwrap()),
		std::fs::read_to_string(golden.join("check-0.md")).unwrap()
	);
}

#[test]
fn render_reads_desks_step_shape_like_the_golden() {
	let dir = scratch("render-desk");
	let config = dir.join("desk.toml");
	std::fs::write(
		&config,
		"title = \"Example checks\"\n\n[[suite]]\nname = \"Desk steps\"\ncheck = \"Example Desk steps\"\nartifact = \"x\"\nflavour = \"steps\"\n",
	)
	.unwrap();
	let out = dir.join("out");
	let file = root().join("tests/fixtures/steps/desk-job.xml");

	let o = bin(&[
		"render",
		"--config",
		config.to_str().unwrap(),
		"--out",
		out.to_str().unwrap(),
		file.to_str().unwrap(),
	]);

	assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
	let comment = std::fs::read_to_string(out.join("comment.md")).unwrap();
	assert!(comment.contains("`Desk steps`"));
	assert!(comment.contains("Failure 1 of"));
	assert!(out.join("checks/0.md").is_file());
	let _ = fixture("steps/desk-job.xml");
}

#[test]
fn render_sorts_files_into_the_suites_that_claim_them() {
	let dir = scratch("render-two");
	let config = dir.join("desk.toml");
	std::fs::write(&config, DESK_TOML).unwrap();
	let out = dir.join("out");
	let steps = dir.join("results/steps.xml");
	let rust = dir.join("results/nextest-core.xml");
	std::fs::create_dir_all(dir.join("results")).unwrap();
	std::fs::write(&steps, fixture("steps/desk-job.xml")).unwrap();
	std::fs::write(&rust, fixture("nextest/pass.xml")).unwrap();

	let o = bin(&[
		"render",
		"--config",
		config.to_str().unwrap(),
		steps.to_str().unwrap(),
		rust.to_str().unwrap(),
		"--out",
		out.to_str().unwrap(),
	]);

	assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
	assert!(out.join("checks/0.md").is_file());
	assert!(out.join("checks/1.md").is_file());
	assert!(!out.join("checks/2.md").exists());
	let comment = std::fs::read_to_string(out.join("comment.md")).unwrap();
	assert!(comment.contains("### Desk checks"));
	assert!(comment.contains("`Desk steps`") && comment.contains("`Rust tests`"));

	// A file no suite claims is a mistake, not a silent omission.
	let stray = dir.join("results/other.json");
	std::fs::write(&stray, "{}").unwrap();
	let o = bin(&[
		"render",
		"--config",
		config.to_str().unwrap(),
		stray.to_str().unwrap(),
	]);
	assert_eq!(o.status.code(), Some(2));
	assert!(text(&o.stderr).contains("matches no suite"));
}

#[test]
fn render_refuses_a_file_it_cannot_use() {
	let dir = scratch("render-bad");
	let missing = dir.join("none.xml");
	assert_eq!(
		bin(&["render", missing.to_str().unwrap()]).status.code(),
		Some(2)
	);

	let bad = dir.join("bad.xml");
	std::fs::write(&bad, "<testsuite><testcase").unwrap();
	let o = bin(&["render", bad.to_str().unwrap()]);
	assert_eq!(o.status.code(), Some(2));
	assert!(o.stdout.is_empty());

	let doctype = dir.join("doctype.xml");
	std::fs::write(&doctype, "<!DOCTYPE x [<!ENTITY a \"b\">]><testsuite/>").unwrap();
	assert_eq!(
		bin(&["render", doctype.to_str().unwrap()]).status.code(),
		Some(2)
	);
}

#[test]
fn render_is_deterministic() {
	let file = root().join("tests/fixtures/nextest/fail.xml");
	let a = bin(&["render", file.to_str().unwrap()]);
	let b = bin(&["render", file.to_str().unwrap()]);
	assert_eq!(a.stdout, b.stdout);
	assert!(!a.stdout.is_empty());
}

#[test]
fn flavour_overrides_the_suites() {
	let file = root().join("tests/fixtures/bun/pass.xml");
	let o = bin(&["render", "--flavour", "bun", file.to_str().unwrap()]);
	assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
}
