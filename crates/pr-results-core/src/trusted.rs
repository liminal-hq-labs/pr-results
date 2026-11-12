// Values the tool controls (hashes, names, URLs, numbers) and the `Safe` text built from them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A renderer needs a few things that are neither a literal nor untrusted report text: a commit
//! hash, a link, a duration. Each is parsed here once, with strict rules, so a value that reaches
//! output is known to hold only characters that mean nothing to Markdown. They live in this crate
//! because `Safe` can be built from checked parts only inside it.

use std::fmt;

use crate::flavour::clean_path;
use crate::model::Untrusted;
use crate::safe::{is_invisible, Safe, Source};

/// The longest repository path a report may name, in bytes.
const MAX_PATH_BYTES: usize = 1024;

/// The first line of the sticky comment starts with this, up to the key.
const MARKER_PREFIX: &str = "<!-- pr-results:1 key=";

/// A full commit hash: 40 lowercase hexadecimal digits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha(String);

impl Sha {
	pub fn parse(s: &str) -> Option<Self> {
		let ok = s.len() == 40 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
		ok.then(|| Self(s.to_owned()))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}

	/// The first seven digits.
	pub fn short(&self) -> &str {
		self.0.get(..7).unwrap_or(&self.0)
	}
}

/// `owner/name`, with the characters GitHub allows in each part.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoSlug {
	owner: String,
	name: String,
}

impl RepoSlug {
	pub fn parse(s: &str) -> Option<Self> {
		let (owner, name) = s.split_once('/')?;
		let owner_ok = (1..=39).contains(&owner.len())
			&& !owner.starts_with('-')
			&& owner
				.bytes()
				.all(|b| b.is_ascii_alphanumeric() || b == b'-');
		let name_ok = (1..=100).contains(&name.len())
			&& name != "."
			&& name != ".."
			&& name
				.bytes()
				.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
		(owner_ok && name_ok).then(|| Self {
			owner: owner.to_owned(),
			name: name.to_owned(),
		})
	}

	pub fn owner(&self) -> &str {
		&self.owner
	}

	pub fn name(&self) -> &str {
		&self.name
	}
}

impl fmt::Display for RepoSlug {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}/{}", self.owner, self.name)
	}
}

/// A workflow run's number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId(pub u64);

/// Tells one sticky comment from another in the same pull request: `[a-z0-9][a-z0-9-]{0,31}`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommentKey(String);

