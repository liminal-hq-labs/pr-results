// Shared test support: a run context, scenarios built from fixtures, and rendering to files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

pub mod inert;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use pr_results_core::{
	parse_audit_json, parse_clippy_json, parse_junit, parse_sarif, CommentKey, Flavour, Limits,
	RepoSlug, RunId, ServerUrl, Severity, Sha, Suite,
};
use pr_results_render::{
	check, comment, measure, status, CheckState, Comment, Conclusion, Degradation, Input, Level,
	Options, OtherCheck, RunContext, Section, StatusState, Style, Symbols, Target,
};

pub const SHA: &str = "3f9c2e1d4b5a69788796a5b4c3d2e1f001122334";
pub const LINK_PREFIX: &str = "https://github.com/example/app/";
pub const FORGEJO_LINK_PREFIX: &str = "https://forgejo.example.invalid/example/app/";

pub fn link_prefix(target: Target) -> &'static str {
	match target {
		Target::GitHub => LINK_PREFIX,
		Target::Forgejo => FORGEJO_LINK_PREFIX,
	}
}

pub fn ctx() -> RunContext {
	ctx_for(Target::GitHub)
}

pub fn ctx_for(target: Target) -> RunContext {
	RunContext {
		target,
		server: ServerUrl::parse(match target {
			Target::GitHub => "https://github.com",
			Target::Forgejo => "https://forgejo.example.invalid",
		})
		.unwrap(),
		repo: RepoSlug::parse("example/app").unwrap(),
		head_sha: Sha::parse(SHA).unwrap(),
		run_id: RunId(19_283_746),
		run_number: match target {
			Target::GitHub => None,
			Target::Forgejo => Some(412),
		},
		key: CommentKey::parse("ci").unwrap(),
		tool_version: "0.1.0",
	}
}

pub fn fixtures_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

pub fn golden_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

/// The suites of fixture files, read with `flavour` and merged by name.
pub fn load(files: &[&str], flavour: Flavour) -> Vec<Suite> {
	let mut report = pr_results_core::Report::default();
	for file in files {
		let bytes = std::fs::read(fixtures_dir().join(file)).unwrap();
		let parsed = parse_junit(&bytes, flavour, &Limits::default()).unwrap();
		report.suites.extend(parsed.suites);
	}
	report.merge_same_named();
	report.suites
}

/// The suites of SARIF fixtures, failing at `fail_on` and above.
pub fn load_sarif(files: &[&str], fail_on: Severity) -> Vec<Suite> {
	let mut report = pr_results_core::Report::default();
	for file in files {
		let bytes = std::fs::read(fixtures_dir().join(file)).unwrap();
		let parsed = parse_sarif(&bytes, &Limits::default(), None, fail_on).unwrap();
		report.suites.extend(parsed.suites);
	}
	report.merge_same_named();
	report.suites
}

/// The suites of a `cargo clippy --message-format=json` fixture.
pub fn load_clippy(file: &str, fail_on: Severity) -> Vec<Suite> {
	let bytes = std::fs::read(fixtures_dir().join(file)).unwrap();
	parse_clippy_json(&bytes, &Limits::default(), None, fail_on)
		.unwrap()
		.suites
}

/// The suites of a `cargo audit --json` fixture.
pub fn load_audit(file: &str, fail_on: Severity) -> Vec<Suite> {
	let bytes = std::fs::read(fixtures_dir().join(file)).unwrap();
	parse_audit_json(&bytes, &Limits::default(), None, fail_on)
		.unwrap()
		.suites
}

pub fn parse_text(xml: &str, flavour: Flavour) -> Vec<Suite> {
	let mut report = parse_junit(xml.as_bytes(), flavour, &Limits::default()).unwrap();
	report.merge_same_named();
	report.suites
}

#[derive(Clone)]
pub struct OwnedSection {
	pub title: String,
	pub check_name: String,
	pub required: bool,
	pub suites: Vec<Suite>,
	pub inputs_skipped: usize,
	pub run_locally: Option<String>,
	pub job: Option<String>,
}

pub fn section(title: &str, required: bool, suites: Vec<Suite>) -> OwnedSection {
	OwnedSection {
		title: title.to_owned(),
		check_name: format!("Example {title}"),
		required,
		suites,
		inputs_skipped: 0,
		run_locally: None,
		job: None,
	}
}

