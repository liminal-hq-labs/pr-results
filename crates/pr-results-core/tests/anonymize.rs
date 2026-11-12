// The anonymizer: deterministic, no word of the input left, and the structure the readers need.
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

use std::collections::BTreeSet;

use common::all_fixtures;
use pr_results_core::{anonymize_junit, parse_junit, Flavour, Limits, ParseError, Report};
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

/// Words the anonymizer keeps on purpose: the reporters' and JavaScript's own vocabulary, and the
/// values that say which tool wrote a file. Numbers of up to four digits are kept as well.
const VOCABULARY: &[&str] = &[
	"at",
	"async",
	"new",
	"node",
	"internal",
	"file",
	"anonymous",
	"node_modules",
	"ATTACHMENT",
	"Retry",
	"panicked",
	"vitest",
	"tests",
	"bun",
	"test",
	"jest",
	"nextest",
	"run",
	"Error",
	"AssertionError",
	"TypeError",
	"RangeError",
	"ReferenceError",
	"SyntaxError",
	"EvalError",
	"URIError",
	"FAILURE",
	"ERROR",
	"failure",
	"chromium",
	"firefox",
	"webkit",
	// The fixed timestamp.
	"2026",
	"01",
	"00",
	"000Z",
	"01T00",
	"000",
];

/// Every attribute value and piece of text in a document, in order.
fn values(xml: &str) -> Vec<String> {
	let mut reader = Reader::from_str(xml);
	let mut out = Vec::new();
	loop {
		match reader.read_event().unwrap() {
			Event::Eof => break,
			Event::Start(e) | Event::Empty(e) => {
				for a in e.attributes() {
					let a = a.unwrap();
					out.push(
						a.normalized_value(XmlVersion::Implicit1_0)
							.unwrap()
							.into_owned(),
					);
				}
			}
			Event::Text(t) => out.push(t.xml_content(XmlVersion::Implicit1_0).into_owned()),
			Event::CData(c) => out.push(c.xml_content(XmlVersion::Implicit1_0).into_owned()),
			Event::GeneralRef(r) => {
				let c = r.resolve_char_ref().unwrap().unwrap_or_else(|| match &*r {
					"lt" => '<',
					"gt" => '>',
					"amp" => '&',
					"apos" => '\'',
					_ => '"',
				});
				out.push(c.to_string());
			}
			_ => {}
		}
	}
	out
}

fn words(text: &str) -> BTreeSet<String> {
	text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
		.filter(|w| !w.is_empty())
		.map(str::to_owned)
		.collect()
}

fn private_words(xml: &str) -> BTreeSet<String> {
	words(&values(xml).join("\n"))
		.into_iter()
		.filter(|w| {
			// A number is not a word: times and counts are kept, and long numbers in prose are
			// replaced.
			!VOCABULARY.contains(&w.as_str()) && !w.bytes().all(|b| b.is_ascii_digit())
		})
		.collect()
}

fn anonymize(bytes: &[u8]) -> String {
	anonymize_junit(bytes, "").unwrap()
}

#[test]
fn the_same_input_and_salt_give_the_same_output_and_another_salt_gives_another() {
	for (name, bytes) in all_fixtures() {
		let a = anonymize_junit(&bytes, "salt").unwrap_or_else(|e| panic!("{name}: {e}"));
		let b = anonymize_junit(&bytes, "salt").unwrap();
		assert_eq!(a, b, "{name}");
		let c = anonymize_junit(&bytes, "another").unwrap();
		if !private_words(&String::from_utf8_lossy(&bytes)).is_empty() {
			assert_ne!(a, c, "{name}: the salt changes the tokens");
		}
	}
}

#[test]
fn no_word_of_the_input_is_left() {
	for (name, bytes) in all_fixtures() {
		let input = String::from_utf8(bytes.clone()).unwrap();
		let out = anonymize(&bytes);
		let left = words(&values(&out).join("\n"));
		let leaked: Vec<String> = private_words(&input)
			.into_iter()
			.filter(|w| left.contains(w))
			.collect();
		assert!(leaked.is_empty(), "{name}: {leaked:?}");
	}
}

