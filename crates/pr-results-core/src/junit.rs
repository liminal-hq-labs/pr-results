// A streaming JUnit XML reader that treats the whole input as hostile.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quick_xml::errors::Error as XmlError;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::ParseError;
use crate::flavour;
use crate::model::{Attempt, Case, Flavour, Report, Status, Suite, TextBuf, Untrusted};

/// The most attributes one element may carry.
const MAX_ATTRIBUTES: usize = 256;

/// What the parser will accept before it gives up.
#[derive(Debug, Clone)]
pub struct Limits {
	/// The whole input, in bytes, both as given and after invalid UTF-8 is replaced (each bad byte
	/// becomes the three-byte U+FFFD, so the second is the larger).
	pub max_bytes: usize,
	/// All the text the report keeps, in bytes: names, class names, paths, messages and bodies,
	/// counted once per stored copy after invalid UTF-8 is replaced. A file path shared by every
	/// case of a suite is stored and counted once.
	pub max_text_bytes: usize,
	/// Element nesting, root included.
	pub max_depth: usize,
	/// Test cases across all suites.
	pub max_cases: usize,
	/// Suites across the input, counting each top-level `testsuite` once.
	pub max_suites: usize,
	/// Retry attempts kept per case; the rest are counted in `Case::attempts_omitted`.
	pub max_attempts: usize,
	/// Names, class names and file paths, in bytes.
	pub max_name_bytes: usize,
	/// A failure message, in bytes.
	pub max_message_bytes: usize,
	/// A failure body and captured output, in bytes.
	pub max_detail_bytes: usize,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			max_bytes: 50 * 1024 * 1024,
			max_text_bytes: 64 * 1024 * 1024,
			max_depth: 64,
			max_cases: 200_000,
			max_suites: 10_000,
			max_attempts: 100,
			max_name_bytes: 1024,
			max_message_bytes: 1024,
			max_detail_bytes: 16 * 1024,
		}
	}
}

/// Parses one JUnit file.
///
/// The input is untrusted. It is rejected, with an error that names the problem and quotes
/// nothing, when it is too large, truncated, not well formed, nested too deeply, holds too many
/// cases, declares a document type or an encoding other than UTF-8, or uses an entity other than
/// the five predefined ones and numeric references. Invalid UTF-8 is replaced with U+FFFD. The
/// result depends only on the bytes and the arguments.
pub fn parse_junit(input: &[u8], flavour: Flavour, limits: &Limits) -> Result<Report, ParseError> {
	if input.len() > limits.max_bytes {
		return Err(ParseError::TooLarge {
			len: input.len(),
			limit: limits.max_bytes,
		});
	}
	let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
	let text = String::from_utf8_lossy(input);
	// Invalid bytes grow to three each, so the copy that is parsed is bounded as well.
	if text.len() > limits.max_bytes {
		return Err(ParseError::TooLarge {
			len: text.len(),
			limit: limits.max_bytes,
		});
	}
	if text.trim().is_empty() {
		return Err(ParseError::Empty);
	}
	if let Some(offset) = text.find('\0') {
		return Err(ParseError::Malformed {
			offset: offset as u64,
		});
	}

	let mut parser = Parser::new(limits);
	parser.run(&text)?;
	let mut suites = parser.suites;
	let resolved = flavour::resolve(flavour, &parser.facts);
	for suite in &mut suites {
		flavour::apply(resolved, suite, limits, &mut parser.budget)?;
	}
	Ok(Report { suites })
}

/// How much kept text is left.
pub(crate) struct Budget {
	used: usize,
	limit: usize,
}

impl Budget {
	fn new(limit: usize) -> Self {
		Self { used: 0, limit }
	}

	pub(crate) fn charge(&mut self, bytes: usize) -> Result<(), ParseError> {
		self.used = self.used.saturating_add(bytes);
		if self.used > self.limit {
			return Err(ParseError::TooMuchText { limit: self.limit });
		}
		Ok(())
	}