#[derive(Clone)]
pub struct Scenario {
	pub name: String,
	pub title: String,
	pub sections: Vec<OwnedSection>,
	pub options: Options,
	pub other_checks: Vec<OtherCheck>,
	/// Whether the check runs and annotations are written too.
	pub checks: bool,
	pub target: Target,
}

impl Scenario {
	pub fn new(name: &str, title: &str, sections: Vec<OwnedSection>) -> Self {
		Self {
			name: name.to_owned(),
			title: title.to_owned(),
			sections,
			options: Options::default(),
			other_checks: Vec::new(),
			checks: true,
			target: Target::GitHub,
		}
	}

	/// The scenario for Forgejo: a comment and a commit status per section.
	pub fn forgejo(mut self) -> Self {
		self.target = Target::Forgejo;
		self
	}

	pub fn with_other_checks(mut self, checks: Vec<OtherCheck>) -> Self {
		self.other_checks = checks;
		self
	}

	/// The table of other checks stays open even when every one of them passed.
	pub fn with_other_checks_open(mut self) -> Self {
		self.options.other_checks_open = true;
		self
	}

	/// The guided style, which is what the settings give when they say nothing.
	pub fn guided(mut self) -> Self {
		self.options.style = Style::Guided;
		self.options.symbols = Symbols::Glyphs;
		self.options.updated_note = true;
		self
	}

	/// The guided style with emoji, Jax, the bar and the alert block.
	pub fn enhanced(mut self) -> Self {
		self = self.guided();
		self.options.symbols = Symbols::Emoji;
		self.options.mascot = true;
		self.options.progress = true;
		self.options.alerts = true;
		self
	}

	/// The same scenario under another name, for the same input in another style.
	pub fn named(mut self, name: &str) -> Self {
		self.name = name.to_owned();
		self
	}

	pub fn with_options(mut self, options: Options) -> Self {
		self.options = options;
		self
	}

	pub fn comment_only(mut self) -> Self {
		self.checks = false;
		self
	}

