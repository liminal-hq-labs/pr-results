// Flavour detection and the per-reporter reading of names, files and lines.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::error::ParseError;
use crate::junit::{Budget, Facts, Limits};
use crate::model::{Case, Flavour, Status, Suite, Untrusted};
use crate::trusted::RepoPath;

/// Settles `Auto` from what the file said about itself. Steps files are never guessed: a product
/// says so in its settings.
pub(crate) fn resolve(requested: Flavour, facts: &Facts) -> Flavour {
	if requested != Flavour::Auto {
		return requested;
	}
	match facts.root_name.as_str() {
		"nextest-run" => Flavour::Nextest,
		"vitest tests" => Flavour::Vitest,
		"bun test" => Flavour::Bun,
		"jest tests" => Flavour::Jest,
		_ if facts.saw_assertions => Flavour::Bun,
		// Playwright writes `<testsuites id="" name="">`: the attributes are there and empty.
		_ if facts.saw_empty_id => Flavour::Playwright,
		_ if facts.saw_attachment => Flavour::Playwright,
		_ => Flavour::Generic,
	}
}

/// The settings that shape how a path from a report becomes a repository path.
pub(crate) struct Paths<'a> {
	/// The longest path kept, in bytes.
	pub cap: usize,
	/// Added in front of every path the report names: the directory the reporter's paths are
	/// relative to, as seen from the repository's root.
	pub prefix: Option<&'a RepoPath>,
}

impl Paths<'_> {
	/// A repository-relative path, or nothing. Without a prefix this is [`clean_path`]. With one,
	/// the prefix is put in front, and a leading run of `..` components climbs out of the prefix
	/// (a Playwright stack names `../src/x.js` relative to its test directory) but never past the
	/// repository's root. Every other refusal of [`clean_path`] stands.
	pub(crate) fn place(&self, raw: &str) -> Option<String> {
		let Some(prefix) = self.prefix else {
			return clean_path(raw, self.cap);
		};
		let raw = raw.trim();
		if raw.is_empty() || raw.len() > self.cap || raw.chars().any(char::is_control) {
			return None;
		}
		let unified = raw.replace('\\', "/");
		if unified.starts_with('/') || unified.contains(':') {
			return None;
		}
		let mut base: Vec<&str> = prefix.as_str().split('/').collect();
		let mut parts: Vec<&str> = Vec::new();
		for component in unified.split('/') {
			match component {
				"" | "." => {}
				".." => {
					if !parts.is_empty() || base.pop().is_none() {
						return None;
					}
				}
				other => parts.push(other),
			}
		}
		if parts.is_empty() {
			return None;
		}
		base.extend(parts);
		let joined = base.join("/");
		(joined.len() <= self.cap).then_some(joined)
	}

	/// `place`, kept as a stored text.
	fn keep(&self, raw: &str, budget: &mut Budget) -> Result<Option<Untrusted>, ParseError> {
		match self.place(raw) {
			Some(path) => Ok(Some(budget.keep(Untrusted::new(path))?)),
			None => Ok(None),
		}
	}

	/// `path` without the prefix, as the report wrote it relative to its own root.
	fn unprefixed<'p>(&self, path: &'p str) -> &'p str {
		self.prefix
			.and_then(|p| path.strip_prefix(p.as_str()))
			.and_then(|rest| rest.strip_prefix('/'))
			.unwrap_or(path)
	}
}

