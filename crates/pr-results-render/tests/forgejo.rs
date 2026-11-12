// The Forgejo target: wording, statuses, the footer, `path:line`, inert titles and the job summary.
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

use common::{
	assert_inert_all, forgejo_scenarios, inert, parse_text, scenarios, section, synthetic_nextest,
	Scenario, FORGEJO_HOSTILE_TITLE, FORGEJO_LINK_PREFIX,
};
use pr_results_core::Flavour;
use pr_results_render::{
	forgejo_inert_title, job_summary, job_summary_within, StatusState, Target,
	JOB_SUMMARY_CAP_BYTES, STATUS_DESCRIPTION_CHARS,
};

fn failing(target: Target) -> Scenario {
	let mut sc = Scenario::new(
		"t",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			parse_text(&synthetic_nextest("core", 3, 4, 2, 1), Flavour::Nextest),
		)],
	);
	sc.target = target;
	sc
}

#[test]
fn a_forgejo_comment_never_says_check_run_or_github() {
	for sc in forgejo_scenarios() {
		for (file, text) in sc.outputs() {
			let lower = text.to_ascii_lowercase();
			for word in ["check run", "github"] {
				assert!(!lower.contains(word), "{}/{file}: {word}", sc.name);
			}
		}
	}
	// And at every step of the budget, which the budget test also covers.
	let mut sc = common::truncation_base_for(Target::Forgejo);
	sc.options.comment_limit = 700;
	assert!(!sc.comment().body.as_str().contains("check run"));
}

#[test]
fn github_words_stay_in_the_wording_and_the_check_code() {
	let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
	for entry in std::fs::read_dir(dir).unwrap() {
		let path = entry.unwrap().path();
		let name = path.file_name().unwrap().to_string_lossy().into_owned();
		let text = std::fs::read_to_string(&path).unwrap();
		let code = text.split("#[cfg(test)]").next().unwrap();
		let hits = code
			.lines()
			.filter(|l| !l.trim_start().starts_with("//"))
			.filter(|l| l.trim() != "GitHub,")
			.map(|l| l.replace("Target::GitHub", ""))
			.any(|l| l.contains("check run") || l.contains("GitHub"));
		let allowed = matches!(name.as_str(), "wording.rs" | "check.rs");
		assert!(
			!hits || allowed,
			"{name}: GitHub's words outside wording.rs and check.rs"
		);
	}
}

#[test]
fn github_still_says_check_runs() {
	let mut sc = common::truncation_base();
	sc.options.comment_limit = 700;
	let body = sc.comment().body.to_string();
	assert!(
		body.contains("The check runs hold the full results."),
		"{body}"
	);
}

#[test]
fn forgejo_has_no_check_runs() {
	let sc = failing(Target::Forgejo);
	assert!(sc.with_ctx(|i| pr_results_render::check(i, 0)).is_none());
	assert!(sc
		.with_ctx(|i| pr_results_render::check_with_paths(i, 0, &|_| true))
		.is_none());
	assert!(failing(Target::GitHub)
		.with_ctx(|i| pr_results_render::check(i, 0))
		.is_some());
}

#[test]
fn failures_show_their_location_on_forgejo_and_github_is_unchanged() {
	let forgejo = failing(Target::Forgejo).comment().body.to_string();
	assert!(
		forgejo.contains("at `crates/core/src/m0.rs:10`"),
		"{forgejo}"
	);
	let github = failing(Target::GitHub).comment().body.to_string();
	assert!(!github.contains("at `"), "{github}");
}

#[test]
fn a_location_with_no_line_is_the_path_alone_and_one_with_no_path_is_left_out() {
	let xml = "<testsuites><testsuite name=\"s\">\
		<testcase classname=\"c\" name=\"a\" file=\"src/a.rs\"><failure message=\"m\"/></testcase>\
		<testcase classname=\"c\" name=\"b\" file=\"../escape.rs\" line=\"4\"><failure message=\"m\"/></testcase>\
		<testcase classname=\"c\" name=\"c\" file=\"src/c.rs\" line=\"0\"><failure message=\"m\"/></testcase>\
		</testsuite></testsuites>";
	let mut sc = Scenario::new(
		"t",
		"Example checks",
		vec![section("S", false, parse_text(xml, Flavour::Generic))],
	);
	sc.target = Target::Forgejo;
	let body = sc.comment().body.to_string();
	assert!(body.contains("at `src/a.rs`"), "{body}");
	assert!(body.contains("at `src/c.rs`"), "{body}");
	assert!(!body.contains("escape.rs"), "{body}");
}