	/// Charges for `text` and hands it back.
	pub(crate) fn keep(&mut self, text: Untrusted) -> Result<Untrusted, ParseError> {
		self.charge(text.len())?;
		Ok(text)
	}
}

/// What the file said about itself, for flavour detection.
#[derive(Debug, Default)]
pub(crate) struct Facts {
	pub root_name: String,
	pub saw_assertions: bool,
	pub saw_attachment: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tag {
	Testsuites,
	Testsuite,
	Testcase,
	Failure,
	Error,
	Skipped,
	FlakyFailure,
	FlakyError,
	RerunFailure,
	RerunError,
	SystemOut,
	SystemErr,
	/// nextest puts a retry's panic text in a `stack-trace` child.
	StackTrace,
	Other,
}

impl Tag {
	fn is_result(self) -> bool {
		matches!(
			self,
			Self::Failure
				| Self::Error
				| Self::Skipped
				| Self::FlakyFailure
				| Self::FlakyError
				| Self::RerunFailure
				| Self::RerunError
		)
	}
}

struct SuiteBuilder {
	name: Untrusted,
	time_ms: Option<u64>,
	cases: Vec<Case>,
}

#[derive(Default)]
struct CaseBuilder {
	name: Untrusted,
	classname: Untrusted,
	file: Option<Untrusted>,
	line: Option<u32>,
	time_ms: u64,
	failure: Option<(Status, Option<Untrusted>, Option<Untrusted>)>,
	skipped: Option<(Option<Untrusted>, Option<Untrusted>)>,
	attempts: Vec<Attempt>,
	attempts_omitted: usize,
	system_out: Option<Untrusted>,
	system_err: Option<Untrusted>,
}

/// The element whose text is being collected.
struct Sink {
	message: Option<Untrusted>,
	text: TextBuf,
	cap: usize,
}

struct Parser<'l> {
	limits: &'l Limits,
	stack: Vec<Tag>,
	roots: usize,
	suites: Vec<Suite>,
	suite: Option<SuiteBuilder>,
	/// The `file` attribute of each open suite, innermost last. Cases share it, not copy it.
	files: Vec<Option<Untrusted>>,
	case: Option<CaseBuilder>,
	sink: Option<Sink>,
	cases_seen: usize,
	budget: Budget,
	facts: Facts,
	/// Byte position of the event being handled, for error messages.
	position: u64,
}

impl<'l> Parser<'l> {
	fn new(limits: &'l Limits) -> Self {
		Self {
			limits,
			stack: Vec::new(),
			roots: 0,
			suites: Vec::new(),
			suite: None,
			files: Vec::new(),
			case: None,
			sink: None,
			cases_seen: 0,
			budget: Budget::new(limits.max_text_bytes),
			facts: Facts::default(),
			position: 0,
		}
	}

	fn run(&mut self, text: &str) -> Result<(), ParseError> {
		let mut reader = Reader::from_str(text);
		loop {
			let event = match reader.read_event() {
				Ok(event) => event,
				Err(XmlError::Syntax(_)) => return Err(ParseError::Truncated),
				Err(XmlError::Escape(_)) => return Err(ParseError::BadEntity),
				Err(_) => {
					return Err(ParseError::Malformed {
						offset: reader.error_position(),
					})
				}
			};
			self.position = reader.buffer_position();
			match event {
				Event::Eof => break,
				Event::DocType(_) => return Err(ParseError::Dtd),
				Event::Decl(decl) => {
					if let Some(Ok(encoding)) = decl.encoding() {
						if !encoding.eq_ignore_ascii_case("utf-8") {
							return Err(ParseError::UnsupportedEncoding);
						}
					}
				}
				Event::Start(e) => self.open(&e)?,
				Event::Empty(e) => {
					self.open(&e)?;
					self.close()?;
				}
				Event::End(_) => self.close()?,
				Event::Text(t) => {
					let content = t.xml_content(XmlVersion::Implicit1_0);
					self.text(&content)?;
				}
				Event::CData(c) => {
					let content = c.xml_content(XmlVersion::Implicit1_0);
					self.text(&content)?;
				}
				Event::GeneralRef(r) => {
					let resolved = match r.resolve_char_ref() {
						Ok(Some(c)) => c,
						Ok(None) => match &*r {
							"lt" => '<',
							"gt" => '>',
							"amp" => '&',
							"apos" => '\'',
							"quot" => '"',
							_ => return Err(ParseError::BadEntity),
						},
						Err(_) => return Err(ParseError::BadEntity),
					};
					let mut buf = [0u8; 4];
					self.text(resolved.encode_utf8(&mut buf))?;
				}
				Event::Comment(_) | Event::PI(_) => {}
			}
		}
		if !self.stack.is_empty() || self.roots == 0 {
			return Err(ParseError::Truncated);
		}
		Ok(())
	}

