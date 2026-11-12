// Read bounded, downloaded artefact files for in-job publishing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Read;
use std::path::Path;

use crate::CliError;
use pr_results_core::{parse_junit, Report};
use pr_results_forge::{Inputs, Plan};

pub(crate) type LocalFile = (String, String, Vec<u8>);

/// Open every component relative to an already opened directory, without following links.
#[cfg(unix)]
pub(crate) fn files(root: &Path, plan: &Plan) -> Result<(Vec<LocalFile>, usize), CliError> {
	use rustix::fs::{openat, Dir, Mode, OFlags, CWD};
	let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
	let root = openat(CWD, root, flags | OFlags::DIRECTORY, Mode::empty())
		.map_err(|_| CliError::usage("the input path must be a readable directory, not a link"))?;
	let mut pending = vec![(root, Vec::<String>::new())];
	let mut found = Vec::new();
	let mut skipped = 0usize;
	let mut count = 0usize;
	let mut total = 0u64;
	let mut directories = 0usize;
	while let Some((fd, parts)) = pending.pop() {
		directories += 1;
		if directories > plan.archive.max_files.saturating_add(1) || parts.len() > 64 {
			return Err(CliError::usage("too many input directories"));
		}
		let mut dir = Dir::read_from(&fd)
			.map_err(|_| CliError::usage("an input directory cannot be read"))?;
		while let Some(entry) = dir.read() {
			let entry = entry.map_err(|_| CliError::usage("an input directory cannot be read"))?;
			let name = entry
				.file_name()
				.to_str()
				.map_err(|_| CliError::usage("an input name is not UTF-8"))?;
			if name == "." || name == ".." {
				continue;
			}
			let file = match openat(&fd, entry.file_name(), flags, Mode::empty()) {
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
				pending.push((file.into(), path));
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
			if data.len() as u64 > plan.archive.max_entry_bytes
				|| total > plan.archive.max_total_bytes
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
	}
	found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
	Ok((found, skipped))
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
			match parse_junit(data, section.flavour, &plan.parse) {
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
