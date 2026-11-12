// The pr-results command line: `publish`, `render` and `validate-config`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]

use std::process::ExitCode;

use pr_results::{run, Network, Streams};

fn main() -> ExitCode {
	let mut arguments = Vec::new();
	for arg in std::env::args_os().skip(1) {
		match arg.into_string() {
			Ok(text) => arguments.push(text),
			Err(_) => {
				eprintln!("pr-results: an argument is not valid UTF-8");
				return ExitCode::from(2);
			}
		}
	}
	let vars = |name: &str| std::env::var(name).ok();
	let mut stdout = std::io::stdout().lock();
	let mut stderr = std::io::stderr().lock();
	let mut io = Streams {
		out: &mut stdout,
		err: &mut stderr,
	};
	ExitCode::from(run(&arguments, &vars, &Network, &mut io))
}