	fn text(&mut self, content: &str) -> Result<(), ParseError> {
		if self.stack.is_empty() {
			// Only whitespace may sit outside the root.
			return if content.trim().is_empty() {
				Ok(())
			} else {
				Err(ParseError::Structure("text outside the root element"))
			};
		}
		if let Some(sink) = &mut self.sink {
			if self.stack.last().is_some_and(|t| *t != Tag::Other) {
				sink.text.push_capped(content, sink.cap);
			}
		}
		Ok(())
	}

	fn tag_for(&self, name: &str) -> Result<Tag, ParseError> {
		let parent = self.stack.last().copied();
		let Some(parent) = parent else {
			return match name {
				"testsuites" => Ok(Tag::Testsuites),
				"testsuite" => Ok(Tag::Testsuite),
				_ => Err(ParseError::UnexpectedRoot),
			};
		};
		Ok(match (parent, name) {
			(Tag::Testsuites | Tag::Testsuite, "testsuite") => Tag::Testsuite,
			(Tag::Testsuite, "testcase") => Tag::Testcase,
			(Tag::Testsuites, "testcase") => {
				return Err(ParseError::Structure("a test case outside a suite"))
			}
			(Tag::Testcase, "failure") => Tag::Failure,
			(Tag::Testcase, "error") => Tag::Error,
			(Tag::Testcase, "skipped") => Tag::Skipped,
			(Tag::Testcase, "flakyFailure") => Tag::FlakyFailure,
			(Tag::Testcase, "flakyError") => Tag::FlakyError,
			(Tag::Testcase, "rerunFailure") => Tag::RerunFailure,
			(Tag::Testcase, "rerunError") => Tag::RerunError,
			(Tag::Testcase, "system-out") => Tag::SystemOut,
			(Tag::Testcase, "system-err") => Tag::SystemErr,
			(p, "stack-trace") if p.is_result() => Tag::StackTrace,
			_ => Tag::Other,
		})
	}

	fn attributes(&self, e: &BytesStart<'_>) -> Result<Vec<(String, String)>, ParseError> {
		let mut out = Vec::new();
		for attr in e.attributes() {
			let attr = attr.map_err(|_| ParseError::Malformed {
				offset: self.position,
			})?;
			if out.len() >= MAX_ATTRIBUTES {
				return Err(ParseError::TooManyAttributes {
					limit: MAX_ATTRIBUTES,
				});
			}
			let value = attr
				.normalized_value(XmlVersion::Implicit1_0)
				.map_err(|_| ParseError::BadEntity)?;
			out.push((attr.key.as_ref().to_owned(), value.into_owned()));
		}
		Ok(out)
	}

