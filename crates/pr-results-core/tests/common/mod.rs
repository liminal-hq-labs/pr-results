// Shared helpers for the integration tests: fixtures and a deterministic generator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn fixtures_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

pub fn fixture(relative: &str) -> Vec<u8> {
	std::fs::read(fixtures_dir().join(relative))
		.unwrap_or_else(|e| panic!("fixture {relative}: {e}"))
}

/// Every XML fixture under `tests/fixtures`, as (relative path, bytes), in a fixed order.
pub fn all_fixtures() -> Vec<(String, Vec<u8>)> {
	let mut found = Vec::new();
	let mut dirs = vec![fixtures_dir()];
	while let Some(dir) = dirs.pop() {
		for entry in std::fs::read_dir(&dir).unwrap() {
			let path = entry.unwrap().path();
			if path.is_dir() {
				dirs.push(path);
			} else if path.extension().is_some_and(|e| e == "xml") {
				let rel = path
					.strip_prefix(fixtures_dir())
					.unwrap()
					.to_string_lossy()
					.into_owned();
				found.push((rel, std::fs::read(&path).unwrap()));
			}
		}
	}
	found.sort();
	found
}

/// A small xorshift generator: the same seed gives the same sequence on every machine.
pub struct Rng(pub u64);

impl Rng {
	pub fn next(&mut self) -> u64 {
		let mut x = self.0;
		x ^= x << 13;
		x ^= x >> 7;
		x ^= x << 17;
		self.0 = x;
		x
	}

	pub fn below(&mut self, n: usize) -> usize {
		(self.next() % n as u64) as usize
	}
}
