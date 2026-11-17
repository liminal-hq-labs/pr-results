// `pr-results wrap` against real commands: what passes through, what is kept, and how it ends.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(unix)]
#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::{scratch, Factory, REPO};
use pr_results::{run, Streams};
use pr_results_forge::FakeForge;

struct Ran {
	code: u8,
	out: Vec<u8>,
	err: Vec<u8>,
	file: PathBuf,
}

impl Ran {
	fn xml(&self) -> String {
		std::fs::read_to_string(&self.file).unwrap()
	}
	fn out(&self) -> String {
		String::from_utf8_lossy(&self.out).into_owned()
	}
	fn err(&self) -> String {
		String::from_utf8_lossy(&self.err).into_owned()
	}
}

/// Runs `pr-results wrap --out <file> <flags> -- sh -c <script>` and returns what came of it.
fn sh(name: &str, flags: &[&str], script: &str) -> Ran {
	let file = scratch(name).join("out/step.xml");
	let mut args: Vec<String> = vec!["wrap".into(), "--out".into(), file.display().to_string()];
	args.extend(flags.iter().map(|f| (*f).to_owned()));
	args.extend(["--".into(), "sh".into(), "-c".into(), script.into()]);
	let (code, out, err) = wrap(&args);
	Ran {
		code,
		out,
		err,
		file,
	}
}

fn wrap(args: &[String]) -> (u8, Vec<u8>, Vec<u8>) {
	let fake = FakeForge::new(REPO);
	let factory = Factory::new(&fake);
	let vars: BTreeMap<String, String> = BTreeMap::new();
	let lookup = |name: &str| vars.get(name).cloned();
	let mut out = Vec::new();
	let mut err = Vec::new();
	let code = {
		let mut io = Streams {
			out: &mut out,
			err: &mut err,
		};
		run(args, &lookup, &factory, &mut io)
	};
	(code, out, err)
}

#[test]
fn a_command_that_passes_passes_and_keeps_none_of_its_output() {
	let r = sh("pass", &[], "echo hello; echo oops >&2");
	assert_eq!(r.code, 0);
	assert_eq!(r.out(), "hello\n");
	assert_eq!(r.err(), "oops\n");
	let xml = r.xml();
	assert!(xml.contains("failures=\"0\""), "{xml}");
	assert!(
		!xml.contains("<failure") && !xml.contains("hello") && !xml.contains("oops"),
		"{xml}"
	);
}

#[test]
fn a_command_that_fails_keeps_its_status_and_its_output() {
	let r = sh(
		"fail",
		&["--name", "Check"],
		"echo one; sleep 0.05; echo two >&2; exit 3",
	);
	assert_eq!(
		r.code, 3,
		"the job must stay red with the command's own status"
	);
	assert_eq!(r.out(), "one\n");
	assert_eq!(r.err(), "two\n");
	let xml = r.xml();
	assert!(
		xml.contains("<failure message=\"exit status 3\" type=\"step\">one\ntwo</failure>"),
		"{xml}"
	);
	assert!(xml.contains("<testsuite name=\"Check\""), "{xml}");
	assert!(xml.contains("classname=\"wrap:Check\""), "{xml}");
	assert!(
		xml.contains("name=\"sh\""),
		"a shell's script is not the step's title: {xml}"
	);
}

#[test]
fn the_head_and_the_tail_are_kept_and_the_middle_is_counted() {
	let r = sh("ends", &["--head", "2", "--tail", "3"], "seq 1 500; exit 1");
	let xml = r.xml();
	assert!(
		xml.contains(">1\n2\n[\u{2026} 495 lines omitted \u{2026}]\n498\n499\n500</failure>"),
		"{xml}"
	);
	assert_eq!(
		r.out().lines().count(),
		500,
		"every line still goes to the log"
	);
}

#[test]
fn a_lot_of_output_costs_a_small_file_and_a_long_line_is_cut() {
	let r = sh("much", &[], "yes line | head -n 300000; exit 1");
	let xml = r.xml();
	assert!(xml.len() < 6_000, "{}", xml.len());
	assert!(xml.contains("lines omitted"), "{xml}");

	let long = sh(
		"long",
		&[],
		"head -c 200000 /dev/zero | tr '\\0' a; echo; exit 1",
	);
	assert!(long.xml().len() < 6_000, "{}", long.xml().len());
	assert!(long.xml().contains('\u{2026}'));
	assert_eq!(long.out().trim_end().len(), 200_000, "the log is not cut");
}

#[test]
fn only_the_status_and_the_time_are_kept_when_asked() {
	let r = sh("quiet", &["--no-output"], "echo SECRET-VALUE; exit 1");
	assert_eq!(r.code, 1);
	assert_eq!(
		r.out(),
		"SECRET-VALUE\n",
		"the log is the job's own business"
	);
	let xml = r.xml();
	assert!(!xml.contains("SECRET-VALUE"), "{xml}");
	assert!(
		xml.contains("<failure message=\"exit status 1\" type=\"step\"></failure>"),
		"{xml}"
	);
}

