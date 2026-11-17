// `pr-results wrap`: runs a command, lets its output through, and records how it went as step JUnit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A job that runs `cargo clippy` already knows whether it passed and what it printed. Wrapping the
//! command writes that down in the `steps` shape the publisher reads, so the comment can show why
//! it failed without anyone opening the job. The command runs once, here, exactly as before:
//!
//! - It is run directly with its arguments, never through a shell.
//! - Its output goes through unchanged, so the job's log and any workflow command it writes
//!   (`::add-mask::` and the like) still work.
//! - The exit status is its own, so a red job stays red. A command that could not be run exits
//!   127 (not found) or 126, and one killed by a signal exits 128 plus the signal.
//!
//! What is kept is bounded: a few lines from the start, the last lines, a count of what is between,
//! and a cap on the length of one line, whatever the command prints. The kept lines are written
//! **only when the command failed**, because GitHub masks a secret in a log but not in a file.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::Instant;

use crate::args::WrapArgs;
use crate::{CliError, Streams};

/// One line is kept to this many bytes; the rest of it is dropped.
const MAX_LINE_BYTES: usize = 4096;
/// A step's title and the suite's name are cut to this many characters.
const MAX_TITLE_CHARS: usize = 200;
const CHUNK: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pipe {
	Out,
	Err,
}

/// The kept part of the output: the first `head` lines, the last `keep` lines, and how many lay
/// between. Memory is bounded by those two numbers and [`MAX_LINE_BYTES`], not by the output.
#[derive(Debug, Default)]
struct Tail {
	head: usize,
	keep: usize,
	first: Vec<String>,
	last: VecDeque<String>,
	omitted: u64,
}

impl Tail {
	fn new(head: usize, keep: usize) -> Self {
		Self {
			head,
			keep,
			..Self::default()
		}
	}

	fn push(&mut self, line: String) {
		if self.first.len() < self.head {
			self.first.push(line);
			return;
		}
		if self.keep == 0 {
			self.omitted += 1;
			return;
		}
		if self.last.len() == self.keep {
			self.last.pop_front();
			self.omitted += 1;
		}
		self.last.push_back(line);
	}

	/// The kept lines in order, with one line saying how many were left out. `None` for no output.
	fn body(&self) -> Option<String> {
		let mut lines: Vec<String> = self.first.clone();
		if self.omitted > 0 {
			lines.push(format!(
				"[\u{2026} {} {} omitted \u{2026}]",
				self.omitted,
				if self.omitted == 1 { "line" } else { "lines" }
			));
		}
		lines.extend(self.last.iter().cloned());
		(!lines.is_empty()).then(|| lines.join("\n"))
	}
}

/// Splits what one pipe produces into lines, keeping at most [`MAX_LINE_BYTES`] of each.
#[derive(Debug, Default)]
struct Lines {
	current: Vec<u8>,
	cut: bool,
}

impl Lines {
	fn feed(&mut self, bytes: &[u8], into: &mut Tail) {
		for &b in bytes {
			if b == b'\n' {
				self.finish(into);
			} else if self.current.len() < MAX_LINE_BYTES {
				self.current.push(b);
			} else {
				self.cut = true;
			}
		}
	}

	/// The last line, which has no line break after it.
	fn finish(&mut self, into: &mut Tail) {
		if self.current.is_empty() && !self.cut {
			into.push(String::new());
			return;
		}
		let mut line = clean(&self.current);
		if self.cut {
			line.push('\u{2026}');
		}
		into.push(line);
		self.current.clear();
		self.cut = false;
	}

	fn end(&mut self, into: &mut Tail) {
		if !self.current.is_empty() || self.cut {
			self.finish(into);
		}
	}
}

/// A line as text: invalid UTF-8 replaced, a carriage return taken as a redraw of the line (so a
/// progress bar leaves its last state), and terminal escape sequences removed.
fn clean(raw: &[u8]) -> String {
	let text = String::from_utf8_lossy(raw);
	let text = text.trim_end_matches('\r');
	let shown = text.rsplit('\r').next().unwrap_or("");
	strip_ansi(shown)
}