impl CommentKey {
	pub fn parse(s: &str) -> Option<Self> {
		let mut bytes = s.bytes();
		let first_ok = bytes
			.next()
			.is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
		let rest_ok = bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
		(first_ok && rest_ok && s.len() <= 32).then(|| Self(s.to_owned()))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

/// `https://host` or `https://host:port`: no user, path, query or fragment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerUrl(String);

impl ServerUrl {
	pub fn parse(s: &str) -> Option<Self> {
		let rest = s.strip_prefix("https://")?;
		let (host, port) = match rest.split_once(':') {
			Some((host, port)) => (host, Some(port)),
			None => (rest, None),
		};
		if let Some(port) = port {
			let digits =
				!port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit());
			let value = port.parse::<u16>().ok().filter(|p| *p != 0);
			if !digits || value.is_none() || (port.len() > 1 && port.starts_with('0')) {
				return None;
			}
		}
		if host.is_empty() || host.len() > 253 {
			return None;
		}
		let label_ok = |label: &str| {
			!label.is_empty()
				&& label.len() <= 63
				&& !label.starts_with('-')
				&& !label.ends_with('-')
				&& label
					.bytes()
					.all(|b| b.is_ascii_alphanumeric() || b == b'-')
		};
		if !host.split('.').all(label_ok) {
			return None;
		}
		Some(Self(format!(
			"https://{}{}",
			host.to_ascii_lowercase(),
			port.map(|p| format!(":{p}")).unwrap_or_default()
		)))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

/// A path inside the repository that passed the rules of decision D26: relative, no `..`, no
/// drive letter or URL, no control characters, and nothing that draws as blank.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath(String);

impl RepoPath {
	/// Normalizes `.` and empty components and backslashes, and refuses the rest.
	pub fn parse(raw: &str) -> Option<Self> {
		let cleaned = clean_path(raw, MAX_PATH_BYTES)?;
		if cleaned.chars().any(is_invisible) {
			return None;
		}
		Some(Self(cleaned))
	}

	/// The path a case reports, if there is one and it is whole: a path that was cut short is no
	/// path.
	pub fn from_case(file: &Untrusted) -> Option<Self> {
		if file.omitted() > 0 {
			return None;
		}
		Self::parse(file.as_str())
	}

	/// This path followed by `relative`, checked again as a whole. `None` when the result breaks
	/// the path rules, so a prefix and a path that are each fine can never make one that is not
	/// (`relative` may not climb out with `..`).
	pub fn join(&self, relative: &str) -> Option<Self> {
		Self::parse(&format!("{}/{}", self.0, relative))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl Source for RepoPath {
	fn raw(&self) -> &str {
		&self.0
	}
}

/// A link the tool builds from parts it controls. There is no constructor from a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedUrl(String);

impl TrustedUrl {
	pub fn commit(server: &ServerUrl, repo: &RepoSlug, sha: &Sha) -> Self {
		Self(format!("{}/{}/commit/{}", server.0, repo, sha.0))
	}

	pub fn run(server: &ServerUrl, repo: &RepoSlug, id: RunId) -> Self {
		Self(format!("{}/{}/actions/runs/{}", server.0, repo, id.0))
	}

	/// A Forgejo run's page, which is named by the run's number (the number the runner exports
	/// as `GITHUB_RUN_NUMBER`), where GitHub's is named by its id.
	pub fn forgejo_run(server: &ServerUrl, repo: &RepoSlug, number: u64) -> Self {
		Self(format!("{}/{}/actions/runs/{}", server.0, repo, number))
	}

	/// A comment on a Forgejo pull request, by its anchor on the pull request's page.
	pub fn comment_anchor(server: &ServerUrl, repo: &RepoSlug, pr: u64, comment: u64) -> Self {
		Self(format!(
			"{}/{}/pulls/{}#issuecomment-{}",
			server.0, repo, pr, comment
		))
	}

	/// The artefacts section at the foot of a run's page.
	pub fn run_artifacts(server: &ServerUrl, repo: &RepoSlug, id: RunId) -> Self {
		Self(format!("{}#artifacts", Self::run(server, repo, id).0))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

/// The hidden first line of the sticky comment. Reading one back is strict: any other shape is
/// not ours, so a user's comment with a look-alike marker is never mistaken for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
	pub key: CommentKey,
	pub sha: Sha,
	pub run: RunId,
}

impl Marker {
	/// Reads the first line of `body`. The line must be exactly what [`Marker::render`] writes,
	/// except that a line ending of `\r\n` is accepted, because a comment edited in a browser may
	/// come back with one.
	pub fn parse(body: &str) -> Option<Self> {
		let line = body.split('\n').next()?;
		let line = line.strip_suffix('\r').unwrap_or(line);
		let rest = line.strip_prefix(MARKER_PREFIX)?;
		let (key, rest) = rest.split_once(" sha=")?;
		let (sha, rest) = rest.split_once(" run=")?;
		let run = rest.strip_suffix(" -->")?;
		let canonical = !run.is_empty()
			&& run.len() <= 20
			&& run.bytes().all(|b| b.is_ascii_digit())
			&& (run == "0" || !run.starts_with('0'));
		if !canonical {
			return None;
		}
		Some(Self {
			key: CommentKey::parse(key)?,
			sha: Sha::parse(sha)?,
			run: RunId(run.parse().ok()?),
		})
	}

	pub fn render(&self) -> Safe {
		Safe::from_checked(format!(
			"{MARKER_PREFIX}{} sha={} run={} -->",
			self.key.0, self.sha.0, self.run.0
		))
	}

	/// Whether `body` starts with the marker of `key`, whatever its hash and run: how the sticky
	/// comment is found. The caller also checks the comment's author.
	pub fn prefix_matches(body: &str, key: &CommentKey) -> bool {
		body.strip_prefix(MARKER_PREFIX)
			.and_then(|rest| rest.strip_prefix(key.0.as_str()))
			.is_some_and(|rest| rest.starts_with(' '))
	}
}

impl Safe {
	/// A Markdown link. The label is already `Safe` and the destination can only be a
	/// [`TrustedUrl`].
	pub fn link(url: &TrustedUrl, label: &Safe) -> Safe {
		Safe::from_checked(format!("[{label}]({})", url.0))
	}

	/// A count with a narrow no-break space (U+202F) between groups of three digits, as Canadian
	/// style allows: `1 978`. A thousand is the first to be grouped.
	pub fn grouped(n: u64) -> Safe {
		let digits = n.to_string();
		let len = digits.len();
		let mut out = String::new();
		for (i, c) in digits.chars().enumerate() {
			if i > 0 && (len - i) % 3 == 0 {
				out.push('\u{202f}');
			}
			out.push(c);
		}
		Safe::from_checked(out)
	}

	/// A duration read at a glance: `0.4 s`, `18.2 s`, `1 m 47 s`, `2 h 5 m`. Each is rounded to
	/// its last unit, half up.
	pub fn duration_ms(ms: u64) -> Safe {
		let tenths = ms.saturating_add(50) / 100;
		let text = if tenths < 600 {
			format!("{}.{} s", tenths / 10, tenths % 10)
		} else {
			let secs = ms.saturating_add(500) / 1000;
			if secs < 3600 {
				format!("{} m {} s", secs / 60, secs % 60)
			} else {
				let mins = ms.saturating_add(30_000) / 60_000;
				format!("{} h {} m", mins / 60, mins % 60)
			}
		};
		Safe::from_checked(text)
	}

	/// The first seven digits of a commit hash, in a code span.
	pub fn sha_short(sha: &Sha) -> Safe {
		Safe::from_checked(format!("`{}`", sha.short()))
	}

	/// A repository path, in a code span.
	pub fn path(p: &RepoPath) -> Safe {
		Safe::code_span(p, MAX_PATH_BYTES)
	}

	/// Everything after the first line, which for a comment is the hidden marker.
	pub fn after_first_line(&self) -> Safe {
		let text = self.as_str();
		Safe::from_checked(
			text.split_once('\n')
				.map(|(_, rest)| rest.to_owned())
				.unwrap_or_default(),
		)
	}

	/// The first `n` lines, each with the line break that ends it (the last one only if the text
	/// has one there). A cut at a line boundary, so no escape sequence is split.
	pub fn first_lines(&self, n: usize) -> Safe {
		Safe::from_checked(self.as_str().split_inclusive('\n').take(n).collect())
	}

	/// A run of `n` tildes, at most 65 536: the line that closes a tilde code fence of that length.
	pub fn tildes(n: usize) -> Safe {
		Safe::from_checked("~".repeat(n.min(65_536)))
	}

	/// A run of `n` backticks, at most 65 536: the line that closes a code fence of that length.
	pub fn backticks(n: usize) -> Safe {
		Safe::from_checked("`".repeat(n.min(65_536)))
	}
}
