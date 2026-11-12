// The settings file: `.github/pr-results.toml`, read strictly and turned into a publish plan.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The file is trusted (it is read from the default branch), but it is still read strictly: an
//! unknown key, a value of the wrong type or a number out of range is an error that names the
//! key and the line, so a typo never changes behaviour quietly. A setting that names a feature
//! this version does not have is an error too, rather than a setting that does nothing.

use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use pr_results_core::{CommentKey, Flavour, Glob, RepoPath};
use pr_results_forge::{BlobUrl, HostKind};
use pr_results_render::{Options, Symbols};
use toml::de::{DeArray, DeTable, DeValue};
use toml::Spanned;

/// The most characters a title, a suite name or a check name may have.
const MAX_NAME_CHARS: usize = 100;
const MAX_SUITES: usize = 50;
const MAX_FILE_GLOBS: usize = 32;

/// What is wrong with a settings file, and on which line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
	pub line: Option<usize>,
	pub message: String,
}

impl fmt::Display for ConfigError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.line {
			Some(line) => write!(f, "line {line}: {}", self.message),
			None => f.write_str(&self.message),
		}
	}
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentMode {
	Always,
	Off,
}

#[derive(Debug, Clone)]
pub struct SuiteSettings {
	pub name: String,
	pub check: Option<String>,
	pub artifact: Glob,
	pub files: Vec<Glob>,
	pub flavour: Flavour,
	pub required: bool,
	/// Put in front of every path the suite's reports name, so annotations land on repository
	/// paths when the reporter's root is a subdirectory.
	pub path_prefix: Option<RepoPath>,
}

#[derive(Debug, Clone)]
pub struct Limits {
	pub max_artifact_bytes: u64,
	pub max_files: usize,
	pub max_file_bytes: u64,
	pub max_cases: usize,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			max_artifact_bytes: 100 * 1024 * 1024,
			max_files: 500,
			max_file_bytes: 50 * 1024 * 1024,
			max_cases: 200_000,
		}
	}
}

/// `[forge]`: which forge the tool talks to and how to reach it. Every key is optional, and the
/// command line's `--forge` and `--bot-login` override `kind` and `bot_login`.
#[derive(Debug, Clone, Default)]
pub struct ForgeSettings {
	/// Explicitly permit HTTP for Forgejo on a trusted network.
	pub allow_http: bool,
	/// As written; `None` is GitHub.
	pub kind: Option<HostKind>,
	/// Confirms the API address, which must be on the runner's own host. It never moves the token.
	pub api_url: Option<String>,
	/// A PEM file of the certificate authorities to trust instead of the built-in roots.
	pub ca_file: Option<std::path::PathBuf>,
	/// Where GitHub Enterprise Server keeps artefacts, when not on its own host. GitHub only.
	pub blob_hosts: Vec<String>,
	/// The account whose comments are ours, when the token is a bot account's personal token.
	pub bot_login: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Settings {
	/// The heading and the prefix of default check names. `None` is the repository's name in
	/// title case, known only when publishing.
	pub title: Option<String>,
	pub key: CommentKey,
	pub mode: CommentMode,
	pub max_failures: usize,
	pub excerpt_lines: usize,
	pub symbols: Symbols,
	pub max_annotations: usize,
	pub flaky_annotations: bool,
	pub limits: Limits,
	pub forge: ForgeSettings,
	/// Empty means: infer one suite from each `*-test-results*` artefact.
	pub suites: Vec<SuiteSettings>,
}

impl Default for Settings {
	fn default() -> Self {
		let options = Options::default();
		Self {
			title: None,
			key: default_key(),
			mode: CommentMode::Always,
			max_failures: options.max_failures,
			excerpt_lines: options.excerpt_lines,
			symbols: options.symbols,
			max_annotations: options.max_annotations,
			flaky_annotations: options.flaky_annotations,
			limits: Limits::default(),
			forge: ForgeSettings::default(),
			suites: Vec::new(),
		}
	}
}

/// The default comment key. `"ci"` is a literal that passes `CommentKey::parse`, which a test
/// checks, so this cannot fail.
#[allow(clippy::expect_used)]
fn default_key() -> CommentKey {
	CommentKey::parse("ci").expect("\"ci\" is a valid comment key")
}

impl Settings {
	/// The renderer's options for these settings.
	pub fn options(&self) -> Options {
		Options {
			max_failures: self.max_failures,
			excerpt_lines: self.excerpt_lines,
			symbols: self.symbols,
			max_annotations: self.max_annotations,
			flaky_annotations: self.flaky_annotations,
			..Options::default()
		}
	}