	fn open(&mut self, e: &BytesStart<'_>) -> Result<(), ParseError> {
		if self.stack.len() >= self.limits.max_depth {
			return Err(ParseError::TooDeep {
				limit: self.limits.max_depth,
			});
		}
		let tag = self.tag_for(e.name().as_ref())?;
		if self.stack.is_empty() {
			self.roots += 1;
			if self.roots > 1 {
				return Err(ParseError::Structure("more than one root element"));
			}
		}
		let relevant = tag != Tag::Other;
		let attrs = if relevant {
			self.attributes(e)?
		} else {
			Vec::new()
		};
		let get = |key: &str| {
			attrs
				.iter()
				.find(|(k, _)| k == key)
				.map(|(_, v)| v.as_str())
		};
		let name_cap = self.limits.max_name_bytes;
		let capped = |v: &str| Untrusted::capped(v, name_cap);

		match tag {
			Tag::Testsuites => {
				self.facts.root_name = get("name").unwrap_or_default().to_owned();
				self.facts.saw_assertions |= get("assertions").is_some();
			}
			Tag::Testsuite => {
				self.facts.saw_assertions |= get("assertions").is_some();
				if self.stack.is_empty() {
					self.facts.root_name = get("name").unwrap_or_default().to_owned();
				}
				let file = match get("file").and_then(|f| flavour::clean_path(f, name_cap)) {
					Some(path) => Some(self.budget.keep(Untrusted::new(path))?),
					None => None,
				};
				self.files.push(file);
				if self.suite.is_none() {
					self.suite = Some(SuiteBuilder {
						name: self.budget.keep(capped(get("name").unwrap_or_default()))?,
						time_ms: get("time").map(parse_time_ms),
						cases: Vec::new(),
					});
				}
			}
			Tag::Testcase => {
				self.cases_seen += 1;
				if self.cases_seen > self.limits.max_cases {
					return Err(ParseError::TooManyCases {
						limit: self.limits.max_cases,
					});
				}
				let own = match get("file").and_then(|f| flavour::clean_path(f, name_cap)) {
					Some(path) => Some(self.budget.keep(Untrusted::new(path))?),
					None => None,
				};
				let file = own.or_else(|| self.files.iter().rev().find_map(Clone::clone));
				self.case = Some(CaseBuilder {
					name: self.budget.keep(capped(get("name").unwrap_or_default()))?,
					classname: self
						.budget
						.keep(capped(get("classname").unwrap_or_default()))?,
					file,
					line: get("line").and_then(|l| l.trim().parse::<u32>().ok()),
					time_ms: get("time").map_or(0, parse_time_ms),
					..CaseBuilder::default()
				});
			}
			t if t.is_result() => {
				let message = match get("message") {
					Some(m) => Some(
						self.budget
							.keep(Untrusted::capped(m, self.limits.max_message_bytes))?,
					),
					None => None,
				};
				self.sink = Some(Sink {
					message,
					text: TextBuf::default(),
					cap: self.limits.max_detail_bytes,
				});
			}
			Tag::SystemOut | Tag::SystemErr => {
				self.sink = Some(Sink {
					message: None,
					text: TextBuf::default(),
					cap: self.limits.max_detail_bytes,
				});
			}
			_ => {}
		}
		self.stack.push(tag);
		Ok(())
	}

	fn close(&mut self) -> Result<(), ParseError> {
		let Some(tag) = self.stack.pop() else {
			return Err(ParseError::Structure("a closing tag with no opening tag"));
		};
		match tag {
			Tag::Testsuite => {
				self.files.pop();
				if self.files.is_empty() {
					if let Some(done) = self.suite.take() {
						if self.suites.len() >= self.limits.max_suites {
							return Err(ParseError::TooManySuites {
								limit: self.limits.max_suites,
							});
						}
						let sum = done
							.cases
							.iter()
							.fold(0u64, |t, c| t.saturating_add(c.time_ms));
						self.suites.push(Suite {
							name: done.name,
							flavour: Flavour::Generic,
							time_ms: done.time_ms.unwrap_or(sum),
							cases: done.cases,
						});
					}
				}
			}
			Tag::Testcase => {
				if let (Some(case), Some(suite)) = (self.case.take(), self.suite.as_mut()) {
					suite.cases.push(finish_case(case));
				}
			}
			t if t.is_result() || matches!(t, Tag::SystemOut | Tag::SystemErr) => {
				let Some(sink) = self.sink.take() else {
					return Ok(());
				};
				if sink.text.as_str().contains("[[ATTACHMENT|") {
					self.facts.saw_attachment = true;
				}
				// Charged whether or not the case keeps it, so repeated elements cannot hide cost.
				self.budget.charge(sink.text.as_str().len())?;
				let max = self.limits.max_attempts;
				if let Some(case) = self.case.as_mut() {
					let detail = sink.text.trimmed();
					match t {
						Tag::Failure => {
							case.failure
								.get_or_insert((Status::Failed, sink.message, detail));
						}
						Tag::Error => {
							case.failure
								.get_or_insert((Status::Errored, sink.message, detail));
						}
						Tag::Skipped => {
							case.skipped.get_or_insert((sink.message, detail));
						}
						Tag::FlakyFailure
						| Tag::RerunFailure
						| Tag::FlakyError
						| Tag::RerunError => {
							if case.attempts.len() >= max {
								case.attempts_omitted = case.attempts_omitted.saturating_add(1);
							} else {
								let status = if matches!(t, Tag::FlakyFailure | Tag::RerunFailure) {
									Status::Failed
								} else {
									Status::Errored
								};
								case.attempts.push(Attempt {
									status,
									message: sink.message,
									detail,
								});
							}
						}
						Tag::SystemOut => case.system_out = detail,
						_ => case.system_err = detail,
					}
				}
			}
			_ => {}
		}
		Ok(())
	}
}

