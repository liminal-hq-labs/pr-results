// Parser tests over the per-producer fixtures, and the hostile-input and robustness tests.
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

use common::{all_fixtures, fixture, Rng};
use pr_results_core::{parse_junit, Flavour, Limits, ParseError, Report, Safe, Status, Suite};

fn parse(path: &str, flavour: Flavour) -> Report {
	parse_junit(&fixture(path), flavour, &Limits::default())
		.unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn counts(s: &Suite) -> (usize, usize, usize, usize, usize) {
	(
		s.count(Status::Passed),
		s.count(Status::Failed),
		s.count(Status::Errored),
		s.count(Status::Skipped),
		s.count(Status::Flaky),
	)
}

fn xml(body: &str) -> Vec<u8> {
	format!("<?xml version=\"1.0\"?>\n{body}").into_bytes()
}

#[test]
fn nextest_pass() {
	let r = parse("nextest/pass.xml", Flavour::Auto);
	assert_eq!(r.suites.len(), 2);
	assert!(r.suites.iter().all(|s| s.flavour == Flavour::Nextest));
	assert_eq!(r.suites[0].name, "pr-results-core");
	assert_eq!(counts(&r.suites[0]), (3, 0, 0, 0, 0));
	assert_eq!(counts(&r.suites[1]), (0, 0, 0, 1, 0));
	assert_eq!(r.suites[0].cases[1].time_ms, 4);
	assert_eq!(
		r.suites[0].time_ms, 8,
		"no time attribute on the suite, so the cases are summed"
	);
}

#[test]
fn nextest_failure_takes_its_location_from_the_panic() {
	let r = parse("nextest/fail.xml", Flavour::Nextest);
	let case = &r.suites[0].cases[1];
	assert_eq!(case.status, Status::Failed);
	assert_eq!(case.time_ms, 1904);
	assert_eq!(
		case.file.as_ref().unwrap().as_str(),
		"crates/pr-results-core/src/junit.rs"
	);
	assert_eq!(case.line, Some(341));
	assert!(case
		.message
		.as_ref()
		.unwrap()
		.as_str()
		.contains("left == right"));
	assert!(case
		.detail
		.as_ref()
		.unwrap()
		.as_str()
		.contains("stack backtrace"));
	assert!(case.system_out.is_some() && case.system_err.is_some());
}

#[test]
fn nextest_flaky_is_a_pass_after_failed_attempts() {
	let r = parse("nextest/flaky.xml", Flavour::Auto);
	let case = &r.suites[0].cases[0];
	assert_eq!(case.status, Status::Flaky);
	assert_eq!(case.attempts.len(), 2);
	assert!(case.attempts.iter().all(|a| a.status == Status::Failed));
	assert!(case.attempts[0]
		.detail
		.as_ref()
		.unwrap()
		.as_str()
		.contains("timed out"));
	assert_eq!(counts(&r.suites[0]), (0, 0, 0, 0, 1));
}

#[test]
fn nextest_retries_exhausted_stays_failed() {
	let r = parse("nextest/retries-exhausted.xml", Flavour::Auto);
	let s = &r.suites[0];
	assert_eq!(counts(s), (0, 1, 1, 1, 0));
	assert_eq!(s.cases[0].attempts.len(), 2);
	assert_eq!(
		s.cases[0].file.as_ref().unwrap().as_str(),
		"crates/pr-results-core/src/parse.rs"
	);
	assert_eq!(s.cases[0].line, Some(17));
	assert_eq!(s.cases[1].status, Status::Errored);
	assert_eq!(s.cases[2].status, Status::Skipped);
	assert_eq!(s.cases[2].message.as_ref().unwrap(), &"ignored");
}

#[test]
fn nextest_panic_locations_in_both_formats_and_never_outside_the_repository() {
	let r = parse("nextest/panic-location.xml", Flavour::Nextest);
	let cases = &r.suites[0].cases;
	assert_eq!(
		(cases[0].file.as_ref().unwrap().as_str(), cases[0].line),
		("src/tool.rs", Some(12))
	);
	assert_eq!(
		(cases[1].file.as_ref().unwrap().as_str(), cases[1].line),
		("src/old.rs", Some(7))
	);
	assert!(
		cases[2].file.is_none() && cases[2].line.is_none(),
		"a path with .. is refused"
	);
}

#[test]
fn steps_flavour() {
	let r = parse("steps/pass.xml", Flavour::Steps);
	assert_eq!(r.suites.len(), 2);
	assert!(r.suites.iter().all(|s| s.flavour == Flavour::Steps));
	assert_eq!(r.suites[0].name, "Host tests");
	assert_eq!(r.suites[0].time_ms, 6250);
	assert_eq!(r.suites[0].cases[1].name, "shell crates, host tests");
	assert_eq!(r.suites[0].cases[1].classname, "tools/check.sh:Host tests");
	assert_eq!(r.suites[0].cases[1].time_ms, 5408);

	let r = parse("steps/step-fail.xml", Flavour::Steps);
	let s = &r.suites[0];
	assert_eq!(counts(s), (1, 1, 0, 2, 0));
	assert_eq!(s.cases[1].message.as_ref().unwrap(), &"exit status 101");
	assert!(s.cases[1]
		.detail
		.as_ref()
		.unwrap()
		.as_str()
		.contains("41 passed; 1 failed"));
	assert_eq!(
		s.cases[2].message.as_ref().unwrap(),
		&"not run: an earlier step failed"
	);
	assert!(
		s.cases.iter().all(|c| c.file.is_none()),
		"steps have no source locations"
	);
}

#[test]
fn step_suites_merge_across_group_jobs() {
	let mut a = parse("steps/groups-split-a.xml", Flavour::Steps);
	let b = parse("steps/groups-split-b.xml", Flavour::Steps);
	a.suites.extend(b.suites);
	a.merge_same_named();
	assert_eq!(a.suites.len(), 2);
	assert_eq!(a.suites[0].name, "Host tests");
	assert_eq!(a.suites[0].cases.len(), 3);
	assert_eq!(a.suites[0].time_ms, 7150);
	assert_eq!(a.suites[1].name, "Targets");
}

#[test]
fn vitest_flavour() {
	let r = parse("vitest/pass.xml", Flavour::Auto);
	assert!(r.suites.iter().all(|s| s.flavour == Flavour::Vitest));
	assert_eq!(r.suites[0].name, "src/lib/format.test.ts");
	assert_eq!(
		r.suites[0].cases[0].name,
		"formatDuration > writes minutes and seconds"
	);
	assert_eq!(
		r.suites[0].cases[0].file.as_ref().unwrap().as_str(),
		"src/lib/format.test.ts"
	);

	let r = parse("vitest/fail-with-stack.xml", Flavour::Vitest);
	let case = &r.suites[0].cases[0];
	assert_eq!(case.status, Status::Failed);
	assert!(case
		.message
		.as_ref()
		.unwrap()
		.as_str()
		.contains("'1 m 47 s'"));
	assert!(case
		.detail
		.as_ref()
		.unwrap()
		.as_str()
		.contains("format.test.ts:18:36"));

	let r = parse("vitest/describe-nesting.xml", Flavour::Vitest);
	assert_eq!(counts(&r.suites[0]), (1, 0, 0, 2, 0));
	assert_eq!(
		r.suites[0].cases[0].name,
		"Panel > layout > wraps long titles"
	);
}

#[test]
fn bun_flavour() {
	let r = parse("bun/pass.xml", Flavour::Auto);
	assert_eq!(r.suites[0].flavour, Flavour::Bun);
	assert_eq!(
		r.suites[0].cases[0].file.as_ref().unwrap().as_str(),
		"test/add.test.ts"
	);
	assert_eq!(r.suites[0].cases[0].line, Some(3));

	let r = parse("bun/fail-file-line.xml", Flavour::Bun);
	let case = &r.suites[0].cases[1];
	assert_eq!(
		(case.status, case.line),
		(Status::Failed, Some(11)),
		"the failing line in the stack wins over the line the test starts on (10)"
	);
	assert!(
		case.message
			.as_ref()
			.unwrap()
			.as_str()
			.contains("expect(received)"),
		"bun writes no message attribute, so the first line of the text is the message"
	);
	assert!(case
		.detail
		.as_ref()
		.unwrap()
		.as_str()
		.contains("Expected: -1"));

	let r = parse("bun/nested.xml", Flavour::Bun);
	assert_eq!(
		r.suites.len(),
		1,
		"nested suites flatten into the file's suite"
	);
	assert_eq!(r.suites[0].cases.len(), 3);
	assert_eq!(r.suites[0].cases[1].classname, "List pop");
	assert_eq!(r.suites[0].cases[2].status, Status::Skipped);
	assert_eq!(
		r.suites[0].time_ms, 30,
		"only the outer suite's time counts"
	);
}

#[test]
fn jest_flavour() {
	let r = parse("jest-junit/default.xml", Flavour::Auto);
	assert_eq!(r.suites.len(), 2);
	assert!(r.suites.iter().all(|s| s.flavour == Flavour::Jest));
	assert_eq!(counts(&r.suites[1]), (0, 1, 0, 1, 0));
	let case = &r.suites[1].cases[0];
	assert!(case
		.message
		.as_ref()
		.unwrap()
		.as_str()
		.contains("toBe(expected)"));
	assert!(case
		.detail
		.as_ref()
		.unwrap()
		.as_str()
		.contains("Received: 1"));
	assert!(case
		.detail
		.as_ref()
		.unwrap()
		.as_str()
		.contains("Object.<anonymous>"));
}

#[test]
fn playwright_flavour_and_shards() {
	let mut a = parse("playwright/shard-1-of-2.xml", Flavour::Playwright);
	let b = parse("playwright/shard-2-of-2.xml", Flavour::Playwright);
	assert_eq!(
		a.suites[0].cases[0].file.as_ref().unwrap().as_str(),
		"login.spec.ts"
	);
	assert_eq!(b.suites[0].cases[1].status, Status::Failed);
	assert!(b.suites[0].cases[1]
		.detail
		.as_ref()
		.unwrap()
		.as_str()
		.contains("Received string"));
	a.suites.extend(b.suites);
	a.merge_same_named();
	assert_eq!(a.suites.len(), 2, "different files stay separate suites");

	let r = parse("playwright/attachments.xml", Flavour::Auto);
	assert_eq!(
		r.suites[0].flavour,
		Flavour::Playwright,
		"detected from the attachment lines"
	);
	assert!(r.suites[0].cases[0]
		.system_out
		.as_ref()
		.unwrap()
		.as_str()
		.contains("[[ATTACHMENT|"));
}

#[test]
fn generic_shapes() {
	let r = parse("generic/nested-suites.xml", Flavour::Auto);
	assert_eq!(r.suites.len(), 1);
	assert_eq!(r.suites[0].flavour, Flavour::Generic);
	assert_eq!(counts(&r.suites[0]), (1, 1, 1, 0, 0));
	assert_eq!(r.suites[0].time_ms, 3500);
	assert_eq!(
		r.suites[0].cases[2].detail.as_ref().unwrap().as_str(),
		"trace"
	);

	let r = parse("generic/bare-testsuite.xml", Flavour::Auto);
	assert_eq!(
		(r.suites.len(), r.suites[0].cases.len(), r.suites[0].time_ms),
		(1, 1, 250)
	);

	let r = parse("generic/empty.xml", Flavour::Auto);
	assert!(r.suites.is_empty());
}

#[test]
fn parsing_is_deterministic() {
	for (name, bytes) in all_fixtures() {
		let a = parse_junit(&bytes, Flavour::Auto, &Limits::default());
		let b = parse_junit(&bytes, Flavour::Auto, &Limits::default());
		assert_eq!(format!("{a:?}"), format!("{b:?}"), "{name}");
	}
}

#[test]
fn hostile_fixture_parses_and_renders_inert() {
	let r = parse("hostile/strings.xml", Flavour::Auto);
	let s = &r.suites[0];
	let first = &s.cases[0];
	let rendered = [
		Safe::text(&s.name),
		Safe::text(&first.classname),
		Safe::text(&first.name),
		Safe::text(first.message.as_ref().unwrap()),
		Safe::text(&s.cases[1].classname),
		Safe::text(&s.cases[1].name),
	];
	for out in &rendered {
		let o = out.as_str();
		assert!(
			!o.contains(['<', '>', '@', '\n', '\u{1b}', '\u{202e}', '\u{200b}', '\u{2066}']),
			"{o}"
		);
		assert!(
			!o.contains("::") && !o.contains("](") && !o.contains("://"),
			"{o}"
		);
	}
	assert!(
		s.cases[1].file.is_none(),
		"a path escaping the repository is dropped"
	);
	let block = Safe::code_block(first.detail.as_ref().unwrap(), 1000);
	assert!(block.as_str().starts_with("````text\n"));
	let log = Safe::log_line(first.message.as_ref().unwrap(), 200);
	assert!(!log.as_str().contains("::") && !log.as_str().contains('\n'));
	let data = Safe::workflow_data(first.message.as_ref().unwrap(), 200);
	assert!(!data.as_str().contains('\n'));
}

// Hostile and malformed input.

#[test]
fn every_parser_rejects_oversize_input() {
	for (name, bytes) in all_fixtures() {
		for flavour in [
			Flavour::Auto,
			Flavour::Nextest,
			Flavour::Steps,
			Flavour::Vitest,
			Flavour::Jest,
			Flavour::Bun,
			Flavour::Playwright,
			Flavour::Generic,
		] {
			let tight = Limits {
				max_bytes: bytes.len() - 1,
				..Limits::default()
			};
			assert_eq!(
				parse_junit(&bytes, flavour, &tight).unwrap_err(),
				ParseError::TooLarge {
					len: bytes.len(),
					limit: bytes.len() - 1
				},
				"{name} as {flavour:?}"
			);
			let exact = Limits {
				max_bytes: bytes.len(),
				..Limits::default()
			};
			assert!(
				parse_junit(&bytes, flavour, &exact).is_ok(),
				"{name} as {flavour:?}"
			);
		}
	}
	// A real over-limit input, not just a tightened limit.
	let big = vec![b' '; Limits::default().max_bytes + 1];
	assert!(matches!(
		parse_junit(&big, Flavour::Auto, &Limits::default()),
		Err(ParseError::TooLarge { .. })
	));
}

#[test]
fn every_parser_rejects_truncated_input() {
	for (name, bytes) in all_fixtures() {
		let end = bytes
			.iter()
			.rposition(|b| !b.is_ascii_whitespace())
			.unwrap() + 1;
		for flavour in [
			Flavour::Auto,
			Flavour::Nextest,
			Flavour::Steps,
			Flavour::Vitest,
			Flavour::Jest,
			Flavour::Bun,
			Flavour::Playwright,
			Flavour::Generic,
		] {
			// The whole document is fine, and so is the whole document without its final newline.
			assert!(
				parse_junit(&bytes[..end], flavour, &Limits::default()).is_ok(),
				"{name}"
			);
			if flavour != Flavour::Auto && flavour != Flavour::Generic {
				continue;
			}
			for cut in 0..end {
				let result = parse_junit(&bytes[..cut], flavour, &Limits::default());
				assert!(result.is_err(), "{name} cut at {cut} of {end} was accepted");
			}
		}
	}
}

#[test]
fn mutated_fixtures_never_panic() {
	let mut rng = Rng(0x2545_f491_4f6c_dd1d);
	let fixtures = all_fixtures();
	for round in 0..6000 {
		let (_, original) = &fixtures[round % fixtures.len()];
		let mut bytes = original.clone();
		for _ in 0..1 + rng.below(4) {
			let at = rng.below(bytes.len());
			match rng.below(4) {
				0 => bytes[at] = rng.next() as u8,
				1 => {
					bytes.remove(at);
				}
				2 => bytes.insert(at, b"<>&;\"'/!-[]\0"[rng.below(12)]),
				_ => {
					let to = (at + rng.below(200)).min(bytes.len());
					bytes.drain(at..to);
				}
			}
			if bytes.is_empty() {
				bytes.push(b'<');
			}
		}
		// Either outcome is fine; reaching the end without a panic is the test.
		let _ = parse_junit(&bytes, Flavour::Auto, &Limits::default()).map_err(|e| e.to_string());
	}
}

#[test]
fn doctype_and_entity_bombs_are_refused() {
	let bomb = xml("<!DOCTYPE lolz [<!ENTITY lol \"lol\"><!ENTITY lol2 \"&lol;&lol;&lol;&lol;&lol;\">]>\n<testsuites><testsuite name=\"&lol2;\"/></testsuites>");
	assert_eq!(
		parse_junit(&bomb, Flavour::Auto, &Limits::default()).unwrap_err(),
		ParseError::Dtd
	);
	let xxe = xml("<!DOCTYPE x [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]><testsuites/>");
	assert_eq!(
		parse_junit(&xxe, Flavour::Auto, &Limits::default()).unwrap_err(),
		ParseError::Dtd
	);
	let undeclared = xml("<testsuites><testsuite name=\"&xxe;\"/></testsuites>");
	assert_eq!(
		parse_junit(&undeclared, Flavour::Auto, &Limits::default()).unwrap_err(),
		ParseError::BadEntity
	);
	let in_text = xml("<testsuites><testsuite name=\"s\"><testcase name=\"a\" classname=\"b\"><failure>&xxe;</failure></testcase></testsuite></testsuites>");
	assert_eq!(
		parse_junit(&in_text, Flavour::Auto, &Limits::default()).unwrap_err(),
		ParseError::BadEntity
	);
	let nul_ref = xml("<testsuites><testsuite name=\"&#0;\"/></testsuites>");
	assert!(parse_junit(&nul_ref, Flavour::Auto, &Limits::default()).is_err());
}

#[test]
fn nesting_is_limited() {
	let build = |levels: usize| {
		let mut s =
			String::from("<testsuites><testsuite name=\"s\"><testcase name=\"a\" classname=\"b\">");
		for _ in 0..levels {
			s.push_str("<x>");
		}
		for _ in 0..levels {
			s.push_str("</x>");
		}
		s.push_str("</testcase></testsuite></testsuites>");
		xml(&s)
	};
	assert!(parse_junit(&build(60), Flavour::Auto, &Limits::default()).is_ok());
	assert_eq!(
		parse_junit(&build(64), Flavour::Auto, &Limits::default()).unwrap_err(),
		ParseError::TooDeep { limit: 64 }
	);
	let deep = xml(&"<a>".repeat(100_000));
	assert!(matches!(
		parse_junit(&deep, Flavour::Auto, &Limits::default()),
		Err(ParseError::UnexpectedRoot)
	));
	let deep_suites = xml(&format!(
		"<testsuites>{}",
		"<testsuite name=\"s\">".repeat(100_000)
	));
	assert!(matches!(
		parse_junit(&deep_suites, Flavour::Auto, &Limits::default()),
		Err(ParseError::TooDeep { .. })
	));
}

#[test]
fn case_count_is_limited() {
	let mut s = String::from("<testsuites><testsuite name=\"s\">");
	for i in 0..100_000 {
		s.push_str(&format!(
			"<testcase classname=\"c\" name=\"t{i}\" time=\"0.001\"/>"
		));
	}
	s.push_str("</testsuite></testsuites>");
	let big = xml(&s);
	let r = parse_junit(&big, Flavour::Auto, &Limits::default()).unwrap();
	assert_eq!(r.suites[0].cases.len(), 100_000);
	let tight = Limits {
		max_cases: 99_999,
		..Limits::default()
	};
	assert_eq!(
		parse_junit(&big, Flavour::Auto, &tight).unwrap_err(),
		ParseError::TooManyCases { limit: 99_999 }
	);
}

#[test]
fn long_text_is_capped_while_streaming() {
	let name = "N".repeat(10 * 1024 * 1024);
	let body = "B".repeat(2 * 1024 * 1024);
	let doc = xml(&format!(
		"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"{name}\"><failure message=\"{name}\">{body}</failure></testcase></testsuite></testsuites>"
	));
	let r = parse_junit(&doc, Flavour::Auto, &Limits::default()).unwrap();
	let case = &r.suites[0].cases[0];
	assert_eq!(case.name.as_str().len(), 1024);
	assert_eq!(case.name.omitted(), 10 * 1024 * 1024 - 1024);
	assert_eq!(case.message.as_ref().unwrap().as_str().len(), 1024);
	assert_eq!(case.detail.as_ref().unwrap().as_str().len(), 16 * 1024);
	assert_eq!(
		case.detail.as_ref().unwrap().omitted(),
		2 * 1024 * 1024 - 16 * 1024
	);
	let shown = Safe::text(&case.name);
	assert!(shown.as_str().contains("more characters omitted"));
	assert!(shown.len_chars() < 1100);
}

#[test]
fn bad_structure_has_a_clear_error() {
	let l = Limits::default();
	let err = |body: &str| parse_junit(&xml(body), Flavour::Auto, &l).unwrap_err();
	assert_eq!(
		parse_junit(b"", Flavour::Auto, &l).unwrap_err(),
		ParseError::Empty
	);
	assert_eq!(
		parse_junit(b" \n\t ", Flavour::Auto, &l).unwrap_err(),
		ParseError::Empty
	);
	assert_eq!(err("<html><body/></html>"), ParseError::UnexpectedRoot);
	assert_eq!(
		err("<testsuites/><testsuites/>"),
		ParseError::Structure("more than one root element")
	);
	assert_eq!(
		err("<testsuites/>stray"),
		ParseError::Structure("text outside the root element")
	);
	assert_eq!(
		err("<testsuites><testcase name=\"a\"/></testsuites>"),
		ParseError::Structure("a test case outside a suite")
	);
	assert!(matches!(
		err("<testsuites><testsuite></testsuites>"),
		ParseError::Malformed { .. }
	));
	assert_eq!(
		err("<testsuites><testsuite name=\"a\">"),
		ParseError::Truncated
	);
	assert!(
		matches!(
			err("<testsuites a=\"1\" a=\"2\"/>"),
			ParseError::Malformed { .. }
		),
		"duplicate attributes"
	);
	let many: String = (0..300).map(|i| format!(" a{i}=\"x\"")).collect();
	assert_eq!(
		err(&format!(
			"<testsuites><testsuite name=\"s\"{many}/></testsuites>"
		)),
		ParseError::TooManyAttributes { limit: 256 }
	);
	let latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><testsuites/>";
	assert_eq!(
		parse_junit(latin1, Flavour::Auto, &l).unwrap_err(),
		ParseError::UnsupportedEncoding
	);
	assert!(matches!(
		parse_junit(b"<testsuites>\0</testsuites>", Flavour::Auto, &l),
		Err(ParseError::Malformed { offset: 12 })
	));
	assert!(
		parse_junit(&[0xff, 0xfe, b'<', 0, b't', 0], Flavour::Auto, &l).is_err(),
		"UTF-16 is not accepted"
	);
}

#[test]
fn error_messages_quote_nothing_from_the_input() {
	let l = Limits::default();
	let evil =
		xml("<testsuites><testsuite name=\"::error::pwned\"><oops></testsuite></testsuites>");
	let message = parse_junit(&evil, Flavour::Auto, &l)
		.unwrap_err()
		.to_string();
	assert!(
		!message.contains("pwned") && !message.contains("oops") && !message.contains("::"),
		"{message}"
	);
}

#[test]
fn invalid_utf8_is_replaced_and_a_bom_is_ignored() {
	let mut doc = vec![0xef, 0xbb, 0xbf];
	doc.extend_from_slice(b"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"bad\xff\xfebytes\"/></testsuite></testsuites>");
	let r = parse_junit(&doc, Flavour::Auto, &Limits::default()).unwrap();
	assert_eq!(
		r.suites[0].cases[0].name.as_str(),
		"bad\u{fffd}\u{fffd}bytes"
	);
}

#[test]
fn odd_numbers_do_not_fail_the_file() {
	let doc = xml(
		"<testsuites><testsuite name=\"s\" time=\"1e3\"><testcase classname=\"c\" name=\"a\" time=\"1,5\" line=\"-3\"/><testcase classname=\"c\" name=\"b\" time=\"0.0005\" line=\"99999999999\"/><testcase classname=\"c\" name=\"c\" time=\"99999999999999999999999.9999\"/></testsuite></testsuites>",
	);
	let r = parse_junit(&doc, Flavour::Auto, &Limits::default()).unwrap();
	let s = &r.suites[0];
	assert_eq!(s.time_ms, 0);
	assert_eq!(s.cases[0].time_ms, 0);
	assert_eq!(s.cases[0].line, None);
	assert_eq!(s.cases[1].time_ms, 1, "0.0005 s rounds to 1 ms");
	assert_eq!(s.cases[1].line, None);
	assert_eq!(s.cases[2].time_ms, u64::MAX);
}

#[test]
fn flavour_names_round_trip() {
	for f in [
		Flavour::Auto,
		Flavour::Generic,
		Flavour::Nextest,
		Flavour::Steps,
		Flavour::Vitest,
		Flavour::Jest,
		Flavour::Bun,
		Flavour::Playwright,
	] {
		assert_eq!(Flavour::from_name(f.name()), Some(f));
	}
	assert_eq!(Flavour::from_name("junit"), None);
}