/// Stamps the suite with its flavour and fills in the locations that flavour implies. What a case
/// gets is charged to `budget`; a path taken from the suite's name is stored once and shared.
pub(crate) fn apply(
	flavour: Flavour,
	suite: &mut Suite,
	limits: &Limits,
	paths: &Paths<'_>,
	budget: &mut Budget,
) -> Result<(), ParseError> {
	suite.flavour = flavour;
	// Only Playwright's `hostname` is a project; elsewhere it is a machine's name, never shown.
	if flavour != Flavour::Playwright {
		suite.project = None;
	}
	let suite_file = match matches!(flavour, Flavour::Vitest | Flavour::Playwright) {
		true => match paths
			.place(suite.name.as_str())
			.filter(|p| looks_like_file(p))
		{
			Some(path) => Some(budget.keep(Untrusted::new(path))?),
			None => None,
		},
		false => None,
	};
	let js = matches!(
		flavour,
		Flavour::Vitest | Flavour::Jest | Flavour::Bun | Flavour::Playwright
	);
	for case in &mut suite.cases {
		if case.file.is_none() {
			match flavour {
				Flavour::Nextest => rust_location(case, limits, paths, budget)?,
				// vitest names its suites and class names after the test file.
				Flavour::Vitest => {
					case.file = match paths.place(case.classname.as_str()) {
						Some(path) if looks_like_file(&path) => {
							Some(budget.keep(Untrusted::new(path))?)
						}
						Some(_) => None,
						None => suite_file.clone(),
					};
				}
				// Playwright writes the file, then (older versions) the titles, joined by " › ".
				Flavour::Playwright => {
					let head = case
						.classname
						.as_str()
						.split(" \u{203a} ")
						.next()
						.unwrap_or_default();
					case.file = match paths.place(head) {
						Some(path) if looks_like_file(&path) => {
							Some(budget.keep(Untrusted::new(path))?)
						}
						Some(_) => None,
						None => suite_file.clone(),
					};
				}
				_ => {}
			}
		}
		if js {
			js_case(flavour, case, limits, paths, budget)?;
		}
	}
	Ok(())
}

/// What the JavaScript reporters share: where a failure happened, a message when the reporter
/// wrote none, and the shape of the names.
fn js_case(
	flavour: Flavour,
	case: &mut Case,
	limits: &Limits,
	paths: &Paths<'_>,
	budget: &mut Budget,
) -> Result<(), ParseError> {
	if matches!(case.status, Status::Failed | Status::Errored) {
		let own = case.file.as_ref().map(|f| paths.unprefixed(f.as_str()));
		let found = [case.detail.as_ref(), case.message.as_ref()]
			.into_iter()
			.flatten()
			.find_map(|text| js_location(text.as_str(), own, paths))
			.or_else(|| match flavour {
				Flavour::Playwright => case
					.detail
					.as_ref()
					.and_then(|d| playwright_header(d.as_str(), paths)),
				_ => None,
			});
		match found {
			Some(Found { file: None, line }) => case.line = Some(line),
			Some(Found {
				file: Some(path),
				line,
			}) => {
				case.file = match &case.file {
					Some(existing) if existing.as_str() == path => Some(existing.clone()),
					_ => Some(budget.keep(Untrusted::new(path))?),
				};
				case.line = Some(line);
			}
			None => {}
		}
		// bun puts the whole assertion, several lines, in the message attribute.
		let first_line = case.message.as_ref().and_then(|m| {
			let text = m.as_str().trim_start();
			(m.omitted() == 0 && text.contains('\n')).then(|| {
				text.lines()
					.next()
					.unwrap_or_default()
					.trim_end()
					.to_owned()
			})
		});
		if let Some(first) = first_line {
			case.message = Some(budget.keep(Untrusted::new(first))?);
		}
		if case.message.is_none() {
			let first = case
				.detail
				.as_ref()
				.and_then(|d| d.as_str().lines().map(str::trim).find(|l| !l.is_empty()))
				.map(|l| Untrusted::capped(l, limits.max_message_bytes));
			if let Some(first) = first {
				case.message = Some(budget.keep(first)?);
			}
		}
	}
	match flavour {
		// bun lists the describe blocks innermost first (`rounding > division > math`); read them
		// outermost first, as they were written.
		Flavour::Bun
			if case.classname.omitted() == 0 && case.classname.as_str().contains(" > ") =>
		{
			let mut blocks: Vec<&str> = case.classname.as_str().split(" > ").collect();
			blocks.reverse();
			let outer_first = Untrusted::capped(&blocks.join(" > "), limits.max_name_bytes);
			case.classname = budget.keep(outer_first)?;
		}
		// jest-junit's default writes the whole title as both the class name and the name, and a
		// test outside any describe block starts with a space.
		Flavour::Jest => {
			let name = case.name.as_str();
			if case.classname.as_str() == name {
				case.classname = Untrusted::default();
			}
			if case.name.omitted() == 0 && name != name.trim() {
				case.name = budget.keep(Untrusted::new(name.trim()))?;
			}
		}
		_ => {}
	}
	Ok(())
}