#[test]
fn the_footer_links_the_commit_and_the_run_and_has_no_artefacts_on_forgejo() {
	let body = failing(Target::Forgejo).comment().body.to_string();
	let footer = body.lines().last().unwrap();
	assert!(footer.contains("](https://forgejo.example.invalid/example/app/actions/runs/412)"));
	assert!(!footer.contains("artefacts") && !footer.contains("#artifacts"));
	let github = failing(Target::GitHub).comment().body.to_string();
	let footer = github.lines().last().unwrap();
	assert!(footer.contains("actions/runs/19283746#artifacts"));
}

#[test]
fn without_a_run_number_the_forgejo_footer_links_the_run_id() {
	let sc = failing(Target::Forgejo);
	let mut ctx = common::ctx_for(Target::Forgejo);
	ctx.run_number = None;
	let sections = [pr_results_render::Section {
		title: "Rust tests",
		check_name: "x",
		required: false,
		suites: &sc.sections[0].suites,
		inputs_skipped: 0,
	}];
	let options = pr_results_render::Options::default();
	let input = pr_results_render::Input {
		ctx: &ctx,
		title: "Example checks",
		sections: &sections,
		options: &options,
	};
	let body = pr_results_render::comment(&input).body.to_string();
	assert!(body.contains("actions/runs/19283746)"), "{body}");
}

#[test]
fn statuses_say_pass_or_fail_and_a_neutral_section_passes() {
	let sc = Scenario::new(
		"t",
		"Example checks",
		vec![
			section(
				"A",
				false,
				parse_text(&synthetic_nextest("a", 1, 1, 2, 0), Flavour::Nextest),
			),
			section(
				"B",
				false,
				parse_text(&synthetic_nextest("b", 0, 0, 1_978, 0), Flavour::Nextest),
			),
			section("C", false, Vec::new()),
			section("D", true, Vec::new()),
		],
	);
	let status = |i| sc.with_ctx(|input| pr_results_render::status(input, i));
	let a = status(0).unwrap();
	assert_eq!(
		(a.state, a.description.as_str()),
		(StatusState::Failure, "1 failed, 2 passed")
	);
	let b = status(1).unwrap();
	assert_eq!(b.state, StatusState::Success);
	// Plain text: no thin space between the digit groups.
	assert_eq!(b.description.as_str(), "1978 passed");
	let c = status(2).unwrap();
	assert_eq!(
		(c.state, c.description.as_str()),
		(StatusState::Success, "No results were uploaded")
	);
	assert_eq!(status(3).unwrap().state, StatusState::Failure);
	assert!(status(4).is_none());
	for i in 0..4 {
		assert!(status(i).unwrap().description.len_chars() <= STATUS_DESCRIPTION_CHARS);
	}
}

#[test]
fn the_status_description_is_the_check_title() {
	let sc = failing(Target::Forgejo);
	let status = sc
		.with_ctx(|i| pr_results_render::status(i, 0))
		.unwrap()
		.description;
	let github = failing(Target::GitHub);
	let title = github
		.with_ctx(|i| pr_results_render::check(i, 0))
		.unwrap()
		.title;
	assert_eq!(status.as_str(), title.as_str());
}

#[test]
fn a_title_that_forgejo_would_link_is_not_written_as_text() {
	for title in [
		"Desk checks",
		"Rust tests: nightly",
		"v1.2 build",
		"build 123456",
		"Planboard",
		"a: b",
	] {
		assert!(forgejo_inert_title(title), "{title}");
	}
	for title in [
		"$5",
		"cost $",
		"fix !12",
		"ci!1",
		"build deadbeef",
		"1234567",
		"abcdef0",
		"x:smile",
		":smile:",
		"a:b_c",
		"a:+1",
		"see http://example.invalid",
		"HTTPS://x",
		"www.example.invalid",
		"ftp://x",
	] {
		assert!(!forgejo_inert_title(title), "{title}");
	}
}

#[test]
fn a_risky_title_goes_in_a_code_span_on_forgejo_and_stays_text_on_github() {
	let comment = |target, title: &str| {
		let mut sc = failing(target);
		sc.title = title.to_owned();
		sc.comment().body.to_string()
	};
	let heading = |body: String| body.lines().nth(1).unwrap().to_owned();
	assert_eq!(
		heading(comment(Target::Forgejo, "x:wink")),
		"### `x:wink`: 3 failures in 1 suite"
	);
	assert_eq!(
		heading(comment(Target::Forgejo, "Desk checks")),
		"### Desk checks: 3 failures in 1 suite"
	);
	// GitHub's heading is unchanged.
	assert!(heading(comment(Target::GitHub, "x:wink")).starts_with("### x:wink: "));
	assert!(heading(comment(Target::GitHub, "deadbeef0")).starts_with("### deadbeef0: "));
}

