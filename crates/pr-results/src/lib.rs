// The pr-results command line as a library, so the commands run in tests without a process.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `publish`, `render` and `validate-config`. The exit codes are the same everywhere:
//!
//! - `0`: it published (even when tests failed), rendered, validated, or had nothing to do.
//! - `1`: publishing itself failed, and the message says what.
//! - `2`: the arguments, the settings file or the environment are wrong.
//! - `4`: tests failed and `--fail-on-failures` was given.
//!
//! Every line written to the log passes through [`Safe::log_line`], so a name that reaches it
//! cannot start a workflow command.

#![forbid(unsafe_code)]

pub mod args;
pub mod config;
pub mod event;
mod publish_cmd;
mod render_cmd;

use std::io::Write;

use pr_results_core::Safe;
use pr_results_forge::{CaBundle, Forge, ForgeError, HostKind, Token, UreqForge};

use args::Command;

/// The version written in every footer.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
pr-results: calm test and check results for pull requests

Usage:
  pr-results publish [--config PATH] [--dry-run DIR] [--token-env NAME]
                     [--bot-login LOGIN] [--forge github|forgejo] [--fail-on-failures]
  pr-results render [--config PATH] [--title TEXT] [--flavour NAME] FILE... [--out DIR]
  pr-results validate-config PATH
  pr-results --version
  pr-results --help

publish   Post the results of a finished CI run, from a workflow_run workflow on the default
          branch: one check run for each suite and one sticky comment on the pull request.
          --dry-run DIR writes requests.json, comment.md and checks/ instead of posting.
          --forge forgejo writes a commit status for each suite instead of a check run
          (overrides [forge] kind).
          Exit status: 0 published or nothing to do, 1 publishing failed, 2 usage,
          settings or environment, 4 tests failed (with --fail-on-failures).
render    Turn local JUnit files into the comment and check Markdown, with no network.
          Without --out the comment is printed. With --out DIR it writes comment.md and
          checks/N.md.
validate-config
          Check a settings file and exit 0 if it is valid.

The settings file is .github/pr-results.toml. See SPEC.md for every setting.
";

/// Where the commands write. Tests pass buffers.
pub struct Streams<'a> {
	pub out: &'a mut dyn Write,
	pub err: &'a mut dyn Write,
}

impl Streams<'_> {
	/// A line on standard output. It passes through `Safe::log_line`, because the runner reads
	/// standard output for workflow commands.
	pub(crate) fn say(&mut self, line: &str) {
		let _ = writeln!(self.out, "{}", Safe::log_line(line, 400));
	}

	/// A workflow command the tool itself writes, such as `::add-mask::`. Never untrusted text.
	pub(crate) fn command(&mut self, line: &str) {
		let _ = writeln!(self.out, "{line}");
	}

	/// A block of Markdown on standard output. A line that would start a workflow command or a
	/// runner directive (`::` or `##[`) is shown with a backslash and a space in front, so what a
	/// report holds cannot run one. The files the tool writes are exact.
	pub(crate) fn block(&mut self, text: &str) {
		for line in text.lines() {
			let start = line.trim_start();
			if start.starts_with("::") || start.starts_with("##[") {
				let _ = write!(self.out, "\\ ");
			}
			let _ = writeln!(self.out, "{line}");
		}
	}

	/// A line on standard error, passed through `Safe::log_line` because it may quote something
	/// a person or a report chose.
	pub(crate) fn complain(&mut self, line: &str) {
		let _ = writeln!(self.err, "pr-results: {}", Safe::log_line(line, 400));
	}
}

/// What stops a command, with the exit status that goes with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CliError {
	pub code: u8,
	pub message: String,
}

impl CliError {
	/// Arguments, settings or environment: exit 2.
	pub(crate) fn usage(message: impl Into<String>) -> Self {
		Self {
			code: 2,
			message: message.into(),
		}
	}

	/// Publishing itself failed: exit 1.
	pub(crate) fn failed(message: impl Into<String>) -> Self {
		Self {
			code: 1,
			message: message.into(),
		}
	}
}

/// What `publish` decided about the forge before any request: which one, where its API is, and
/// how to trust its certificate.
#[derive(Debug, Clone)]
pub struct ForgeSetup {
	pub kind: HostKind,
	/// An `https://` URL on the runner's own host, checked.
	pub api_base: String,
	/// The roots to trust instead of the webpki roots, from `[forge] ca_file`.
	pub ca: Option<CaBundle>,
	/// The hosts a GitHub artefact may be fetched from, from `[forge] blob_hosts`.
	pub blob_hosts: Option<Vec<String>>,
}

/// How `publish` gets a forge: the network in `main`, an in-memory forge in tests.
pub trait ForgeFactory {
	type Forge: Forge;

	fn make(&self, setup: &ForgeSetup, token: Token) -> Result<Self::Forge, ForgeError>;
}

/// The real client.
pub struct Network;

impl ForgeFactory for Network {
	type Forge = UreqForge;

	fn make(&self, setup: &ForgeSetup, token: Token) -> Result<UreqForge, ForgeError> {
		let forge = match setup.kind {
			HostKind::GitHub => UreqForge::new(&setup.api_base, token)?,
			HostKind::Forgejo => UreqForge::forgejo(&setup.api_base, token)?,
		};
		let forge = match &setup.ca {
			Some(ca) => forge.with_ca(ca),
			None => forge,
		};
		Ok(match &setup.blob_hosts {
			Some(hosts) => {
				let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
				forge.with_blob_hosts(&hosts)
			}
			None => forge,
		})
	}
}

/// Runs the command line. `vars` reads the environment, so tests need not set any.
pub fn run<X: ForgeFactory>(
	arguments: &[String],
	vars: &dyn Fn(&str) -> Option<String>,
	factory: &X,
	io: &mut Streams<'_>,
) -> u8 {
	let command = match args::parse(arguments) {
		Ok(c) => c,
		Err(e) => {
			io.complain(&e.0);
			let _ = writeln!(io.err, "Try 'pr-results --help'.");
			return 2;
		}
	};
	let result = match command {
		Command::Help => {
			let _ = write!(io.out, "{HELP}");
			return 0;
		}
		Command::Version => {
			io.say(&format!("pr-results {VERSION}"));
			return 0;
		}
		Command::Publish(a) => publish_cmd::publish_command(&a, vars, factory, io),
		Command::Render(a) => render_cmd::render(&a, io),
		Command::ValidateConfig(path) => render_cmd::validate(&path, io),
	};
	match result {
		Ok(code) => code,
		Err(e) => {
			io.complain(&e.message);
			e.code
		}
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;

	#[test]
	fn the_default_comment_key_parses() {
		assert_eq!(
			config::Settings::default().key,
			pr_results_core::CommentKey::parse("ci").unwrap()
		);
	}
}