#[test]
fn nothing_private_survives_in_a_made_up_report() {
	let xml = r#"<?xml version="1.0"?>
<!-- generated for Acme Corp by build 4471 -->
<testsuites name="Acme Billing Suite" id="9f2c" tests="2" failures="1" time="1.5">
  <testsuite name="invoices/InvoiceService.test.ts" hostname="build-box-17.corp.example.invalid" timestamp="2025-03-04T05:06:07" tests="2" failures="1" time="1.5">
    <testcase classname="invoices/InvoiceService.test.ts" name="InvoiceService &gt; refunds Mallory O'Brien's order" time="0.25" file="invoices/InvoiceService.test.ts" line="12"/>
    <testcase classname="invoices/InvoiceService.test.ts" name="sends mail to alice@acme.example.invalid" time="1.25">
      <failure message="expected token @KEY@ to match" type="CustomBillingError">CustomBillingError: expected token @KEY@
    at Object.refund (/home/runner/work/billing/billing/src/invoices/refunds.ts:88:17)
    at file:///home/runner/work/billing/billing/node_modules/.pnpm/stripe@12/node_modules/stripe/lib/index.js:9:3
</failure>
    </testcase>
  </testsuite>
</testsuites>
"#;
	// Built from parts, so that no literal in the repository looks like a live key.
	let key = ["sk", "live", "4eC39HqLyjWDarjtT1zdp7dc"].join("_");
	let xml = xml.replace("@KEY@", &key);
	let out = anonymize(xml.as_bytes());
	for secret in [
		"Acme",
		"Corp",
		"Billing",
		"Suite",
		"InvoiceService",
		"invoices",
		"Mallory",
		"Brien",
		"alice",
		key.as_str(),
		"CustomBillingError",
		"refunds",
		"refund",
		"build",
		"home",
		"runner",
		"stripe",
		"corp",
		"example",
		"invalid",
		"4471",
		"2025",
		"9f2c",
	] {
		assert!(!out.contains(secret), "{secret} survived:\n{out}");
	}
	// The shape the readers use survives.
	assert!(out.contains("node_modules"));
	assert!(
		out.contains("<testsuites name=\"w"),
		"an unknown root name is a token"
	);
	assert!(out.contains(":88:17)"));
	assert!(out.contains("time=\"0.25\""));
	assert!(out.contains("line=\"12\""));
	assert!(out.contains("timestamp=\"2026-01-01T00:00:00.000Z\""));
	assert!(!out.contains("generated for"), "comments are dropped");
	parse_junit(out.as_bytes(), Flavour::Auto, &Limits::default()).unwrap();
}

fn shape(r: &Report) -> Vec<String> {
	let mut out = Vec::new();
	for s in &r.suites {
		out.push(format!(
			"suite {:?} {}ms project={} cases={}",
			s.flavour,
			s.time_ms,
			s.project.is_some(),
			s.cases.len()
		));
		for c in &s.cases {
			out.push(format!(
				"{:?} {}ms line={:?} file_parts={:?} attempts={} omitted={} message={} detail_lines={}",
				c.status,
				c.time_ms,
				c.line,
				c.file.as_ref().map(|f| f.as_str().split('/').count()),
				c.attempts.len(),
				c.attempts_omitted,
				c.message.is_some(),
				c.detail.as_ref().map_or(0, |d| d.as_str().lines().count()),
			));
		}
	}
	out
}

#[test]
fn the_structure_the_readers_use_survives() {
	for (name, bytes) in all_fixtures() {
		// The word `node_modules` is kept, so a stack that skips it still does.
		let before = parse_junit(&bytes, Flavour::Auto, &Limits::default()).unwrap();
		let after_xml = anonymize(&bytes);
		let after = parse_junit(after_xml.as_bytes(), Flavour::Auto, &Limits::default())
			.unwrap_or_else(|e| panic!("{name}: {e}"));
		assert_eq!(shape(&before), shape(&after), "{name}");
	}
}