/// Removes CSI sequences (colours, cursor moves), OSC sequences (titles, links) and lone escapes.
fn strip_ansi(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	let mut chars = s.chars().peekable();
	while let Some(c) = chars.next() {
		if c != '\u{1b}' {
			out.push(c);
			continue;
		}
		match chars.peek() {
			Some('[') => {
				chars.next();
				for n in chars.by_ref() {
					if ('@'..='~').contains(&n) {
						break;
					}
				}
			}
			Some(']') => {
				chars.next();
				while let Some(n) = chars.next() {
					if n == '\u{7}' {
						break;
					}
					if n == '\u{1b}' {
						if chars.peek() == Some(&'\\') {
							chars.next();
						}
						break;
					}
				}
			}
			Some(_) => {
				chars.next();
			}
			None => {}
		}
	}
	out
}

/// Text for an XML attribute or body: the five specials escaped and the characters XML 1.0 cannot
/// hold left out.
fn xml(text: &str) -> String {
	let mut out = String::with_capacity(text.len());
	for c in text.chars() {
		match c {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' => out.push_str("&quot;"),
			'\t' | '\n' | '\r' => out.push(c),
			c if c < ' ' || c == '\u{fffe}' || c == '\u{ffff}' => {}
			c => out.push(c),
		}
	}
	out
}

fn cut(text: &str, chars: usize) -> String {
	let mut shown: String = text.chars().take(chars).collect();
	if text.chars().count() > chars {
		shown.push('\u{2026}');
	}
	shown
}

/// The program's file name, so `/usr/bin/cargo` is `cargo`.
fn program_name(program: &str) -> &str {
	Path::new(program)
		.file_name()
		.and_then(|n| n.to_str())
		.unwrap_or(program)
}

/// What the step is called when `--step` says nothing: the program and its first argument that is
/// not a flag (`cargo clippy`), never the rest, which may hold something that should not be shown.
fn default_step(command: &[String]) -> String {
	let mut words = Vec::new();
	if let Some(program) = command.first() {
		let program = program_name(program);
		words.push(program.to_owned());
		// What follows a shell's `-c` is a whole script, which is not a title.
		if matches!(program, "sh" | "bash" | "dash" | "zsh" | "ksh" | "fish") {
			return words.join(" ");
		}
	}
	if let Some(word) = command.iter().skip(1).find(|w| !w.starts_with('-')) {
		words.push(word.clone());
	}
	words.join(" ")
}

/// The JUnit document: one suite, one step, and a failure with the kept output when it failed.
fn document(name: &str, step: &str, seconds: f64, failure: Option<(&str, Option<&str>)>) -> String {
	let name = xml(&cut(name, MAX_TITLE_CHARS));
	let step = xml(&cut(step, MAX_TITLE_CHARS));
	let failures = usize::from(failure.is_some());
	let time = format!("{seconds:.3}");
	let mut out = String::new();
	out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
	out.push_str(&format!(
		"<testsuites name=\"{name}\" tests=\"1\" failures=\"{failures}\" time=\"{time}\">\n"
	));
	out.push_str(&format!(
		"  <testsuite name=\"{name}\" tests=\"1\" failures=\"{failures}\" skipped=\"0\" time=\"{time}\">\n"
	));
	out.push_str(&format!(
		"    <testcase classname=\"wrap:{name}\" name=\"{step}\" time=\"{time}\""
	));
	match failure {
		None => out.push_str("/>\n"),
		Some((message, body)) => {
			out.push_str(">\n");
			out.push_str(&format!(
				"      <failure message=\"{}\" type=\"step\">{}</failure>\n",
				xml(message),
				body.map(xml).unwrap_or_default()
			));
			out.push_str("    </testcase>\n");
		}
	}
	out.push_str("  </testsuite>\n</testsuites>\n");
	out
}

/// How the command ended.
struct Ended {
	code: u8,
	/// Why it failed, in words (`exit status 3`). Meaningless when `code` is 0.
	message: String,
}