#[test]
fn every_forgejo_golden_is_inert_for_forgejo() {
	let mut seen = 0;
	for sc in scenarios() {
		if sc.target == Target::Forgejo {
			seen += 1;
			assert_inert_all(&sc);
		}
	}
	assert_eq!(seen, 4);
}

#[test]
fn the_forgejo_rules_catch_what_forgejo_links() {
	for line in [
		"see deadbeef0 here",
		"fixes !12",
		"costs $5",
		"a :smile: b",
		"a \\$5",
	] {
		let md = format!(
			"<!-- pr-results:1 key=ci sha={} run=1 -->\n{line}\n",
			common::SHA
		);
		let caught = std::panic::catch_unwind(|| {
			inert::assert_inert_for(Target::Forgejo, "t", &md, FORGEJO_LINK_PREFIX);
		});
		assert!(caught.is_err(), "{line}");
		// GitHub does not care about any of them.
		inert::assert_inert_for(Target::GitHub, "t", &md, FORGEJO_LINK_PREFIX);
	}
	assert!(FORGEJO_HOSTILE_TITLE.contains("deadbeef0"));
}

fn long_comment(target: Target) -> pr_results_render::Comment {
	let mut sc = Scenario::new(
		"t",
		"Example checks",
		vec![section(
			"Rust tests",
			false,
			parse_text(&synthetic_nextest("big", 40, 20, 5, 4), Flavour::Nextest),
		)],
	);
	sc.target = target;
	sc.comment()
}

#[test]
fn a_summary_within_the_cap_is_the_comment_without_its_marker() {
	let c = long_comment(Target::Forgejo);
	let summary = job_summary(&c);
	assert!(!summary.as_str().contains("<!--"));
	assert_eq!(summary, c.body.after_first_line());
	assert_eq!(job_summary_within(&c, usize::MAX), summary);
	assert!(summary.as_str().len() < JOB_SUMMARY_CAP_BYTES);
}

#[test]
fn a_summary_over_the_cap_is_cut_whole_closed_and_says_so() {
	for target in [Target::GitHub, Target::Forgejo] {
		let c = long_comment(target);
		let whole = job_summary(&c);
		let mut cut_somewhere_open = false;
		for cap in (300..whole.as_str().len()).step_by(613) {
			let summary = job_summary_within(&c, cap);
			let text = summary.as_str();
			assert!(text.len() <= cap, "{cap}: {}", text.len());
			assert!(
				text.ends_with("left out to fit the size limit for a job summary.\n"),
				"{cap}"
			);
			assert!(text.contains("The last "));
			assert_eq!(text.lines().next(), whole.as_str().lines().next(), "{cap}");
			let opens = text.matches("<details").count();
			let closes = text.matches("</details>").count();
			assert_eq!(opens, closes, "{cap}");
			inert::assert_inert_for(
				target,
				&format!("cap {cap}"),
				text,
				common::link_prefix(target),
			);
			cut_somewhere_open |= text.contains("\n</details>\n\nThe last");
		}
		assert!(cut_somewhere_open, "no cut landed inside a failure block");
	}
}

#[test]
fn a_cut_inside_a_code_block_closes_the_fence() {
	let c = long_comment(Target::Forgejo);
	let whole = job_summary(&c);
	let mut closed = 0;
	for cap in (300..whole.as_str().len()).step_by(97) {
		let text = job_summary_within(&c, cap).as_str().to_owned();
		if text.contains("\n````\n</details>\n\nThe last") && !text.contains("\n````\n\n</details>")
		{
			closed += 1;
		}
		inert::assert_inert("fence", &text, common::LINK_PREFIX);
	}
	assert!(closed > 0, "no cut landed inside a code block");
}

#[test]
fn a_cap_too_small_for_the_note_gives_nothing() {
	let c = long_comment(Target::Forgejo);
	assert!(job_summary_within(&c, 100).as_str().is_empty());
	assert!(job_summary_within(&c, 0).as_str().is_empty());
}

#[test]
fn the_cap_is_under_the_platforms_one_mebibyte() {
	const { assert!(JOB_SUMMARY_CAP_BYTES < 1024 * 1024) };
	assert_eq!(JOB_SUMMARY_CAP_BYTES, 900 * 1024);
}