#[test]
fn one_word_is_one_token_everywhere_so_names_paths_and_frames_still_agree() {
	let bytes = std::fs::read(common::fixtures_dir().join("vitest/real-default.xml")).unwrap();
	let out = anonymize(&bytes);
	let r = parse_junit(out.as_bytes(), Flavour::Auto, &Limits::default()).unwrap();
	let failed = r.suites[0]
		.cases
		.iter()
		.find(|c| c.line.is_some())
		.expect("a located failure");
	// The case's file came from the class name and its line from a frame that names the same path.
	assert_eq!(
		failed.file.as_ref().unwrap().as_str(),
		r.suites[0].name.as_str()
	);
}

#[test]
fn tokens_do_not_collide_for_distinct_words() {
	let mut doc = String::from("<testsuites>");
	let mut seen = BTreeSet::new();
	for i in 0..20_000 {
		doc.push_str(&format!("<testsuite name=\"word{i}x\"/>"));
	}
	doc.push_str("</testsuites>");
	let out = anonymize(doc.as_bytes());
	for v in values(&out).into_iter().filter(|v| !v.trim().is_empty()) {
		assert!(seen.insert(v), "two words share a token");
	}
	assert_eq!(seen.len(), 20_000);
}

#[test]
fn odd_values_are_written_as_well_formed_xml() {
	let xml = "<testsuites name=\"x\"><testsuite name=\"a &quot;b&quot; &lt;c&gt; &amp;\n\t\r d \u{1f600} é 日本語 &#x1;\"><testcase name=\"n\" classname=\"c\"><failure message=\"line one&#10;line two\"><![CDATA[ <![CDATA ]]]]><![CDATA[> \u{7f} ]]></failure></testcase></testsuite></testsuites>";
	let r = anonymize_junit(xml.as_bytes(), "s");
	// `&#x1;` is not a character XML 1.0 allows: it is written as U+FFFD or refused, never as itself.
	let out = r.unwrap();
	assert!(!out.contains('\u{1}'));
	parse_junit(out.as_bytes(), Flavour::Generic, &Limits::default()).unwrap();
	assert!(out.contains("&#10;"), "a newline in an attribute stays one");
	assert!(out.contains('\u{1f600}'), "symbols are not words");
	assert!(!out.contains("日本語"), "letters in any script are words");
	assert!(!out.contains('é'));
}

#[test]
fn bad_input_is_refused_like_the_parser_refuses_it() {
	let cases: [(&str, ParseError); 8] = [
		("", ParseError::Empty),
		("   \n", ParseError::Empty),
		("<!DOCTYPE x><testsuites/>", ParseError::Dtd),
		("<testsuites>&xxe;</testsuites>", ParseError::BadEntity),
		("<testsuites><testsuite>", ParseError::Truncated),
		(
			"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><testsuites/>",
			ParseError::UnsupportedEncoding,
		),
		(
			"<testsuites/><testsuites/>",
			ParseError::Structure("more than one root element"),
		),
		(
			"junk<testsuites/>",
			ParseError::Structure("text outside the root element"),
		),
	];
	for (input, want) in cases {
		assert_eq!(
			anonymize_junit(input.as_bytes(), "").unwrap_err(),
			want,
			"{input:?}"
		);
	}
	assert!(matches!(
		anonymize_junit(b"<a>\0</a>", ""),
		Err(ParseError::Malformed { .. })
	));
	let deep = format!("{}{}", "<a>".repeat(70), "</a>".repeat(70));
	assert!(matches!(
		anonymize_junit(deep.as_bytes(), ""),
		Err(ParseError::TooDeep { .. })
	));
	let big = vec![b' '; Limits::default().max_bytes + 1];
	assert!(matches!(
		anonymize_junit(&big, ""),
		Err(ParseError::TooLarge { .. })
	));
	// A byte-order mark and invalid UTF-8 are handled as the parser handles them.
	let out = anonymize_junit(b"\xef\xbb\xbf<testsuites name=\"\xff\xfe\"/>", "").unwrap();
	assert!(out.contains("<testsuites"));
}