fn ended(status: ExitStatus) -> Ended {
	if let Some(code) = status.code() {
		return Ended {
			code: u8::try_from(code).unwrap_or(1),
			message: format!("exit status {code}"),
		};
	}
	#[cfg(unix)]
	{
		use std::os::unix::process::ExitStatusExt;
		if let Some(signal) = status.signal() {
			return Ended {
				code: u8::try_from(128 + signal).unwrap_or(255),
				message: format!("killed by signal {signal}"),
			};
		}
	}
	Ended {
		code: 1,
		message: "ended without a status".to_owned(),
	}
}

/// Reads one pipe to its end, handing each chunk to the main thread.
fn pump<R: Read + Send + 'static>(mut from: R, pipe: Pipe, to: mpsc::Sender<(Pipe, Vec<u8>)>) {
	std::thread::spawn(move || {
		let mut buf = vec![0u8; CHUNK];
		while let Ok(n) = from.read(&mut buf) {
			if n == 0 {
				break;
			}
			let Some(chunk) = buf.get(..n) else { break };
			if to.send((pipe, chunk.to_vec())).is_err() {
				break;
			}
		}
	});
}

/// Runs the command, passing its output through and keeping the part that is kept.
fn run_child(
	command: &[String],
	io: &mut Streams<'_>,
	tail: &mut Tail,
) -> Result<Ended, std::io::Error> {
	let Some((program, rest)) = command.split_first() else {
		return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
	};
	let mut child = Command::new(program)
		.args(rest)
		.stdin(Stdio::inherit())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()?;
	let (send, receive) = mpsc::channel();
	if let Some(out) = child.stdout.take() {
		pump(out, Pipe::Out, send.clone());
	}
	if let Some(err) = child.stderr.take() {
		pump(err, Pipe::Err, send.clone());
	}
	drop(send);

	let mut out_lines = Lines::default();
	let mut err_lines = Lines::default();
	for (pipe, bytes) in receive {
		let (sink, lines): (&mut dyn Write, &mut Lines) = match pipe {
			Pipe::Out => (&mut *io.out, &mut out_lines),
			Pipe::Err => (&mut *io.err, &mut err_lines),
		};
		let _ = sink.write_all(&bytes);
		let _ = sink.flush();
		lines.feed(&bytes, tail);
	}
	out_lines.end(tail);
	err_lines.end(tail);
	Ok(ended(child.wait()?))
}

pub(crate) fn wrap(a: &WrapArgs, io: &mut Streams<'_>) -> Result<u8, CliError> {
	let started = Instant::now();
	let mut tail = Tail::new(a.head, a.tail);
	let name = a
		.name
		.clone()
		.or_else(|| a.command.first().map(|p| program_name(p).to_owned()))
		.unwrap_or_default();
	let step = a.step.clone().unwrap_or_else(|| default_step(&a.command));

	let result = run_child(&a.command, io, &mut tail);
	let seconds = started.elapsed().as_secs_f64();
	let (code, message, body) = match result {
		Ok(end) => {
			let body = if a.no_output { None } else { tail.body() };
			(end.code, end.message, body)
		}
		Err(e) => {
			let (code, why) = match e.kind() {
				std::io::ErrorKind::NotFound => (127, "the command was not found"),
				std::io::ErrorKind::PermissionDenied => (126, "the command could not be run"),
				_ => (126, "the command could not be run"),
			};
			io.complain(&format!(
				"wrap: {why}: {}",
				a.command.first().map_or("", |p| p.as_str())
			));
			(code, format!("could not run: {why}"), None)
		}
	};
	let failure = (code != 0).then_some((message.as_str(), body.as_deref()));
	let file = document(&name, &step, seconds, failure);
	if let Err(e) = write_file(&a.out, &file) {
		io.complain(&format!(
			"wrap: could not write {}: {}",
			a.out.display(),
			e.kind()
		));
	}
	Ok(code)
}

