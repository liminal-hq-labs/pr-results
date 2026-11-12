// The pr-results command line. Only `--version` and `--help` exist so far.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]

use std::process::ExitCode;

use pr_results_core::Safe;

const HELP: &str = "\
pr-results: calm test and check results for pull requests

This build can read JUnit files and nothing more: it does not post anything yet.

Usage:
  pr-results --version
  pr-results --help
";

fn main() -> ExitCode {
	let mut args = std::env::args().skip(1);
	match args.next().as_deref() {
		Some("--version" | "-V") => {
			println!("pr-results {}", env!("CARGO_PKG_VERSION"));
			ExitCode::SUCCESS
		}
		Some("--help" | "-h") | None => {
			print!("{HELP}");
			ExitCode::SUCCESS
		}
		Some(other) => {
			// The argument goes to a log that a runner may read for commands, so it passes
			// through `Safe` like any other untrusted text.
			eprintln!(
				"pr-results: unknown argument: {}",
				Safe::log_line(other, 80)
			);
			eprintln!("Try 'pr-results --help'.");
			ExitCode::from(2)
		}
	}
}