/// A place found in a stack: the line, and the path unless it is the case's own file.
#[derive(Debug, PartialEq, Eq)]
struct Found {
	file: Option<String>,
	line: u32,
}

/// The first stack frame that names a place in the repository.
///
/// Frames look like `at fn (path:line:col)`, `at path:line:col`, `at path:line` (Playwright) and
/// `❯ fn path:line:col` (vitest). A frame is skipped when it is inside `node_modules`, has no line,
/// is a Node internal (`node:internal/...`), or is a path that cannot be placed in the repository.
/// A `file://` URL and an absolute path are skipped too, except that an absolute path ending in
/// `/` and the case's own file is that file: the runner's checkout is not known, but the file
/// that the report itself named is.
fn js_location(text: &str, own_file: Option<&str>, paths: &Paths<'_>) -> Option<Found> {
	text.lines().find_map(|line| {
		let (raw, line_no) = frame_location(line)?;
		if let Some(absolute) = raw
			.strip_prefix("file://")
			.or(Some(raw).filter(|r| r.starts_with('/')))
		{
			let own = own_file?;
			let absolute = absolute.replace('\\', "/");
			return (absolute.starts_with('/') && absolute.ends_with(&format!("/{own}")))
				.then_some(Found {
					file: None,
					line: line_no,
				});
		}
		let placed = paths.place(raw)?;
		// A name with no extension is a word in a message, not a source file.
		if !placed
			.rsplit('/')
			.next()
			.is_some_and(|last| last.contains('.'))
		{
			return None;
		}
		if placed.split('/').any(|c| c == "node_modules") {
			return None;
		}
		Some(Found {
			file: Some(placed),
			line: line_no,
		})
	})
}

/// Playwright starts a failure with `[project] › path:line:col › titles`: the place the test is
/// declared, relative to the test directory. The fallback when the stack names nothing usable.
fn playwright_header(detail: &str, paths: &Paths<'_>) -> Option<Found> {
	let first = detail.lines().map(str::trim).find(|l| !l.is_empty())?;
	let rest = first.strip_prefix('[')?;
	let (_, after) = rest.split_once("] \u{203a} ")?;
	let token = after.split(" \u{203a} ").next()?;
	let (raw, line) = split_line(token)?;
	if raw.starts_with('/') {
		return None;
	}
	Some(Found {
		file: Some(paths.place(raw).filter(|p| looks_like_file(p))?),
		line,
	})
}

/// The `path` and `line` of one stack-frame line, if it is a frame and has both.
fn frame_location(line: &str) -> Option<(&str, u32)> {
	let line = line.trim();
	let rest = line
		.strip_prefix("at ")
		.or_else(|| line.strip_prefix('\u{276f}'))?
		.trim();
	let place = match (rest.ends_with(')'), rest.rfind('(')) {
		(true, Some(open)) => rest.get(open + 1..rest.len() - 1)?,
		_ => rest.rsplit(char::is_whitespace).next()?,
	};
	split_line(place)
}

/// `path:line:col` or `path:line` as a path and a line (not nought).
fn split_line(place: &str) -> Option<(&str, u32)> {
	let mut parts = place.rsplitn(3, ':');
	let last = parts.next()?;
	let middle = parts.next()?;
	let (path, line) = match parts.next() {
		Some(path) if is_number(last) && is_number(middle) => (path, middle),
		None if is_number(last) => (middle, last),
		_ => return None,
	};
	let line: u32 = line.parse().ok()?;
	(line > 0).then_some((path, line))
}

