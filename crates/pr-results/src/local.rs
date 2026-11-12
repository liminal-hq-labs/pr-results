// Read bounded, downloaded artefact files for in-job publishing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Read;
use std::path::Path;

use crate::CliError;
use pr_results_core::{parse_junit_in, Report};
use pr_results_forge::{Inputs, Plan};

pub(crate) type LocalFile = (String, String, Vec<u8>);

/// Open every component relative to an already opened directory, without following links.
#[cfg(unix)]
pub(crate) fn files(root: &Path, plan: &Plan) -> Result<(Vec<LocalFile>, usize), CliError> {
	use rustix::fs::{openat, Dir, Mode, OFlags};
	let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
	let root = open_root(root, flags)?;
	let root_dir =
		Dir::read_from(&root).map_err(|_| CliError::usage("an input directory cannot be read"))?;
	let mut pending = vec![(root_dir, Vec::<String>::new())];
	let mut found = Vec::new();
	let mut skipped = 0usize;
	let mut count = 0usize;
	let mut total = 0u64;
	let mut directories = 1usize;
	let mut entries = 0usize;
	while let Some((dir, parts)) = pending.last_mut() {
		let Some(entry) = dir.read() else {
			pending.pop();
			continue;
		};
		let entry = entry.map_err(|_| CliError::usage("an input directory cannot be read"))?;
		let name = entry
			.file_name()
			.to_str()
			.map_err(|_| CliError::usage("an input name is not UTF-8"))?;
		if name == "." || name == ".." {
			continue;
		}
		entries = entries.saturating_add(1);
		if entries > plan.archive.max_files.saturating_mul(2).saturating_add(1) {
			return Err(CliError::usage("too many input entries"));
		}
		let fd = dir
			.fd()
			.map_err(|_| CliError::usage("an input directory cannot be read"))?;
		let file = match openat(fd, entry.file_name(), flags, Mode::empty()) {
			Ok(fd) => std::fs::File::from(fd),
			Err(rustix::io::Errno::LOOP) => {
				skipped += 1;
				continue;
			}
			Err(_) => return Err(CliError::usage("an input entry cannot be opened")),
		};
		let meta = file
			.metadata()
			.map_err(|_| CliError::usage("an input entry cannot be read"))?;
		let mut path = parts.clone();
		path.push(name.to_owned());
		if meta.is_dir() {
			directories = directories.saturating_add(1);
			if directories > plan.archive.max_files.saturating_add(1) || path.len() > 64 {
				return Err(CliError::usage("too many input directories"));
			}
			let dir =
				Dir::new(file).map_err(|_| CliError::usage("an input directory cannot be read"))?;
			pending.push((dir, path));
			continue;
		}
		if !meta.is_file() {
			skipped += 1;
			continue;
		}
		count += 1;
		total = total.saturating_add(meta.len());
		if count > plan.archive.max_files
			|| meta.len() > plan.archive.max_entry_bytes
			|| total > plan.archive.max_total_bytes
		{
			return Err(CliError::usage(
				"the local inputs exceed the configured file or byte limits",
			));
		}
		let mut data = Vec::new();
		file.take(plan.archive.max_entry_bytes.saturating_add(1))
			.read_to_end(&mut data)
			.map_err(|_| CliError::usage("an input file cannot be read"))?;
		if data.len() as u64 > meta.len() {
			total = total.saturating_add(data.len() as u64 - meta.len());
		}
		if data.len() as u64 > plan.archive.max_entry_bytes || total > plan.archive.max_total_bytes
		{
			return Err(CliError::usage(
				"the local inputs exceed the configured byte limits",
			));
		}
		if let [artifact, rest @ ..] = path.as_slice() {
			if !rest.is_empty() {
				found.push((artifact.clone(), rest.join("/"), data));
			}
		}
	}
	found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
	Ok((found, skipped))
}

/// Resolve the selected root one directory at a time; NOFOLLOW alone applies only to the last component.
#[cfg(unix)]
fn open_root(root: &Path, flags: rustix::fs::OFlags) -> Result<std::os::fd::OwnedFd, CliError> {
	use rustix::fs::{openat, Mode, OFlags, CWD};
	use std::path::Component;
	if root.as_os_str().is_empty() {
		return Err(CliError::usage("the input path is empty"));
	}
	let directory_flags = flags | OFlags::DIRECTORY;
	let start = if root.is_absolute() { "/" } else { "." };
	let fail =
		|_| CliError::usage("the input path must contain only readable directories, not links");
	let mut fd = openat(CWD, start, directory_flags, Mode::empty()).map_err(fail)?;
	for part in root.components() {
		match part {
			Component::Normal(name) => {
				fd = openat(&fd, name, directory_flags, Mode::empty()).map_err(fail)?
			}
			Component::RootDir | Component::CurDir => {}
			_ => {
				return Err(CliError::usage(
					"the input path cannot contain parent components",
				))
			}
		}
	}
	Ok(fd)
}

#[cfg(not(unix))]
pub(crate) fn files(_root: &Path, _plan: &Plan) -> Result<(Vec<LocalFile>, usize), CliError> {
	Err(CliError::usage("in-job publishing requires a Unix runner"))
}

