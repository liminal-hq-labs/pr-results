// Anonymizes a JUnit file: `anonymize_junit (--salt TEXT | --random-salt) IN.xml OUT.xml`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The command behind `scripts/anonymize-junit.sh`. It reads one file and writes one file; the
//! rules are in `pr_results_core::anonymize_junit` and `docs/anonymizing-reports.md`. A salt is
//! required, because the tokens are only as hard to guess as the salt is: `--salt` takes one
//! (sixteen bytes or more, or a warning follows) and `--random-salt` makes one and prints it.

use std::io::Read;
use std::process::ExitCode;

/// The shortest salt that does not draw a warning.
const GOOD_SALT_BYTES: usize = 16;

fn main() -> ExitCode {
	let mut args = std::env::args().skip(1);
	let mut salt: Option<String> = None;
	let mut random = false;
	let mut files = Vec::new();
	while let Some(arg) = args.next() {
		match arg.as_str() {
			"--salt" => match args.next() {
				Some(value) if salt.is_none() => salt = Some(value),
				_ => return usage(),
			},
			"--random-salt" => random = true,
			_ => files.push(arg),
		}
	}
	let [input, output] = files.as_slice() else {
		return usage();
	};
	let salt = match (salt, random) {
		(Some(salt), false) => salt,
		(None, true) => match random_salt() {
			Ok(salt) => {
				eprintln!("anonymize-junit: salt {salt} (keep it private; the same salt gives the same output)");
				salt
			}
			Err(e) => {
				eprintln!("anonymize-junit: cannot read /dev/urandom: {e}");
				return ExitCode::from(2);
			}
		},
		_ => return usage(),
	};
	if salt.len() < GOOD_SALT_BYTES {
		eprintln!(
			"anonymize-junit: warning: the salt is {} bytes; use {GOOD_SALT_BYTES} or more (--random-salt makes one), or the tokens can be guessed",
			salt.len()
		);
	}
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

/// Sixteen bytes from the system, as thirty-two hex digits.
fn random_salt() -> std::io::Result<String> {
	let mut bytes = [0u8; 16];
	std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
	Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn usage() -> ExitCode {
	eprintln!("usage: anonymize_junit (--salt TEXT | --random-salt) IN.xml OUT.xml");
	eprintln!("a salt is required: the tokens are only as hard to guess as it is");
	ExitCode::from(2)
}