fn write_file(path: &Path, text: &str) -> std::io::Result<()> {
	if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
		std::fs::create_dir_all(dir)?;
	}
	std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

	use super::*;

	#[test]
	fn colours_titles_and_links_are_removed_and_text_is_kept() {
		assert_eq!(strip_ansi("\u{1b}[31merror\u{1b}[0m: bad"), "error: bad");
		assert_eq!(strip_ansi("\u{1b}[1;38;5;196mx\u{1b}[K"), "x");
		assert_eq!(strip_ansi("a\u{1b}]0;title\u{7}b"), "ab");
		assert_eq!(
			strip_ansi("a\u{1b}]8;;https://x\u{1b}\\link\u{1b}]8;;\u{1b}\\"),
			"alink"
		);
		assert_eq!(strip_ansi("a\u{1b}Mb"), "ab");
		assert_eq!(strip_ansi("trailing\u{1b}"), "trailing");
		assert_eq!(strip_ansi("unterminated\u{1b}[31"), "unterminated");
	}

	#[test]
	fn a_carriage_return_is_a_redraw_and_crlf_is_a_line_break() {
		assert_eq!(clean(b"10%\r50%\r100%"), "100%");
		assert_eq!(clean(b"line\r"), "line");
		assert_eq!(clean(b"\xff\xfe ok"), "\u{fffd}\u{fffd} ok");
	}

	#[test]
	fn the_head_and_the_tail_are_kept_and_what_is_between_is_counted() {
		let mut tail = Tail::new(2, 3);
		for n in 1..=10 {
			tail.push(format!("line {n}"));
		}
		assert_eq!(
			tail.body().unwrap(),
			"line 1\nline 2\n[\u{2026} 5 lines omitted \u{2026}]\nline 8\nline 9\nline 10"
		);
		let mut short = Tail::new(2, 3);
		for n in 1..=4 {
			short.push(format!("line {n}"));
		}
		assert_eq!(short.body().unwrap(), "line 1\nline 2\nline 3\nline 4");
		let mut none = Tail::new(0, 0);
		none.push("x".to_owned());
		assert_eq!(none.body().unwrap(), "[\u{2026} 1 line omitted \u{2026}]");
		assert_eq!(Tail::new(2, 2).body(), None);
	}

	#[test]
	fn one_long_line_is_cut_and_memory_does_not_grow_with_the_output() {
		let mut tail = Tail::new(1, 1);
		let mut lines = Lines::default();
		lines.feed(&vec![b'a'; 50_000], &mut tail);
		lines.feed(b"\n", &mut tail);
		let body = tail.body().unwrap();
		assert!(body.len() < MAX_LINE_BYTES + 8, "{}", body.len());
		assert!(body.ends_with('\u{2026}'));
		for _ in 0..10_000 {
			lines.feed(b"x\n", &mut tail);
		}
		assert!(tail.first.len() <= 1 && tail.last.len() <= 1);
	}

	#[test]
	fn text_for_xml_is_escaped_and_loses_what_xml_cannot_hold() {
		assert_eq!(xml("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
		assert_eq!(xml("a\u{0}b\u{1b}c\u{fffe}d\te\n"), "abcd\te\n");
	}

	#[test]
	fn the_default_step_is_the_program_and_its_first_word_and_nothing_else() {
		let words = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
		assert_eq!(
			default_step(&words(&["cargo", "clippy", "--all"])),
			"cargo clippy"
		);
		assert_eq!(
			default_step(&words(&["/usr/bin/cargo", "--locked", "audit"])),
			"cargo audit"
		);
		assert_eq!(
			default_step(&words(&["curl", "--header", "token=SECRET"])),
			"curl token=SECRET",
			"the first word that is not a flag is shown, so say --step for such a command"
		);
		assert_eq!(default_step(&words(&["make"])), "make");
		assert_eq!(default_step(&words(&["bash", "-c", "echo $TOKEN"])), "bash");
	}

	#[test]
	fn the_document_has_a_failure_only_when_the_command_failed() {
		let ok = document("Clippy", "cargo clippy", 1.5, None);
		assert!(ok.contains("failures=\"0\"") && !ok.contains("<failure"));
		let bad = document(
			"Clippy",
			"cargo clippy",
			1.5,
			Some(("exit status 101", Some("a\nb"))),
		);
		assert!(bad.contains("failures=\"1\""));
		assert!(bad.contains("<failure message=\"exit status 101\" type=\"step\">a\nb</failure>"));
		assert!(bad.contains("classname=\"wrap:Clippy\""));
		assert!(bad.contains("time=\"1.500\""));
	}
}