	/// Reads a settings file. Errors name the key and the line.
	pub fn parse(source: &str) -> Result<Self, ConfigError> {
		let reader = Reader { source };
		let root = DeTable::parse(source).map_err(|e| ConfigError {
			line: e.span().map(|s| reader.line(s.start)),
			message: e.message().to_owned(),
		})?;
		reader.settings(root.get_ref())
	}
}

/// Walks a parsed document and keeps the source, to turn a byte offset into a line.
struct Reader<'s> {
	source: &'s str,
}

type Table<'a> = DeTable<'a>;

impl Reader<'_> {
	fn line(&self, offset: usize) -> usize {
		let head = self.source.get(..offset).unwrap_or(self.source);
		1 + head.bytes().filter(|b| *b == b'\n').count()
	}

	fn err(&self, span: Range<usize>, message: impl Into<String>) -> ConfigError {
		ConfigError {
			line: Some(self.line(span.start)),
			message: message.into(),
		}
	}

	/// Every key of `table` must be one of `allowed`. `path` prefixes the key in the message.
	fn only(&self, table: &Table<'_>, allowed: &[&str], path: &str) -> Result<(), ConfigError> {
		for (key, _) in table.iter() {
			if !allowed.contains(&key.get_ref().as_ref()) {
				return Err(self.err(key.span(), format!("unknown key `{path}{}`", key.get_ref())));
			}
		}
		Ok(())
	}

	fn find<'t, 'a>(&self, table: &'t Table<'a>, name: &str) -> Option<&'t Spanned<DeValue<'a>>> {
		table
			.iter()
			.find(|(k, _)| k.get_ref().as_ref() == name)
			.map(|(_, v)| v)
	}

	fn string(
		&self,
		table: &Table<'_>,
		name: &str,
		path: &str,
	) -> Result<Option<(String, Range<usize>)>, ConfigError> {
		match self.find(table, name) {
			None => Ok(None),
			Some(v) => match v.get_ref() {
				DeValue::String(s) => Ok(Some((s.to_string(), v.span()))),
				_ => Err(self.err(v.span(), format!("`{path}{name}` must be a string"))),
			},
		}
	}

	fn boolean(
		&self,
		table: &Table<'_>,
		name: &str,
		path: &str,
	) -> Result<Option<bool>, ConfigError> {
		match self.find(table, name) {
			None => Ok(None),
			Some(v) => match v.get_ref() {
				DeValue::Boolean(b) => Ok(Some(*b)),
				_ => Err(self.err(v.span(), format!("`{path}{name}` must be true or false"))),
			},
		}
	}

	/// A whole number from `min` to `max`.
	fn number(
		&self,
		table: &Table<'_>,
		name: &str,
		path: &str,
		min: u64,
		max: u64,
	) -> Result<Option<u64>, ConfigError> {
		let Some(v) = self.find(table, name) else {
			return Ok(None);
		};
		let DeValue::Integer(i) = v.get_ref() else {
			return Err(self.err(v.span(), format!("`{path}{name}` must be a whole number")));
		};
		match u64::from_str_radix(i.as_str(), i.radix()) {
			Ok(n) if (min..=max).contains(&n) => Ok(Some(n)),
			_ => Err(self.err(
				v.span(),
				format!("`{path}{name}` must be a whole number from {min} to {max}"),
			)),
		}
	}

	fn subtable<'t, 'a>(
		&self,
		table: &'t Table<'a>,
		name: &str,
	) -> Result<Option<&'t Table<'a>>, ConfigError> {
		match self.find(table, name) {
			None => Ok(None),
			Some(v) => match v.get_ref() {
				DeValue::Table(t) => Ok(Some(t)),
				_ => Err(self.err(v.span(), format!("`{name}` must be a table"))),
			},
		}
	}

	fn name(&self, text: &str, span: Range<usize>, what: &str) -> Result<String, ConfigError> {
		let count = text.chars().count();
		if text.trim().is_empty() || count > MAX_NAME_CHARS || text.chars().any(char::is_control) {
			return Err(self.err(
				span,
				format!("{what} must be 1 to {MAX_NAME_CHARS} printable characters"),
			));
		}
		Ok(text.to_owned())
	}

	fn settings(&self, root: &Table<'_>) -> Result<Settings, ConfigError> {
		self.only(
			root,
			&[
				"schema",
				"title",
				"comment",
				"checks",
				"annotations",
				"baseline",
				"limits",
				"forge",
				"suite",
			],
			"",
		)?;
		let mut s = Settings::default();

		if let Some(v) = self.find(root, "schema") {
			if !matches!(self.number(root, "schema", "", 1, 1), Ok(Some(1))) {
				return Err(self.err(v.span(), "`schema` must be 1"));
			}
		}
		if let Some((t, span)) = self.string(root, "title", "")? {
			s.title = Some(self.name(&t, span, "`title`")?);
		}

		if let Some(t) = self.subtable(root, "comment")? {
			self.comment(t, &mut s)?;
		}
		if let Some(t) = self.subtable(root, "checks")? {
			self.checks(t)?;
		}
		if let Some(t) = self.subtable(root, "annotations")? {
			self.only(t, &["max_per_suite", "flaky"], "annotations.")?;
			if let Some(n) = self.number(t, "max_per_suite", "annotations.", 0, 500)? {
				s.max_annotations = n as usize;
			}
			if let Some(b) = self.boolean(t, "flaky", "annotations.")? {
				s.flaky_annotations = b;
			}
		}
		if let Some(t) = self.subtable(root, "baseline")? {
			// Checked so that a typo is caught now. Comparing with the base branch is planned for
			// v0.3, so these have no effect yet.
			self.only(
				t,
				&["branch", "workflow", "slower_percent", "slower_min_seconds"],
				"baseline.",
			)?;
			self.string(t, "branch", "baseline.")?;
			self.string(t, "workflow", "baseline.")?;
			self.number(t, "slower_percent", "baseline.", 0, 10_000)?;
			self.number(t, "slower_min_seconds", "baseline.", 0, 86_400)?;
		}
		if let Some(t) = self.subtable(root, "limits")? {
			self.limits(t, &mut s.limits)?;
		}
		if let Some(t) = self.subtable(root, "forge")? {
			s.forge = self.forge(t)?;
		}
		if let Some(v) = self.find(root, "suite") {
			s.suites = self.suites(v)?;
		}
		self.distinct_check_names(&s, root)?;
		Ok(s)
	}

	fn comment(&self, t: &Table<'_>, s: &mut Settings) -> Result<(), ConfigError> {
		self.only(
			t,
			&[
				"key",
				"mode",
				"max_failures",
				"excerpt_lines",
				"symbols",
				"other_checks",
			],
			"comment.",
		)?;
		if let Some((k, span)) = self.string(t, "key", "comment.")? {
			s.key = CommentKey::parse(&k).ok_or_else(|| {
				self.err(
					span,
					"`comment.key` must be 1 to 32 characters of a to z, 0 to 9 and -, not starting with -",
				)
			})?;
		}
		if let Some((m, span)) = self.string(t, "mode", "comment.")? {
			s.mode = match m.as_str() {
				"always" => CommentMode::Always,
				"off" => CommentMode::Off,
				"changes" | "failures" => {
					return Err(self.err(
						span,
						"`comment.mode` of \"changes\" and \"failures\" arrives in v0.3: use \"always\" or \"off\"",
					))
				}
				_ => {
					return Err(self.err(
						span,
						"`comment.mode` must be \"always\", \"changes\", \"failures\" or \"off\"",
					))
				}
			};
		}
		if let Some(n) = self.number(t, "max_failures", "comment.", 0, 1000)? {
			s.max_failures = n as usize;
		}
		if let Some(n) = self.number(t, "excerpt_lines", "comment.", 0, 1000)? {
			s.excerpt_lines = n as usize;
		}
		if let Some((v, span)) = self.string(t, "symbols", "comment.")? {
			s.symbols = match v.as_str() {
				"text" => Symbols::Text,
				"glyphs" => Symbols::Glyphs,
				_ => return Err(self.err(span, "`comment.symbols` must be \"text\" or \"glyphs\"")),
			};
		}
		if let (Some(v), Some(true)) = (
			self.find(t, "other_checks"),
			self.boolean(t, "other_checks", "comment.")?,
		) {
			return Err(self.err(
				v.span(),
				"`comment.other_checks` arrives in v0.3: leave it false",
			));
		}
		Ok(())
	}

	fn checks(&self, t: &Table<'_>) -> Result<(), ConfigError> {
		self.only(t, &["per_suite", "combined"], "checks.")?;
		if let (Some(v), Some(false)) = (
			self.find(t, "per_suite"),
			self.boolean(t, "per_suite", "checks.")?,
		) {
			return Err(self.err(
				v.span(),
				"`checks.per_suite = false` arrives with the combined check in v0.3: leave it true",
			));
		}
		if let Some((c, span)) = self.string(t, "combined", "checks.")? {
			if !c.is_empty() {
				return Err(self.err(span, "`checks.combined` arrives in v0.3: leave it empty"));
			}
		}
		Ok(())
	}

	fn forge(&self, t: &Table<'_>) -> Result<ForgeSettings, ConfigError> {
		self.only(
			t,
			&[
				"kind",
				"api_url",
				"ca_file",
				"blob_hosts",
				"bot_login",
				"allow_http",
			],
			"forge.",
		)?;
		let mut f = ForgeSettings {
			allow_http: self.boolean(t, "allow_http", "forge.")?.unwrap_or(false),
			..ForgeSettings::default()
		};
		if let Some((k, span)) = self.string(t, "kind", "forge.")? {
			f.kind =
				Some(HostKind::from_name(&k).ok_or_else(|| {
					self.err(span, "`forge.kind` must be \"github\" or \"forgejo\"")
				})?);
		}
		if let Some((url, span)) = self.string(t, "api_url", "forge.")? {
			if url.starts_with("http://") && !f.allow_http {
				return Err(self.err(
					span,
					"`forge.api_url` is plain http, which is refused: the token would cross the network in clear",
				));
			}
			let candidate = url
				.strip_prefix("http://")
				.map(|rest| format!("https://{rest}"))
				.unwrap_or_else(|| url.clone());
			let usable = BlobUrl::parse(&candidate).is_some() && !url.contains(['?', '#']);
			if !usable {
				return Err(self.err(
					span,
					"`forge.api_url` must be an https:// address with no user information, query or fragment",
				));
			}
			f.api_url = Some(url);
		}
		if let Some((path, span)) = self.string(t, "ca_file", "forge.")? {
			if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
				return Err(self.err(span, "`forge.ca_file` must be the path of a PEM file"));
			}
			f.ca_file = Some(std::path::PathBuf::from(path));
		}
		if let Some(v) = self.find(t, "blob_hosts") {
			let DeValue::Array(items) = v.get_ref() else {
				return Err(self.err(v.span(), "`forge.blob_hosts` must be a list of host names"));
			};
			if items.len() > 16 {
				return Err(self.err(v.span(), "`forge.blob_hosts` lists more than 16 hosts"));
			}
			for item in items.iter() {
				let DeValue::String(h) = item.get_ref() else {
					return Err(self.err(item.span(), "`forge.blob_hosts` must hold strings"));
				};
				if !valid_blob_host(h) {
					return Err(self.err(
						item.span(),
						"`forge.blob_hosts` holds a name that is not a host name or `*.suffix`",
					));
				}
				f.blob_hosts.push(h.to_ascii_lowercase());
			}
			if !f.blob_hosts.is_empty() && f.kind == Some(HostKind::Forgejo) {
				return Err(self.err(
					v.span(),
					"`forge.blob_hosts` is for GitHub Enterprise Server and is refused for forgejo, which has no blobs",
				));
			}
		}
		if let Some((login, span)) = self.string(t, "bot_login", "forge.")? {
			if !valid_login(&login) {
				return Err(self.err(span, "`forge.bot_login` is not a login"));
			}
			f.bot_login = Some(login);
		}
		Ok(f)
	}

	fn limits(&self, t: &Table<'_>, l: &mut Limits) -> Result<(), ConfigError> {
		self.only(
			t,
			&[
				"max_artifact_bytes",
				"max_files",
				"max_file_bytes",
				"max_cases",
			],
			"limits.",
		)?;
		const GIB: u64 = 1024 * 1024 * 1024;
		if let Some(n) = self.number(t, "max_artifact_bytes", "limits.", 1, GIB)? {
			l.max_artifact_bytes = n;
		}
		if let Some(n) = self.number(t, "max_files", "limits.", 1, 10_000)? {
			l.max_files = n as usize;
		}
		if let Some(n) = self.number(t, "max_file_bytes", "limits.", 1, 256 * 1024 * 1024)? {
			l.max_file_bytes = n;
		}
		if let Some(n) = self.number(t, "max_cases", "limits.", 1, 1_000_000)? {
			l.max_cases = n as usize;
		}
		Ok(())
	}

	fn suites(&self, v: &Spanned<DeValue<'_>>) -> Result<Vec<SuiteSettings>, ConfigError> {
		let DeValue::Array(items) = v.get_ref() else {
			return Err(self.err(v.span(), "`suite` must be an array of tables, `[[suite]]`"));
		};
		let items: &DeArray<'_> = items;
		if items.len() > MAX_SUITES {
			return Err(self.err(v.span(), format!("there are more than {MAX_SUITES} suites")));
		}
		let mut out = Vec::new();
		for item in items.iter() {
			let DeValue::Table(t) = item.get_ref() else {
				return Err(self.err(item.span(), "`suite` must be an array of tables"));
			};
			out.push(self.suite(t, item.span())?);
		}
		Ok(out)
	}

	fn suite(&self, t: &Table<'_>, at: Range<usize>) -> Result<SuiteSettings, ConfigError> {
		self.only(
			t,
			&[
				"name",
				"check",
				"artifact",
				"files",
				"format",
				"flavour",
				"required",
				"path_prefix",
			],
			"suite.",
		)?;
		let Some((name, span)) = self.string(t, "name", "suite.")? else {
			return Err(self.err(at, "a `[[suite]]` needs a `name`"));
		};
		let name = self.name(&name, span, "`suite.name`")?;
		let check = match self.string(t, "check", "suite.")? {
			Some((c, span)) => Some(self.name(&c, span, "`suite.check`")?),
			None => None,
		};
		let Some((artifact, span)) = self.string(t, "artifact", "suite.")? else {
			return Err(self.err(at, "a `[[suite]]` needs an `artifact`"));
		};
		let artifact = Glob::new(&artifact).map_err(|e| {
			self.err(
				span,
				format!("`suite.artifact` is not a usable pattern: {e}"),
			)
		})?;
		let files = match self.find(t, "files") {
			None => vec![self.default_glob()?],
			Some(v) => self.globs(v)?,
		};
		if let Some((f, span)) = self.string(t, "format", "suite.")? {
			match f.as_str() {
				"junit" => {}
				"steps-times" | "steps-json" => {
					return Err(self.err(
						span,
						"`suite.format` of \"steps-times\" and \"steps-json\" is not supported: only \"junit\"",
					))
				}
				_ => return Err(self.err(span, "`suite.format` must be \"junit\"")),
			}
		}
		let flavour = match self.string(t, "flavour", "suite.")? {
			None => Flavour::Auto,
			Some((f, span)) => Flavour::from_name(&f).ok_or_else(|| {
				self.err(
					span,
					"`suite.flavour` must be auto, nextest, vitest, bun, jest, playwright, steps or generic",
				)
			})?,
		};
		let required = self.boolean(t, "required", "suite.")?.unwrap_or(false);
		let path_prefix = match self.string(t, "path_prefix", "suite.")? {
			Some((p, span)) if !p.is_empty() => Some(RepoPath::parse(&p).ok_or_else(|| {
				self.err(
					span,
					"`suite.path_prefix` must be a relative path inside the repository, such as \"apps/web/\"",
				)
			})?),
			_ => None,
		};
		Ok(SuiteSettings {
			name,
			check,
			artifact,
			files,
			flavour,
			required,
			path_prefix,
		})
	}

	fn default_glob(&self) -> Result<Glob, ConfigError> {
		Glob::new("**/*.xml").map_err(|e| ConfigError {
			line: None,
			message: e.to_string(),
		})
	}

	fn globs(&self, v: &Spanned<DeValue<'_>>) -> Result<Vec<Glob>, ConfigError> {
		let DeValue::Array(items) = v.get_ref() else {
			return Err(self.err(v.span(), "`suite.files` must be a list of patterns"));
		};
		if items.is_empty() || items.len() > MAX_FILE_GLOBS {
			return Err(self.err(
				v.span(),
				format!("`suite.files` must list 1 to {MAX_FILE_GLOBS} patterns"),
			));
		}
		let mut out = Vec::new();
		for item in items.iter() {
			let DeValue::String(s) = item.get_ref() else {
				return Err(self.err(item.span(), "`suite.files` must hold strings"));
			};
			out.push(Glob::new(s).map_err(|e| {
				self.err(
					item.span(),
					format!("`suite.files` holds a pattern that cannot be used: {e}"),
				)
			})?);
		}
		Ok(out)
	}

	/// Two suites with one check name would write one check run twice.
	fn distinct_check_names(&self, s: &Settings, root: &Table<'_>) -> Result<(), ConfigError> {
		let mut seen = BTreeSet::new();
		for suite in &s.suites {
			let name = suite.check.clone().unwrap_or_else(|| {
				default_check_name(s.title.as_deref().unwrap_or(""), &suite.name)
			});
			if !seen.insert(name) {
				let span = self.find(root, "suite").map_or(0..0, Spanned::span);
				return Err(self.err(
					span,
					"two suites have the same check name: give one a different `check`",
				));
			}
		}
		Ok(())
	}
}