pub(crate) fn read(files: &[LocalFile], plan: &Plan) -> Inputs {
	let mut out = Inputs::default();
	for section in &plan.sections {
		let mut report = Report::default();
		let mut skipped = 0;
		for (_, _, data) in files.iter().filter(|(a, n, _)| {
			section.artifact.is_match(a) && section.files.iter().any(|g| g.is_match(n))
		}) {
			match parse_junit_in(
				data,
				section.flavour,
				&plan.parse,
				section.path_prefix.as_ref(),
			) {
				Ok(parsed) => report.suites.extend(parsed.suites),
				Err(_) => skipped += 1,
			}
		}
		report.merge_same_named();
		out.suites.push(report.suites);
		out.skipped.push(skipped);
	}
	out
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
	use super::*;
	use pr_results_core::{CommentKey, Flavour, Glob, RepoPath};
	use pr_results_forge::SectionPlan;

	#[test]
	fn local_reports_apply_the_suite_path_prefix() {
		let section = SectionPlan {
			title: "Web".into(),
			check_name: "Web".into(),
			required: false,
			artifact: Glob::new("web").unwrap(),
			files: vec![Glob::new("**/*.xml").unwrap()],
			flavour: Flavour::Generic,
			path_prefix: RepoPath::parse("apps/web/"),
		};
		let plan = Plan::new(
			"App",
			CommentKey::parse("ci").unwrap(),
			vec![section],
			"test",
		);
		let xml = br#"<testsuite name="suite"><testcase name="case" file="src/a.js" line="2"><failure>failed</failure></testcase></testsuite>"#.to_vec();
		let inputs = read(&[("web".into(), "test.xml".into(), xml)], &plan);
		assert_eq!(
			inputs.suites[0][0].cases[0].file.as_ref().unwrap().as_str(),
			"apps/web/src/a.js"
		);
	}
	#[cfg(unix)]
	#[test]
	fn input_root_rejects_symlinked_ancestors() {
		use std::os::unix::fs::symlink;
		let relative =
			std::path::PathBuf::from(format!("target/local-root-test-{}", std::process::id()));
		let absolute = std::env::current_dir().unwrap().join(&relative);
		std::fs::create_dir_all(absolute.join("real/results")).unwrap();
		symlink("real", absolute.join("link")).unwrap();
		let plan = Plan::new("App", CommentKey::parse("ci").unwrap(), vec![], "test");
		assert!(files(&absolute.join("real/results"), &plan).is_ok());
		assert!(files(&relative.join("real/results"), &plan).is_ok());
		assert!(files(&absolute.join("link/results"), &plan).is_err());
		assert!(files(&relative.join("link/results"), &plan).is_err());
		assert!(files(&relative.join("real/../real/results"), &plan).is_err());
		std::fs::remove_dir_all(absolute).unwrap();
	}

	#[cfg(target_os = "linux")]
	#[test]
	fn wide_directory_is_rejected_before_exhausting_file_descriptors() {
		const CHILD: &str = "PR_RESULTS_FD_LIMIT_TEST";
		if std::env::var_os(CHILD).is_none() {
			let executable = std::env::current_exe().unwrap();
			let output = std::process::Command::new("prlimit")
				.args(["--nofile=32", "--"])
				.arg(executable)
				.args([
					"--exact",
					"local::tests::wide_directory_is_rejected_before_exhausting_file_descriptors",
					"--nocapture",
				])
				.env(CHILD, "1")
				.output()
				.unwrap();
			assert!(
				output.status.success(),
				"{}",
				String::from_utf8_lossy(&output.stdout)
			);
			return;
		}
		let root = std::env::temp_dir().join(format!("pr-results-fd-test-{}", std::process::id()));
		for i in 0..80 {
			std::fs::create_dir_all(root.join(format!("dir-{i}"))).unwrap();
		}
		let mut plan = Plan::new("App", CommentKey::parse("ci").unwrap(), vec![], "test");
		plan.archive.max_files = 2;
		let error = files(&root, &plan).unwrap_err();
		std::fs::remove_dir_all(root).unwrap();
		assert_eq!(error.message, "too many input directories");
	}
	#[cfg(unix)]
	#[test]
	fn skipped_links_are_bounded() {
		use std::os::unix::fs::symlink;
		let root =
			std::env::temp_dir().join(format!("pr-results-links-test-{}", std::process::id()));
		std::fs::create_dir_all(&root).unwrap();
		for i in 0..6 {
			symlink("missing", root.join(format!("link-{i}"))).unwrap();
		}
		let mut plan = Plan::new("App", CommentKey::parse("ci").unwrap(), vec![], "test");
		plan.archive.max_files = 2;
		let result = files(&root, &plan);
		std::fs::remove_dir_all(root).unwrap();
		assert_eq!(result.unwrap_err().message, "too many input entries");
	}
	#[cfg(target_os = "linux")]
	#[test]
	fn wide_valid_tree_fits_a_low_fd_limit() {
		const CHILD: &str = "PR_RESULTS_WIDE_TREE_TEST";
		if std::env::var_os(CHILD).is_none() {
			let output = std::process::Command::new("prlimit")
				.args(["--nofile=32", "--"])
				.arg(std::env::current_exe().unwrap())
				.args([
					"--exact",
					"local::tests::wide_valid_tree_fits_a_low_fd_limit",
					"--nocapture",
				])
				.env(CHILD, "1")
				.output()
				.unwrap();
			assert!(
				output.status.success(),
				"{} {}",
				String::from_utf8_lossy(&output.stdout),
				String::from_utf8_lossy(&output.stderr)
			);
			return;
		}
		let root =
			std::env::temp_dir().join(format!("pr-results-wide-test-{}", std::process::id()));
		for i in 0..80 {
			let dir = root.join(format!("suite-{i}"));
			std::fs::create_dir_all(&dir).unwrap();
			std::fs::write(dir.join("test.xml"), b"<testsuite/>").unwrap();
		}
		let mut plan = Plan::new("App", CommentKey::parse("ci").unwrap(), vec![], "test");
		plan.archive.max_files = 100;
		let result = files(&root, &plan);
		std::fs::remove_dir_all(root).unwrap();
		assert_eq!(result.unwrap().0.len(), 80);
	}
}
