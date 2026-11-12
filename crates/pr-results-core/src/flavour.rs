// Flavour detection and the per-reporter reading of names, files and lines.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::error::ParseError;
use crate::junit::{Budget, Facts, Limits};
use crate::model::{Case, Flavour, Suite, Untrusted};

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
		_ if facts.saw_attachment => Flavour::Playwright,
		_ => Flavour::Generic,
	}
}

/// Stamps the suite with its flavour and fills in the locations that flavour implies. What a case
/// gets is charged to `budget`; a path taken from the suite's name is stored once and shared.
pub(crate) fn apply(
	flavour: Flavour,
	suite: &mut Suite,
	limits: &Limits,
	budget: &mut Budget,
) -> Result<(), ParseError> {
	suite.flavour = flavour;
	let cap = limits.max_name_bytes;
	let suite_file = match matches!(flavour, Flavour::Vitest | Flavour::Playwright) {
		true => match clean_path(suite.name.as_str(), cap).filter(|p| looks_like_file(p)) {
			Some(path) => Some(budget.keep(Untrusted::new(path))?),
			None => None,
		},
		false => None,
	};
	for case in &mut suite.cases {
		if case.file.is_some() {
			continue;
		}
		match flavour {
			Flavour::Nextest => rust_location(case, limits, budget)?,
			// vitest names its suites and class names after the test file.
			Flavour::Vitest => {
				case.file = match clean_path(case.classname.as_str(), cap) {
					Some(path) if looks_like_file(&path) => {
						Some(budget.keep(Untrusted::new(path))?)
					}
					Some(_) => None,
					None => suite_file.clone(),
				};
			}
			// Playwright writes the file, then the titles, joined by " › ".
			Flavour::Playwright => {
				let head = case
					.classname
					.as_str()
					.split(" \u{203a} ")
					.next()
					.unwrap_or_default();
				case.file = match clean_path(head, cap) {
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
	Ok(())
}

fn looks_like_file(path: &str) -> bool {
	path.rsplit('/')
		.next()
		.is_some_and(|last| last.contains('.') && !last.contains(' '))
}

/// Reads `panicked at <path>:<line>:<col>` from a failure, in both the current format
/// (`panicked at src/lib.rs:12:5:`) and the older one (`panicked at 'msg', src/lib.rs:12:5`).
fn rust_location(case: &mut Case, limits: &Limits, budget: &mut Budget) -> Result<(), ParseError> {
	let sources = [case.detail.as_ref(), case.message.as_ref()];
	for text in sources.into_iter().flatten() {
		if let Some((path, line)) = find_panic_location(text.as_str(), limits.max_name_bytes) {
			case.file = Some(budget.keep(Untrusted::new(path))?);
			case.line = Some(line);
			return Ok(());
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