/// An account login as either forge writes one: letters, digits, `-`, `_` and `.`, and the `[`
/// and `]` of a GitHub app's `name[bot]`.
pub fn valid_login(login: &str) -> bool {
	!login.is_empty()
		&& login.len() <= 64
		&& login
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'[' | b']'))
}

/// A host name, or `*.suffix` for any name under it: letters, digits, `-` and `.`.
fn valid_blob_host(h: &str) -> bool {
	let name = h.strip_prefix("*.").unwrap_or(h);
	!name.is_empty()
		&& name.len() <= 253
		&& !name.starts_with(['.', '-'])
		&& !name.ends_with(['.', '-'])
		&& !name.contains("..")
		&& name
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
}

/// `<title> <suite name>`, or the suite's name alone when there is no title yet.
pub fn default_check_name(title: &str, suite: &str) -> String {
	if title.is_empty() {
		suite.to_owned()
	} else {
		format!("{title} {suite}")
	}
}

/// The repository's name in title case: `desk` is "Desk" and `pr-results` is "Pr Results".
pub fn title_from_repo(name: &str) -> String {
	let words: Vec<String> = name
		.split(['-', '_', '.'])
		.filter(|w| !w.is_empty())
		.map(|w| {
			let mut chars = w.chars();
			match chars.next() {
				Some(first) => first.to_uppercase().chain(chars).collect(),
				None => String::new(),
			}
		})
		.collect();
	if words.is_empty() {
		"Test results".to_owned()
	} else {
		words.join(" ")
	}
}