	pub fn with_ctx<R>(&self, f: impl FnOnce(&Input<'_>) -> R) -> R {
		let ctx = ctx_for(self.target);
		let sections: Vec<Section<'_>> = self
			.sections
			.iter()
			.map(|s| Section {
				title: &s.title,
				check_name: &s.check_name,
				required: s.required,
				suites: &s.suites,
				inputs_skipped: s.inputs_skipped,
				run_locally: s.run_locally.as_deref(),
				job: s.job.as_deref(),
			})
			.collect();
		let input = Input {
			ctx: &ctx,
			title: &self.title,
			sections: &sections,
			other_checks: &self.other_checks,
			options: &self.options,
		};
		f(&input)
	}

	pub fn comment(&self) -> Comment {
		self.with_ctx(comment)
	}

	/// `state` and `description` of a section's commit status.
	pub fn status_file(&self, index: usize) -> String {
		let out = self.with_ctx(|i| status(i, index)).unwrap();
		let state = match out.state {
			StatusState::Success => "success",
			StatusState::Failure => "failure",
		};
		format!("state: {state}\ndescription: {}\n", out.description)
	}

	/// The files of the scenario's golden directory, in order.
	pub fn outputs(&self) -> Vec<(String, String)> {
		let mut files = vec![("comment.md".to_owned(), self.comment().body.to_string())];
		if self.target == Target::Forgejo {
			for index in 0..self.sections.len() {
				files.push((format!("status-{index}.txt"), self.status_file(index)));
			}
			return files;
		}
		if !self.checks {
			return files;
		}
		let mut notes = String::new();
		for index in 0..self.sections.len() {
			let out = self.with_ctx(|i| check(i, index)).unwrap();
			let conclusion = match out.conclusion {
				Conclusion::Success => "success",
				Conclusion::Failure => "failure",
				Conclusion::Neutral => "neutral",
			};
			files.push((
				format!("check-{index}.md"),
				format!(
					"conclusion: {conclusion}\ntitle: {}\n\n--- summary ---\n{}\n--- text ---\n{}",
					out.title, out.summary, out.text
				),
			));
			let _ = writeln!(
				notes,
				"check {index}: {} annotations, {} not shown",
				out.annotations.len(),
				out.annotations_capped
			);
			for a in &out.annotations {
				let level = match a.level {
					Level::Notice => "notice",
					Level::Warning => "warning",
					Level::Failure => "failure",
				};
				let _ = writeln!(
					notes,
					"- {level} {}:{}-{}\n  title: {}\n  message: {}",
					a.path.as_str(),
					a.start_line,
					a.end_line,
					a.title,
					a.message.as_str().replace('\n', "\n    ")
				);
			}
		}
		files.push(("annotations.txt".to_owned(), notes));
		files
	}
}

/// A nextest-shaped report with `failing` failures of `detail_lines` lines each, `passing` passes
/// and `flaky` tests that passed on their third try.
pub fn synthetic_nextest(
	name: &str,
	failing: usize,
	detail_lines: usize,
	passing: usize,
	flaky: usize,
) -> String {
	let mut xml = String::from("<testsuites name=\"nextest-run\">\n");
	let _ = writeln!(xml, "<testsuite name=\"{name}\">");
	for k in 0..failing {
		let _ = writeln!(
			xml,
			"<testcase name=\"module_{k}::fails_{k}\" classname=\"{name}\" time=\"0.{:03}\">",
			(k * 37) % 1000
		);
		let _ = writeln!(
			xml,
			"<failure message=\"assertion failed: value {k} is out of range\" type=\"test failure\">thread 'module_{k}::fails_{k}' panicked at crates/{name}/src/m{k}.rs:{}:5:",
			k + 10
		);
		for j in 0..detail_lines {
			let _ = writeln!(
				xml,
				"line {j}: expected {k} to equal {} in module_{k} while checking item {j}",
				k + 1
			);
		}
		xml.push_str("</failure>\n</testcase>\n");
	}
	for k in 0..passing {
		let _ = writeln!(
			xml,
			"<testcase name=\"module_{k}::passes_{k}\" classname=\"{name}\" time=\"0.{:03}\"/>",
			(k * 91) % 1000
		);
	}
	for k in 0..flaky {
		let _ = writeln!(
			xml,
			"<testcase name=\"module_{k}::flaky_{k}\" classname=\"{name}\" time=\"0.{:03}\">",
			(k * 13) % 1000
		);
		for _ in 0..2 {
			xml.push_str(
				"<flakyFailure type=\"test failure\"><stack-trace>timed out</stack-trace></flakyFailure>\n",
			);
		}
		xml.push_str("</testcase>\n");
	}
	xml.push_str("</testsuite>\n</testsuites>\n");
	xml
}

/// The base of the `truncated-*` scenarios: enough failures, flaky tests and passing suites that
/// every step of the budget has something to cut.
pub fn other(name: &str, state: CheckState) -> OtherCheck {
	OtherCheck {
		name: name.to_owned(),
		state,
		command: None,
	}
}

/// One check of each state, out of order, so the table's own ordering is what a golden shows.
pub fn other_checks_mix() -> Vec<OtherCheck> {
	vec![
		other("Format", CheckState::Passed),
		other("Docs build", CheckState::Running),
		other("Security audit", CheckState::Failed),
		other("Clippy (default)", CheckState::Passed),
		other("Release dry run", CheckState::Skipped),
		other("Deploy preview", CheckState::ActionRequired),
		other("Clippy (all-features)", CheckState::Passed),
		other("Nightly", CheckState::Cancelled),
	]
}

pub fn truncation_base() -> Scenario {
	truncation_base_for(Target::GitHub)
}

pub fn truncation_base_for(target: Target) -> Scenario {
	let mut sections = vec![section(
		"Rust tests",
		false,
		parse_text(&synthetic_nextest("core", 24, 12, 20, 8), Flavour::Nextest),
	)];
	for letter in ["Alpha", "Bravo", "Charlie", "Delta", "Echo"] {
		sections.push(section(
			&format!("{letter} checks"),
			false,
			parse_text(&synthetic_nextest(letter, 0, 0, 6, 0), Flavour::Nextest),
		));
	}
	let options = Options {
		max_failures: 30,
		..Options::default()
	};
	let mut sc = Scenario::new("truncation-base", "Example checks", sections)
		.with_options(options)
		.comment_only();
	sc.target = target;
	sc
}

/// For each step the base reaches, the comment limit at which it is the last one taken: the size
/// of the comment right after that step.
pub fn truncation_limits() -> Vec<(Degradation, usize)> {
	truncation_limits_for(Target::GitHub)
}

pub fn truncation_limits_for(target: Target) -> Vec<(Degradation, usize)> {
	limits_of(&truncation_base_for(target))
}

/// The same for any base scenario.
pub fn limits_of(base: &Scenario) -> Vec<(Degradation, usize)> {
	let mut found: Vec<(Degradation, usize)> = Vec::new();
	let mut limit = usize::MAX;
	for _ in 0..24 {
		let mut sc = base.clone();
		sc.options.comment_limit = limit;
		let c = sc.comment();
		let size = measure(&c.body);
		if let Some(last) = c.steps_taken.last() {
			if !found.iter().any(|(step, _)| step == last) {
				found.push((*last, size));
			}
			if *last == Degradation::Minimal {
				break;
			}
		}
		limit = size.saturating_sub(1);
	}
	found
}

pub fn truncation_scenarios() -> Vec<Scenario> {
	truncation_limits()
		.into_iter()
		.map(|(step, limit)| {
			let mut sc = truncation_base();
			sc.name = format!("truncated-{}", step.name());
			sc.options.comment_limit = limit;
			sc
		})
		.collect()
}

/// The truncation base in the guided style, with other checks so every step has something to cut.
pub fn guided_truncation_base() -> Scenario {
	let mut checks: Vec<OtherCheck> = (1..=6)
		.map(|n| other(&format!("Job number {n} of the matrix"), CheckState::Passed))
		.collect();
	checks.push(other_with(
		"Security audit",
		CheckState::Failed,
		"cargo audit",
	));
	let mut sc = truncation_base().guided().with_other_checks(checks);
	sc.name = "guided-truncation-base".to_owned();
	sc
}

pub fn guided_truncation_scenarios() -> Vec<Scenario> {
	limits_of(&guided_truncation_base())
		.into_iter()
		.map(|(step, limit)| {
			let mut sc = guided_truncation_base();
			sc.name = format!("guided-truncated-{}", step.name());
			sc.options.comment_limit = limit;
			sc
		})
		.collect()
}

pub fn other_with(name: &str, state: CheckState, command: &str) -> OtherCheck {
	OtherCheck {
		name: name.to_owned(),
		state,
		command: Some(command.to_owned()),
	}
}

/// The checks of a project with a few jobs: all of them passing but one.
fn project_checks(audit: CheckState) -> Vec<OtherCheck> {
	vec![
		other("Format", CheckState::Passed),
		other("Clippy (default)", CheckState::Passed),
		other("Clippy (no-default-features)", CheckState::Passed),
		other("Clippy (all-features)", CheckState::Passed),
		other_with("Security audit", audit, "cargo audit"),
	]
}

/// A nextest report with failing tests whose names are not safe to put in a command, and one that is.
fn awkward_names() -> String {
	let mut cases = String::new();
	for name in [
		"it_is_fine",
		"quote'; echo hi; '",
		"has space",
		"back`tick`",
		"dollar$(id)",
		"line\nbreak",
		"caf\u{e9}",
		"test(=x) | test(=y)",
	] {
		let name = name.replace('&', "&amp;").replace('<', "&lt;");
		cases.push_str(&format!(
			"<testcase name=\"{name}\" classname=\"app\" time=\"0.2\"><failure message=\"boom\"/></testcase>"
		));
	}
	format!("<testsuites><testsuite name=\"app\">{cases}</testsuite></testsuites>")
}

pub fn scenarios() -> Vec<Scenario> {
	let rust = |files: &[&str]| load(files, Flavour::Nextest);
	let mut all = vec![Scenario::new(
		"all-pass",
		"Example checks",
		vec![section("Rust tests", false, rust(&["nextest/pass.xml"]))],
	)];

	all.push(Scenario::new(
		"failures",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			rust(&["nextest/fail.xml", "nextest/retries-exhausted.xml"]),
		)],
	));

	all.push(Scenario::new(
		"flaky",
		"Example checks",
		vec![section("Rust tests", false, rust(&["nextest/flaky.xml"]))],
	));

	all.push(Scenario::new(
		"retries-exhausted",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			rust(&["nextest/retries-exhausted.xml"]),
		)],
	));

	let skipped = "<testsuites name=\"x\"><testsuite name=\"gfx\">\
		<testcase classname=\"gfx\" name=\"fills\" time=\"0.1\"/>\
		<testcase classname=\"gfx\" name=\"blits\" time=\"0.2\"/>\
		<testcase classname=\"gfx\" name=\"needs a display\" time=\"0\"><skipped message=\"no display\"/></testcase>\
		<testcase classname=\"gfx\" name=\"needs a mouse\" time=\"0\"><skipped/></testcase>\
		<testcase classname=\"gfx\" name=\"needs a serial port\" time=\"0\"><skipped/></testcase>\
		</testsuite></testsuites>";
	all.push(Scenario::new(
		"skipped",
		"Example checks",
		vec![section(
			"Graphics",
			false,
			parse_text(skipped, Flavour::Generic),
		)],
	));

	all.extend(truncation_scenarios());
	all.extend(guided_truncation_scenarios());

	let mut skipped_inputs = section("Playwright", false, Vec::new());
	skipped_inputs.inputs_skipped = 2;
	all.push(Scenario::new(
		"multiple-suites",
		"Example checks",
		vec![
			section("Rust tests", false, rust(&["nextest/pass.xml"])),
			section(
				"Frontend tests",
				false,
				load(
					&["vitest/pass.xml", "vitest/fail-with-stack.xml"],
					Flavour::Vitest,
				),
			),
			section(
				"Desk steps",
				true,
				load(&["steps/step-fail.xml"], Flavour::Steps),
			),
			skipped_inputs,
		],
	));

	all.push(Scenario::new(
		"steps-merged",
		"Example checks",
		vec![section(
			"Desk steps",
			true,
			load(
				&["steps/groups-split-a.xml", "steps/groups-split-b.xml"],
				Flavour::Steps,
			),
		)],
	));

	all.push(Scenario::new(
		"desk-job",
		"Desk checks",
		vec![section(
			"Desk steps",
			true,
			load(&["steps/desk-job.xml"], Flavour::Steps),
		)],
	));

	let hostile_title =
		"@everyone GH-123 <img src=x onerror=alert(1)> [x](https://example.invalid)";
	all.push(Scenario::new(
		"hostile",
		hostile_title,
		vec![
			section(
				"@owner/team #1 | pipe",
				false,
				load(&["hostile/strings.xml"], Flavour::Generic),
			),
			section("GH", false, Vec::new()),
		],
	));

	// Real output of the four JavaScript reporters, one section each.
	all.push(Scenario::new(
		"js-real",
		"Example checks",
		vec![
			section(
				"Vitest",
				false,
				load(&["vitest/real-file.xml"], Flavour::Auto),
			),
			section("Bun", false, load(&["bun/real-default.xml"], Flavour::Auto)),
			section("Jest", false, load(&["jest/real-file.xml"], Flavour::Auto)),
			section(
				"Playwright",
				false,
				load(
					&[
						"playwright/real-shard-1-of-2.xml",
						"playwright/real-shard-2-of-2.xml",
					],
					Flavour::Auto,
				),
			),
		],
	));

	// The same reports with the attacker's text in every place a name or message comes from.
	let real =
		|file: &str| String::from_utf8(std::fs::read(fixtures_dir().join(file)).unwrap()).unwrap();
	let evil = "@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe";
	let mut hostile_sections = Vec::new();
	for (title, file, flavour, words) in [
		(
			"Vitest",
			"vitest/real-file.xml",
			Flavour::Vitest,
			["fails with a stack", "always broken", "division by zero"],
		),
		(
			"Bun",
			"bun/real-default.xml",
			Flavour::Bun,
			["fails with a stack", "always broken", "division by zero"],
		),
		(
			"Jest",
			"jest/real-file.xml",
			Flavour::Jest,
			["fails with a stack", "always broken", "division by zero"],
		),
		(
			"Playwright",
			"playwright/real-default.xml",
			Flavour::Playwright,
			["fails with a stack", "always broken", "division by zero"],
		),
	] {
		let mut text = real(file);
		for word in words {
			text = text.replace(
				word,
				&evil
					.replace('&', "&amp;")
					.replace('<', "&lt;")
					.replace('>', "&gt;"),
			);
		}
		hostile_sections.push(section(title, false, parse_text(&text, flavour)));
	}
	all.push(Scenario::new("js-hostile", evil, hostile_sections));

	let rust_fail = || {
		section(
			"Rust tests",
			false,
			load(&["nextest/fail.xml"], Flavour::Nextest),
		)
	};
	all.push(
		Scenario::new("other-checks", "Example checks", vec![rust_fail()])
			.comment_only()
			.with_other_checks(other_checks_mix()),
	);
	all.push(
		Scenario::new("other-checks-glyphs", "Example checks", vec![rust_fail()])
			.comment_only()
			.with_other_checks(other_checks_mix())
			.with_options(Options {
				symbols: Symbols::Glyphs,
				..Options::default()
			}),
	);
	all.push(
		Scenario::new(
			"other-checks-all-pass",
			"Example checks",
			vec![section(
				"Rust tests",
				false,
				load(&["nextest/pass.xml"], Flavour::Nextest),
			)],
		)
		.comment_only()
		.with_other_checks(vec![
			other("Format", CheckState::Passed),
			other("Security audit", CheckState::Failed),
		]),
	);
	all.push(
		Scenario::new(
			"other-checks-hostile",
			"Example checks",
			vec![section(
				"Rust tests",
				false,
				load(&["nextest/pass.xml"], Flavour::Nextest),
			)],
		)
		.comment_only()
		.with_other_checks(vec![
			other(
				"@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe",
				CheckState::Failed,
			),
			other("line one\nline two", CheckState::Passed),
		]),
	);

	// The guided style: the same inputs again, with what it adds.
	let pass = || {
		section(
			"Rust tests",
			false,
			load(&["nextest/pass.xml"], Flavour::Nextest),
		)
	};
	let fail = || {
		section(
			"Rust tests",
			false,
			load(
				&["nextest/fail.xml", "nextest/retries-exhausted.xml"],
				Flavour::Nextest,
			),
		)
	};
	let flaky = || {
		section(
			"Rust tests",
			false,
			load(&["nextest/flaky.xml"], Flavour::Nextest),
		)
	};
	let multi = || {
		let mut skipped_inputs = section("Playwright", false, Vec::new());
		skipped_inputs.inputs_skipped = 2;
		vec![
			section(
				"Rust tests",
				false,
				load(&["nextest/pass.xml"], Flavour::Nextest),
			),
			section(
				"Frontend tests",
				false,
				load(
					&["vitest/pass.xml", "vitest/fail-with-stack.xml"],
					Flavour::Vitest,
				),
			),
			section(
				"Desk steps",
				true,
				load(&["steps/step-fail.xml"], Flavour::Steps),
			),
			skipped_inputs,
		]
	};
	let hostile_other = || {
		vec![
			other(
				"@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe",
				CheckState::Failed,
			),
			other_with(
				"</details> > [!CAUTION]",
				CheckState::ActionRequired,
				"cargo test -- --nocapture",
			),
			other("line one\nline two", CheckState::Passed),
		]
	};
	let hostile_title =
		"@everyone GH-123 <img src=x onerror=alert(1)> [x](https://example.invalid)";
	let mut with_job = fail();
	with_job.job = Some("Tests".to_owned());
	let mut guided = vec![
		Scenario::new("guided-job-replaced", "Example checks", vec![with_job]).with_other_checks({
			let mut checks = project_checks(CheckState::Failed);
			checks.push(other("Tests", CheckState::Failed));
			checks
		}),
		Scenario::new("guided-all-pass", "Example checks", vec![pass()])
			.with_other_checks(project_checks(CheckState::Passed)),
		Scenario::new("guided-other-checks-open", "Example checks", vec![pass()])
			.with_other_checks(project_checks(CheckState::Passed))
			.with_other_checks_open(),
		Scenario::new("guided-failures", "Example checks", vec![fail()])
			.with_other_checks(project_checks(CheckState::Failed)),
		Scenario::new("guided-flaky", "Example checks", vec![flaky()]),
		Scenario::new("guided-multiple-suites", "Example checks", multi())
			.with_other_checks(project_checks(CheckState::Failed)),
		Scenario::new(
			"guided-no-results-optional",
			"Example checks",
			vec![section("Rust tests", false, Vec::new())],
		)
		.with_other_checks(vec![
			other("Format", CheckState::Passed),
			other("Docs build", CheckState::Running),
			other_with("Security audit", CheckState::Failed, "cargo audit"),
		]),
		Scenario::new(
			"guided-no-results-required",
			"Example checks",
			vec![section("Desk steps", true, Vec::new())],
		),
		Scenario::new(
			"guided-hostile",
			hostile_title,
			vec![
				section(
					"@owner/team #1 | pipe",
					false,
					load(&["hostile/strings.xml"], Flavour::Generic),
				),
				section("GH", false, Vec::new()),
			],
		)
		.with_other_checks(hostile_other()),
		Scenario::new(
			"guided-awkward-names",
			"Example checks",
			vec![section(
				"Rust tests",
				false,
				parse_text(&awkward_names(), Flavour::Nextest),
			)],
		),
		Scenario::new("guided-forgejo-failures", "Example checks", vec![fail()]).forgejo(),
		Scenario::new(
			"guided-forgejo-no-results",
			"Example checks",
			vec![section("Rust tests", false, Vec::new())],
		)
		.forgejo(),
	];
	for sc in &mut guided {
		*sc = sc.clone().guided();
		sc.checks = sc.target == Target::GitHub && sc.name != "guided-awkward-names";
	}
	all.extend(guided);

	// Findings from a SARIF file, shown as the suite they are, with annotations in the check run.
	all.push(
		Scenario::new(
			"sarif-findings",
			"Example checks",
			vec![section(
				"Clippy",
				false,
				load_sarif(&["sarif/clippy.sarif"], Severity::Warning),
			)],
		)
		.guided(),
	);
	all.push(
		Scenario::new(
			"sarif-errors-only",
			"Example checks",
			vec![section(
				"Clippy",
				false,
				load_sarif(&["sarif/clippy.sarif"], Severity::Error),
			)],
		)
		.guided(),
	);
	all.push(
		Scenario::new(
			"sarif-clean-and-multi",
			"Example checks",
			vec![
				section(
					"Scanners",
					false,
					load_sarif(&["sarif/multi-run.sarif"], Severity::Warning),
				),
				section(
					"Semgrep",
					false,
					load_sarif(&["sarif/clean.sarif"], Severity::Warning),
				),
			],
		)
		.guided(),
	);
	all.push(
		Scenario::new(
			"sarif-hostile",
			"Example checks",
			vec![section(
				"Scan",
				false,
				load_sarif(&["sarif/hostile.sarif"], Severity::Warning),
			)],
		)
		.guided(),
	);

	// What cargo writes itself: clippy's diagnostics and the audit's advisories.
	all.push(
		Scenario::new(
			"cargo-clippy-findings",
			"Example checks",
			vec![section(
				"Clippy",
				false,
				load_clippy("cargo/clippy.json", Severity::Warning),
			)],
		)
		.guided(),
	);
	all.push(
		Scenario::new(
			"cargo-audit-findings",
			"Example checks",
			vec![section(
				"Audit",
				false,
				load_audit("cargo/audit.json", Severity::Warning),
			)],
		)
		.guided(),
	);
	all.push(
		Scenario::new(
			"cargo-audit-errors-only",
			"Example checks",
			vec![section(
				"Audit",
				false,
				load_audit("cargo/audit.json", Severity::Error),
			)],
		)
		.guided(),
	);
	all.push(
		Scenario::new(
			"cargo-clean",
			"Example checks",
			vec![
				section(
					"Clippy",
					false,
					load_clippy("cargo/clippy-clean.json", Severity::Warning),
				),
				section(
					"Audit",
					false,
					load_audit("cargo/audit-clean.json", Severity::Warning),
				),
			],
		)
		.guided(),
	);

	// Everything the enhanced style adds, and what it leaves out on Forgejo.
	let mut enhanced = vec![
		Scenario::new("enhanced-all-pass", "Example checks", vec![pass()])
			.with_other_checks(project_checks(CheckState::Passed)),
		Scenario::new("enhanced-other-checks-open", "Example checks", vec![pass()])
			.with_other_checks(project_checks(CheckState::Passed))
			.with_other_checks_open(),
		Scenario::new("enhanced-failures", "Example checks", vec![fail()])
			.with_other_checks(project_checks(CheckState::Failed)),
		Scenario::new("enhanced-flaky", "Example checks", vec![flaky()]),
		Scenario::new("enhanced-multiple-suites", "Example checks", multi())
			.with_other_checks(project_checks(CheckState::Failed)),
		Scenario::new(
			"enhanced-no-results",
			"Example checks",
			vec![section("Rust tests", false, Vec::new())],
		)
		.with_other_checks(vec![other_with(
			"Security audit",
			CheckState::Failed,
			"cargo audit",
		)]),
		Scenario::new(
			"enhanced-hostile",
			hostile_title,
			vec![section(
				"@owner/team #1 | pipe",
				false,
				load(&["hostile/strings.xml"], Flavour::Generic),
			)],
		)
		.with_other_checks(hostile_other()),
		Scenario::new("enhanced-forgejo-failures", "Example checks", vec![fail()]).forgejo(),
	];
	for sc in &mut enhanced {
		*sc = sc.clone().enhanced();
		sc.checks = false;
	}
	all.extend(enhanced);

	all.push(Scenario::new(
		"no-results-required",
		"Example checks",
		vec![section("Desk steps", true, Vec::new())],
	));
	all.push(Scenario::new(
		"no-results-optional",
		"Example checks",
		vec![section("Rust tests", false, Vec::new())],
	));

	let glyph_options = Options {
		symbols: Symbols::Glyphs,
		flaky_annotations: true,
		..Options::default()
	};
	all.push(
		Scenario::new(
			"glyphs",
			"Example checks",
			vec![
				section(
					"Rust tests",
					false,
					rust(&["nextest/fail.xml", "nextest/flaky.xml"]),
				),
				section(
					"Frontend tests",
					false,
					load(&["vitest/pass.xml"], Flavour::Vitest),
				),
				section("Playwright", false, Vec::new()),
				section("Desk steps", true, Vec::new()),
			],
		)
		.with_options(glyph_options),
	);

	all.extend(forgejo_scenarios());
	all
}