fn finish_case(b: CaseBuilder) -> Case {
	// Every kept attempt is a failed or errored try, so any attempt before a pass makes a case
	// flaky.
	let flaky = b.failure.is_none() && b.skipped.is_none() && !b.attempts.is_empty();
	let (status, message, detail) = match (b.failure, b.skipped) {
		(Some((status, message, detail)), _) => (status, message, detail),
		(None, Some((message, detail))) => (Status::Skipped, message, detail),
		(None, None) => (
			if flaky { Status::Flaky } else { Status::Passed },
			None,
			None,
		),
	};
	Case {
		name: b.name,
		classname: b.classname,
		file: b.file,
		line: b.line,
		status,
		time_ms: b.time_ms,
		attempts: b.attempts,
		attempts_omitted: b.attempts_omitted,
		message,
		detail,
		system_out: b.system_out,
		system_err: b.system_err,
	}
}

/// Seconds as written in JUnit, to whole milliseconds, with no floating point. Anything that is
/// not plain decimal digits (an exponent, a locale comma, a sign) counts as zero rather than
/// failing the file, because the time is informational.
pub(crate) fn parse_time_ms(s: &str) -> u64 {
	let s = s.trim();
	let (int, frac) = s.split_once('.').unwrap_or((s, ""));
	if (int.is_empty() && frac.is_empty())
		|| !int.bytes().all(|b| b.is_ascii_digit())
		|| !frac.bytes().all(|b| b.is_ascii_digit())
	{
		return 0;
	}
	let secs = int.bytes().fold(0u64, |n, b| {
		n.saturating_mul(10).saturating_add(u64::from(b - b'0'))
	});
	let mut digits = frac.bytes().map(|b| u64::from(b - b'0'));
	let mut ms = 0u64;
	for _ in 0..3 {
		ms = ms * 10 + digits.next().unwrap_or(0);
	}
	if digits.next().unwrap_or(0) >= 5 {
		ms += 1;
	}
	secs.saturating_mul(1000).saturating_add(ms)
}

#[cfg(test)]
#[allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]
mod tests {
	use super::parse_time_ms;

	#[test]
	fn times_are_whole_milliseconds_without_floats() {
		let table = [
			("0", 0),
			("1", 1000),
			("0.4", 400),
			("1.234", 1234),
			("1.2345", 1235),
			("1.2344", 1234),
			(" 2.5 ", 2500),
			(".5", 500),
			("5.", 5000),
			("", 0),
			(".", 0),
			("-1", 0),
			("1e3", 0),
			("1,5", 0),
			("NaN", 0),
			("0.0005", 1),
			("99999999999999999999999", u64::MAX),
		];
		for (input, ms) in table {
			assert_eq!(parse_time_ms(input), ms, "{input:?}");
		}
	}
}