/// The name for a suite inferred from an artefact: `rust-test-results` is "Rust tests".
pub fn suite_name_from_stem(stem: &str) -> String {
	let words = title_from_repo(stem).to_lowercase();
	let mut chars = words.chars();
	let head: String = chars
		.next()
		.map(|c| c.to_uppercase().collect())
		.unwrap_or_default();
	format!("{head}{} tests", chars.as_str())
}

/// The settings file is read to at most this many bytes.
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// The settings from `path`, or the defaults when `path` is `None`. A message for a person, which
/// names the file and, for a mistake in it, the line.
pub fn read(path: Option<&std::path::Path>) -> Result<Settings, String> {
	let Some(path) = path else {
		return Ok(Settings::default());
	};
	let shown = path.display();
	let file = std::fs::File::open(path).map_err(|_| format!("{shown}: cannot be opened"))?;
	let mut bytes = Vec::new();
	std::io::Read::read_to_end(
		&mut std::io::Read::take(file, MAX_CONFIG_BYTES + 1),
		&mut bytes,
	)
	.map_err(|_| format!("{shown}: cannot be read"))?;
	if bytes.len() as u64 > MAX_CONFIG_BYTES {
		return Err(format!("{shown}: is larger than 1 MiB"));
	}
	let text = String::from_utf8(bytes).map_err(|_| format!("{shown}: is not UTF-8"))?;
	Settings::parse(&text).map_err(|e| format!("{shown}: {e}"))
}