/// The hostile title for Forgejo: everything it links outside code, and what GitHub does too.
pub const FORGEJO_HOSTILE_TITLE: &str =
	"@everyone GH-123 !12 $x$ deadbeef0 :smile: x:wink [x](https://example.invalid)";

pub fn forgejo_scenarios() -> Vec<Scenario> {
	let rust = |files: &[&str]| load(files, Flavour::Nextest);
	let mut all = vec![
		Scenario::new(
			"forgejo-all-pass",
			"Example checks",
			vec![section("Rust tests", false, rust(&["nextest/pass.xml"]))],
		)
		.forgejo(),
		Scenario::new(
			"forgejo-failures",
			"Example checks",
			vec![
				section(
					"Rust tests",
					false,
					rust(&["nextest/fail.xml", "nextest/retries-exhausted.xml"]),
				),
				section(
					"Frontend tests",
					false,
					load(&["vitest/fail-with-stack.xml"], Flavour::Vitest),
				),
				section("Desk steps", true, Vec::new()),
				section("Playwright", false, Vec::new()),
			],
		)
		.forgejo(),
		Scenario::new(
			"forgejo-hostile",
			FORGEJO_HOSTILE_TITLE,
			vec![
				section(
					"@owner/team #1 !12 $x$ | pipe",
					false,
					load(&["hostile/strings.xml"], Flavour::Generic),
				),
				section("GH", false, Vec::new()),
			],
		)
		.forgejo(),
	];
	let mut minimal = truncation_base();
	minimal.options.comment_limit = 700;
	minimal.name = "forgejo-truncated-minimal".to_owned();
	all.push(minimal.forgejo());
	all
}

/// Asserts every Markdown output of the scenario is inert.
pub fn assert_inert_all(sc: &Scenario) {
	for (file, text) in sc.outputs() {
		if file.ends_with(".md") {
			inert::assert_inert_for(
				sc.target,
				&format!("{}/{file}", sc.name),
				&text,
				link_prefix(sc.target),
			);
		}
	}
}
