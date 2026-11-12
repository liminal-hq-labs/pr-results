// The command line, read by hand: three commands and a handful of flags need no parser crate.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use pr_results_core::Flavour;

/// The environment variable that holds the token unless `--token-env` names another.
pub const DEFAULT_TOKEN_ENV: &str = "GITHUB_TOKEN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
	Help,
	Version,
	Publish(PublishArgs),
	Render(RenderArgs),
	ValidateConfig(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishArgs {
	pub config: Option<PathBuf>,
	pub dry_run: Option<PathBuf>,
	pub token_env: String,
	pub bot_login: Option<String>,
	pub fail_on_failures: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderArgs {
	pub config: Option<PathBuf>,
	pub title: Option<String>,
	pub flavour: Option<Flavour>,
	pub files: Vec<PathBuf>,
	pub out: Option<PathBuf>,
}

/// A mistake in the arguments. The text may quote an argument, so it is printed through
/// `Safe::log_line` like any other untrusted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

fn usage(message: impl Into<String>) -> UsageError {
	UsageError(message.into())
}

/// A flag's value: `--flag value` or `--flag=value`.
struct Cursor<'a> {
	args: &'a [String],
	next: usize,
}

impl<'a> Cursor<'a> {
	fn take(&mut self) -> Option<&'a str> {
		let arg = self.args.get(self.next)?;
		self.next += 1;
		Some(arg.as_str())
	}

	fn value(&mut self, flag: &str, inline: Option<&'a str>) -> Result<&'a str, UsageError> {
		match inline {
			Some(v) => Ok(v),
			None => self
				.take()
				.ok_or_else(|| usage(format!("{flag} needs a value"))),
		}
	}
}

/// Splits `--flag=value` into the flag and its value. Short flags and bare words are left whole.
fn split_flag(arg: &str) -> (&str, Option<&str>) {
	if arg.starts_with("--") {
		if let Some((flag, value)) = arg.split_once('=') {
			return (flag, Some(value));
		}
	}
	(arg, None)
}

pub fn parse(args: &[String]) -> Result<Command, UsageError> {
	let mut cursor = Cursor { args, next: 0 };
	match cursor.take() {
		None | Some("--help" | "-h" | "help") => Ok(Command::Help),
		Some("--version" | "-V") => Ok(Command::Version),
		Some("publish") => publish(&mut cursor),
		Some("render") => render(&mut cursor),
		Some("validate-config") => validate_config(&mut cursor),
		Some("collect") => Err(usage(
			"collect is not available yet: render and publish read the files themselves",
		)),
		Some(other) => Err(usage(format!("unknown command: {other}"))),
	}
}

fn publish(c: &mut Cursor<'_>) -> Result<Command, UsageError> {
	let mut a = PublishArgs {
		config: None,
		dry_run: None,
		token_env: DEFAULT_TOKEN_ENV.to_owned(),
		bot_login: None,
		fail_on_failures: false,
	};
	while let Some(arg) = c.take() {
		let (flag, inline) = split_flag(arg);
		match flag {
			"--config" => a.config = Some(PathBuf::from(c.value(flag, inline)?)),
			"--dry-run" => a.dry_run = Some(PathBuf::from(c.value(flag, inline)?)),
			"--token-env" => a.token_env = c.value(flag, inline)?.to_owned(),
			"--bot-login" => a.bot_login = Some(c.value(flag, inline)?.to_owned()),
			"--fail-on-failures" if inline.is_none() => a.fail_on_failures = true,
			"--help" | "-h" => return Ok(Command::Help),
			_ => return Err(usage(format!("unknown argument for publish: {arg}"))),
		}
	}
	Ok(Command::Publish(a))
}

fn render(c: &mut Cursor<'_>) -> Result<Command, UsageError> {
	let mut a = RenderArgs {
		config: None,
		title: None,
		flavour: None,
		files: Vec::new(),
		out: None,
	};
	let mut only_files = false;
	while let Some(arg) = c.take() {
		if only_files {
			a.files.push(PathBuf::from(arg));
			continue;
		}
		let (flag, inline) = split_flag(arg);
		match flag {
			"--" => only_files = true,
			"--config" => a.config = Some(PathBuf::from(c.value(flag, inline)?)),
			"--title" => a.title = Some(c.value(flag, inline)?.to_owned()),
			"--flavour" => {
				let name = c.value(flag, inline)?;
				a.flavour = Some(Flavour::from_name(name).ok_or_else(|| {
					usage(format!(
						"unknown flavour: {name} (auto, generic, nextest, steps, vitest, jest, bun or playwright)"
					))
				})?);
			}
			"--out" => a.out = Some(PathBuf::from(c.value(flag, inline)?)),
			"--help" | "-h" => return Ok(Command::Help),
			_ if arg.starts_with('-') && arg != "-" => {
				return Err(usage(format!("unknown argument for render: {arg}")))
			}
			_ => a.files.push(PathBuf::from(arg)),
		}
	}
	if a.files.is_empty() {
		return Err(usage("render needs at least one JUnit file"));
	}
	Ok(Command::Render(a))
}

fn validate_config(c: &mut Cursor<'_>) -> Result<Command, UsageError> {
	let mut path: Option<PathBuf> = None;
	while let Some(arg) = c.take() {
		match arg {
			"--help" | "-h" => return Ok(Command::Help),
			_ if arg.starts_with('-') => {
				return Err(usage(format!(
					"unknown argument for validate-config: {arg}"
				)))
			}
			_ if path.is_some() => {
				return Err(usage("validate-config takes one path"));
			}
			_ => path = Some(PathBuf::from(arg)),
		}
	}
	path.map(Command::ValidateConfig)
		.ok_or_else(|| usage("validate-config needs the path of a settings file"))
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

	use super::*;

	fn args(list: &[&str]) -> Vec<String> {
		list.iter().map(|s| (*s).to_owned()).collect()
	}

	#[test]
	fn no_arguments_is_help() {
		assert_eq!(parse(&[]).unwrap(), Command::Help);
	}

	#[test]
	fn a_flag_takes_its_value_either_way() {
		let a = parse(&args(&["publish", "--config", "x.toml", "--dry-run=out"])).unwrap();
		let Command::Publish(p) = a else {
			panic!("publish")
		};
		assert_eq!(p.config, Some(PathBuf::from("x.toml")));
		assert_eq!(p.dry_run, Some(PathBuf::from("out")));
		assert_eq!(p.token_env, "GITHUB_TOKEN");
		assert!(!p.fail_on_failures);
	}

	#[test]
	fn a_missing_value_is_named() {
		let e = parse(&args(&["publish", "--config"])).unwrap_err();
		assert_eq!(e.0, "--config needs a value");
	}

	#[test]
	fn render_collects_files_in_any_position() {
		let a = parse(&args(&[
			"render", "a.xml", "--out", "o", "b.xml", "--", "--odd",
		]))
		.unwrap();
		let Command::Render(r) = a else {
			panic!("render")
		};
		assert_eq!(
			r.files,
			vec![
				PathBuf::from("a.xml"),
				PathBuf::from("b.xml"),
				PathBuf::from("--odd")
			]
		);
		assert_eq!(r.out, Some(PathBuf::from("o")));
	}

	#[test]
	fn render_needs_a_file_and_a_known_flavour() {
		assert!(parse(&args(&["render"])).is_err());
		assert!(parse(&args(&["render", "--flavour", "mocha", "a.xml"])).is_err());
		assert!(parse(&args(&["render", "--bogus", "a.xml"])).is_err());
		let ok = parse(&args(&["render", "--flavour=nextest", "a.xml"])).unwrap();
		let Command::Render(r) = ok else {
			panic!("render")
		};
		assert_eq!(r.flavour, Some(Flavour::Nextest));
	}

	#[test]
	fn unknown_commands_and_flags_are_usage_errors() {
		assert!(parse(&args(&["post"])).is_err());
		assert!(parse(&args(&["publish", "--force"])).is_err());
		assert!(parse(&args(&["publish", "--fail-on-failures=1"])).is_err());
		assert!(parse(&args(&["collect"])).is_err());
		assert!(parse(&args(&["validate-config"])).is_err());
		assert!(parse(&args(&["validate-config", "a", "b"])).is_err());
	}
}