#[test]
fn terminal_escapes_are_left_in_the_log_and_out_of_the_file() {
	let r = sh(
		"ansi",
		&[],
		"printf '\\033[31mred\\033[0m\\n'; printf '10%%\\r100%%\\n'; exit 1",
	);
	assert_eq!(r.out(), "\u{1b}[31mred\u{1b}[0m\n10%\r100%\n");
	let xml = r.xml();
	assert!(xml.contains(">red\n100%</failure>"), "{xml}");
	assert!(!xml.contains('\u{1b}'), "{xml:?}");
}

#[test]
fn output_that_is_not_text_goes_through_byte_for_byte() {
	let r = sh("bytes", &[], "printf '\\377\\376\\n'; exit 1");
	assert_eq!(r.out, [0xff, 0xfe, b'\n']);
	assert!(
		r.xml().contains(">\u{fffd}\u{fffd}</failure>"),
		"{}",
		r.xml()
	);
}

#[test]
fn xml_specials_in_the_output_cannot_break_the_file() {
	let r = sh(
		"xml",
		&[],
		"echo '</failure><x a=\"1\">&amp;'; printf '\\001\\n'; exit 1",
	);
	let xml = r.xml();
	assert!(
		xml.contains("&lt;/failure&gt;&lt;x a=&quot;1&quot;&gt;&amp;amp;"),
		"{xml}"
	);
	assert!(!xml.contains('\u{1}'), "{xml:?}");
}

#[test]
fn a_command_that_is_not_there_is_127_and_says_so_in_the_file() {
	let file = scratch("missing").join("m.xml");
	let args: Vec<String> = [
		"wrap",
		"--out",
		file.to_str().unwrap(),
		"--",
		"pr-results-no-such-command",
	]
	.iter()
	.map(|s| (*s).to_owned())
	.collect();
	let (code, _, err) = wrap(&args);
	assert_eq!(code, 127);
	assert!(String::from_utf8_lossy(&err).contains("the command was not found"));
	let xml = std::fs::read_to_string(&file).unwrap();
	assert!(
		xml.contains("message=\"could not run: the command was not found\""),
		"{xml}"
	);
}

#[test]
fn a_command_that_is_killed_exits_128_plus_the_signal() {
	let r = sh("signal", &[], "echo before; kill -TERM $$");
	assert_eq!(r.code, 143);
	assert!(
		r.xml().contains("message=\"killed by signal 15\""),
		"{}",
		r.xml()
	);
	assert!(r.xml().contains(">before</failure>"), "{}", r.xml());
}

#[test]
fn the_file_goes_where_it_is_told_even_in_a_directory_that_is_not_there_yet() {
	let r = sh("dir", &[], "exit 0");
	assert!(r.file.parent().unwrap().is_dir());
}

#[test]
fn a_file_that_cannot_be_written_does_not_change_the_commands_status() {
	let dir = scratch("unwritable");
	let blocker = dir.join("a-file");
	std::fs::write(&blocker, "x").unwrap();
	let out = blocker.join("step.xml");
	let args: Vec<String> = [
		"wrap",
		"--out",
		out.to_str().unwrap(),
		"--",
		"sh",
		"-c",
		"exit 4",
	]
	.iter()
	.map(|s| (*s).to_owned())
	.collect();
	let (code, _, err) = wrap(&args);
	assert_eq!(code, 4);
	assert!(String::from_utf8_lossy(&err).contains("could not write"));
}

#[test]
fn the_file_is_the_step_results_the_publisher_reads() {
	// What `wrap` writes is rendered by the same `render` the publisher uses for the `steps` flavour.
	let r = sh(
		"roundtrip",
		&["--name", "Clippy", "--step", "cargo clippy"],
		"echo 'error: this function has too many arguments (8/7)' >&2; echo '  --> src/lib.rs:9:1' >&2; exit 101",
	);
	assert_eq!(r.code, 101);
	let dir = r.file.parent().unwrap().to_owned();
	let config = dir.join("pr-results.toml");
	std::fs::write(
		&config,
		"schema = 1\ntitle = \"CI\"\n\n[[suite]]\nname = \"Clippy\"\nartifact = \"x\"\nflavour = \"steps\"\n",
	)
	.unwrap();
	let args: Vec<String> = [
		"render",
		"--config",
		config.to_str().unwrap(),
		r.file.to_str().unwrap(),
		"--out",
		dir.join("rendered").to_str().unwrap(),
	]
	.iter()
	.map(|s| (*s).to_owned())
	.collect();
	let (code, _, err) = wrap(&args);
	assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
	let comment = std::fs::read_to_string(dir.join("rendered/comment.md")).unwrap();
	assert!(
		comment.contains("### CI: 1 failure in 1 suite"),
		"{comment}"
	);
	assert!(comment.contains("`cargo clippy`"), "{comment}");
	assert!(comment.contains("exit status 101"), "{comment}");
	assert!(
		comment.contains("error: this function has too many arguments (8/7)"),
		"{comment}"
	);
}
