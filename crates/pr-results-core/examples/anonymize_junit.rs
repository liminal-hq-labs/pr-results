// Anonymizes a JUnit file: `anonymize_junit [--salt TEXT] IN.xml OUT.xml`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The command behind `scripts/anonymize-junit.sh`. It reads one file and writes one file; the
//! rules are in `pr_results_core::anonymize_junit` and `docs/anonymizing-reports.md`.

use std::process::ExitCode;

fn main() -> ExitCode {
	let mut args = std::env::args().skip(1);
	let mut salt = String::new();
	let mut files = Vec::new();
	while let Some(arg) = args.next() {
		if arg == "--salt" {
			match args.next() {
				Some(value) => salt = value,
				None => return usage(),
			}
		} else {
			files.push(arg);
		}
	}
	let [input, output] = files.as_slice() else {
		return usage();
	};
	let bytes = match std::fs::read(input) {
		Ok(bytes) => bytes,
		Err(e) => {
			eprintln!("anonymize-junit: {input}: {}", e.kind());
			return ExitCode::from(2);
		}
	};
	let text = match pr_results_core::anonymize_junit(&bytes, &salt) {
		Ok(text) => text,
		Err(e) => {
			eprintln!("anonymize-junit: {input}: {e}");
			return ExitCode::from(1);
		}
	};
	if let Err(e) = std::fs::write(output, text) {
		eprintln!("anonymize-junit: {output}: {}", e.kind());
		return ExitCode::from(2);
	}
	ExitCode::SUCCESS
}

fn usage() -> ExitCode {
	eprintln!("usage: anonymize_junit [--salt TEXT] IN.xml OUT.xml");
	ExitCode::from(2)
}