fn is_number(s: &str) -> bool {
	!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn looks_like_file(path: &str) -> bool {
	path.rsplit('/')
		.next()
		.is_some_and(|last| last.contains('.') && !last.contains(' '))
}

/// Reads `panicked at <path>:<line>:<col>` from a failure, in both the current format
/// (`panicked at src/lib.rs:12:5:`) and the older one (`panicked at 'msg', src/lib.rs:12:5`).
fn rust_location(
	case: &mut Case,
	limits: &Limits,
	paths: &Paths<'_>,
	budget: &mut Budget,
) -> Result<(), ParseError> {
	let sources = [case.detail.as_ref(), case.message.as_ref()];
	for text in sources.into_iter().flatten() {
		if let Some((path, line)) = find_panic_location(text.as_str(), limits.max_name_bytes) {
			if let Some(placed) = paths.keep(&path, budget)? {
				case.file = Some(placed);
				case.line = Some(line);
				return Ok(());
			}
		}
	}
	Ok(())
}

pub(crate) fn find_panic_location(text: &str, max_path: usize) -> Option<(String, u32)> {
	const MARK: &str = "panicked at ";
	let start = text.find(MARK)? + MARK.len();
	let line = text.get(start..)?.lines().next()?;
	let rest = match line.rfind("', ") {
		Some(i) if line.starts_with('\'') => line.get(i + 3..)?,
		_ => line,
	};
	let token = rest.split_whitespace().next()?.trim_end_matches(':');
	let mut parts = token.rsplitn(3, ':');
	let _col: u32 = parts.next()?.parse().ok()?;
	let line_no: u32 = parts.next()?.parse().ok()?;
	let path = clean_path(parts.next()?, max_path)?;
	Some((path, line_no))
}

/// A repository-relative path, or nothing. Backslashes become slashes; `.` and empty components
/// are dropped. Absolute paths, drive letters, URLs, `..`, control characters and over-long
/// paths are all refused, so a path from a report can never name a place outside the repository.
pub(crate) fn clean_path(raw: &str, max_bytes: usize) -> Option<String> {
	let raw = raw.trim();
	if raw.is_empty() || raw.len() > max_bytes || raw.chars().any(char::is_control) {
		return None;
	}
	let unified = raw.replace('\\', "/");
	if unified.starts_with('/') || unified.contains(':') {
		return None;
	}
	let mut parts = Vec::new();
	for component in unified.split('/') {
		match component {
			"" | "." => {}
			".." => return None,
			other => parts.push(other),
		}
	}
	(!parts.is_empty()).then(|| parts.join("/"))
}

#[cfg(test)]
#[allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]
mod tests {
	use super::{clean_path, find_panic_location};

	#[test]
	fn paths_must_stay_inside_the_repository() {
		let ok = [
			("src/lib.rs", "src/lib.rs"),
			("./src//lib.rs", "src/lib.rs"),
			("src\\win\\lib.rs", "src/win/lib.rs"),
			("  a/b.ts ", "a/b.ts"),
		];
		for (input, want) in ok {
			assert_eq!(clean_path(input, 100).as_deref(), Some(want), "{input:?}");
		}
		let refused = [
			"",
			".",
			"/etc/passwd",
			"../x",
			"a/../../x",
			"a/..",
			"C:\\Windows\\x",
			"c:/x",
			"file:///etc/passwd",
			"https://example.invalid/x",
			"a\0b",
			"a\nb",
			"\\\\server\\share",
		];
		for input in refused {
			assert_eq!(clean_path(input, 100), None, "{input:?}");
		}
		assert_eq!(clean_path("abcdef", 5), None, "over the length cap");
	}

	#[test]
	fn panic_locations() {
		let table = [
			(
				"thread 'x' panicked at src/a.rs:1:2:\nmsg",
				Some(("src/a.rs", 1)),
			),
			(
				"thread 'x' panicked at 'msg, with: colons', src/b.rs:10:20",
				Some(("src/b.rs", 10)),
			),
			(
				"panicked at crates/c/src/lib.rs:7:1",
				Some(("crates/c/src/lib.rs", 7)),
			),
			("panicked at src/a.rs:x:2:", None),
			("panicked at src/a.rs", None),
			("panicked at ", None),
			("panicked at ../a.rs:1:1:", None),
			("no panic here: src/a.rs:1:1", None),
		];
		for (text, want) in table {
			let got = find_panic_location(text, 100);
			assert_eq!(
				got.as_ref().map(|(p, l)| (p.as_str(), *l)),
				want,
				"{text:?}"
			);
		}
	}
}
