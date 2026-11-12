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

/// A salt of sixteen bytes, which is what the command line asks for.
const SALT: &str = "0123456789abcdef";

fn anonymize(bytes: &[u8]) -> String {
	anonymize_junit(bytes, SALT).unwrap()
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

/// The names of the elements and attributes in `xml`, in order.
fn names(xml: &str) -> (Vec<String>, Vec<String>) {
	let mut reader = Reader::from_str(xml);
	let (mut elements, mut attributes) = (Vec::new(), Vec::new());
	loop {
		match reader.read_event().unwrap() {
			Event::Eof => break,
			Event::Start(e) | Event::Empty(e) => {
				elements.push(e.name().as_ref().to_owned());
				for a in e.attributes() {
					attributes.push(a.unwrap().key.as_ref().to_owned());
				}
			}
			_ => {}
		}
	}
	(elements, attributes)
}

#[test]
fn tokens_are_a_prefix_and_twelve_hex_digits_that_depend_on_the_salt() {
	let xml = "<testsuites name=\"x\"><testsuite name=\"alpha beta 12345\"/></testsuites>";
	let a = anonymize_junit(xml.as_bytes(), SALT).unwrap();
	let b = anonymize_junit(xml.as_bytes(), "fedcba9876543210").unwrap();
	let token = |t: &str| {
		t.len() == 13
			&& matches!(t.as_bytes()[0], b'w' | b'n' | b'x')
			&& t[1..]
				.bytes()
				.all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
	};
	let a_values = values(&a);
	let tokens: Vec<&str> = a_values
		.iter()
		.flat_map(|v| v.split(' '))
		.filter(|t| !t.trim().is_empty() && *t != "x")
		.collect();
	assert!(tokens.iter().all(|t| token(t)), "{tokens:?}");
	assert_ne!(values(&a), values(&b), "another salt gives other tokens");
	// A word and the same word under another salt share nothing.
	let (sa, sb) = (values(&a), values(&b));
	for (x, y) in sa.iter().zip(&sb) {
		for (p, q) in x.split(' ').zip(y.split(' ')) {
			if token(p) {
				assert_ne!(p, q);
			}
		}
	}
}

#[test]
fn element_attribute_and_namespace_names_outside_the_vocabulary_are_replaced() {
	let xml = r#"<?xml version="1.0"?>
<acme:testsuites xmlns:acme="http://acme.example.invalid/ns" xmlns="http://acme.example.invalid/default" secretattr="v" name="vitest tests">
  <testsuite name="s" secretproject="orders" acme:flag="1">
    <testcase name="t" classname="c" time="0.5"><secretelement acme:x="y">text</secretelement></testcase>
  </testsuite>
</acme:testsuites>"#;
	let out = anonymize(xml.as_bytes());
	for secret in [
		"acme",
		"secretattr",
		"secretproject",
		"secretelement",
		"orders",
		"flag",
	] {
		assert!(!out.contains(secret), "{secret} survived:\n{out}");
	}
	let (elements, attributes) = names(&out);
	// Every name is the readers' vocabulary or a valid XML name made of a token.
	let vocabulary = ["testsuites", "testsuite", "testcase"];
	for e in &elements {
		let local = e.rsplit(':').next().unwrap();
		assert!(
			vocabulary.contains(&e.as_str())
				|| vocabulary.contains(&local)
				|| (e.starts_with('x') && e.len() >= 13),
			"{e}"
		);
	}
	for a in &attributes {
		assert!(
			["name", "classname", "time", "xmlns"].contains(&a.as_str())
				|| a.starts_with('x')
				|| a.starts_with("xmlns:x"),
			"{a}"
		);
	}
	assert!(
		attributes.iter().any(|a| a.starts_with("xmlns:x")),
		"a prefix declaration stays one"
	);
	assert!(attributes.iter().any(|a| a == "xmlns"));
	// The vocabulary the readers use stays, and the document still parses.
	assert!(out.contains("<testcase name="));
	assert!(out.contains("time=\"0.5\""));
}

#[test]
fn a_prefixed_name_and_its_declaration_use_the_same_token() {
	let xml = "<a:testsuites xmlns:a=\"u\"><a:testsuite/></a:testsuites>";
	let out = anonymize(xml.as_bytes());
	let (elements, attributes) = names(&out);
	let prefix = |n: &str| n.split(':').next().unwrap().to_owned();
	assert_eq!(prefix(&elements[0]), prefix(&elements[1]));
	assert_eq!(attributes[0], format!("xmlns:{}", prefix(&elements[0])));
}

#[test]
fn addresses_ports_and_long_numbers_are_replaced_but_counts_and_positions_stay() {
	let xml = r#"<testsuites name="x" tests="12345" failures="3">
<testsuite name="s" tests="12" failures="3" errors="0" skipped="1" time="1234.5678">
<testcase name="connects to 10.0.0.12:5432 and 192.168.1.7" classname="c" line="42" time="0.25">
<failure message="connect ECONNREFUSED 10.0.0.12:5432 (order 987654321)">Error: connect ECONNREFUSED 10.0.0.12:5432 user 424242 port 8080
    at connect (net.js:12:5)
    at Socket.emit (10.0.0.12:5432)
    at fetch (https://api.example.invalid:8443/v1/x.js:30:9)
    at src/a.js:123456:7
    at localhost:3000
thread 'x' panicked at src/lib.rs:341:5:
[chromium] › a.spec.js:24:4 › login 99
</failure></testcase></testsuite></testsuites>"#;
	let out = anonymize(xml.as_bytes());
	for gone in [
		"10.0.0.12",
		"192.168",
		"5432",
		"987654321",
		"424242",
		"8080",
		"8443",
		"123456",
		"3000",
		"12345",
	] {
		assert!(!out.contains(gone), "{gone} survived:\n{out}");
	}
	// Counts and lines up to four digits, the time, and the positions that end a frame stay.
	for kept in [
		"failures=\"3\"",
		"tests=\"12\"",
		"errors=\"0\"",
		"skipped=\"1\"",
		"line=\"42\"",
		"time=\"1234.5678\"",
		"(w",
		":12:5)",
		":341:5:",
		":24:4 ",
	] {
		assert!(out.contains(kept), "{kept} lost:\n{out}");
	}
	assert!(
		out.contains("tests=\"w") || out.contains("tests=\"n"),
		"five digits is not a count"
	);
	assert!(!out.contains("port 8"), "a number in prose is replaced");
}

#[test]
fn the_output_keeps_the_punctuation_and_layout_it_was_given() {
	let xml = "<testsuites name=\"x\"><testsuite name=\"s\"><testcase name=\"a, b: c (d) [e] {f} 'g'\" classname=\"c\"><failure>  indented line\n\n\tTabbed - dashes + plus\n</failure></testcase></testsuite></testsuites>";
	let out = anonymize(xml.as_bytes());
	let real = |xml: &str| -> Vec<String> {
		values(xml)
			.into_iter()
			.filter(|v| !v.trim().is_empty() || v.contains('\t'))
			.collect()
	};
	let (a, b) = (real(xml), real(&out));
	assert_eq!(a.len(), b.len());
	for (x, y) in a.iter().zip(&b) {
		// Punctuation, spaces, tabs and line breaks stay where they were.
		let punct = |t: &str| {
			t.chars()
				.filter(|c| !c.is_alphanumeric() && *c != '_')
				.collect::<String>()
		};
		assert_eq!(punct(x), punct(y));
	}
}

#[test]
fn word_lengths_are_hidden_but_the_number_of_words_is_not() {
	let xml =
		"<testsuites name=\"x\"><testsuite name=\"a bb ccccccccccccccccccccccc\"/></testsuites>";
	let out = anonymize(xml.as_bytes());
	let v: Vec<String> = values(&out)
		.into_iter()
		.filter(|v| !v.trim().is_empty())
		.collect();
	let words: Vec<&str> = v[1].split(' ').collect();
	assert_eq!(words.len(), 3, "the word count shows");
	assert!(
		words.iter().all(|w| w.len() == 13),
		"every token is the same length: {words:?}"
	);
}
