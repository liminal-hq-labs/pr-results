// Tests for the glob matcher that names artefacts and files in the settings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use pr_results_core::{Glob, GlobError};

fn yes(pattern: &str, text: &str) {
	let g = Glob::new(pattern).unwrap();
	assert!(g.is_match(text), "{pattern} should match {text}");
}

fn no(pattern: &str, text: &str) {
	let g = Glob::new(pattern).unwrap();
	assert!(!g.is_match(text), "{pattern} should not match {text}");
}

#[test]
fn literals_match_only_themselves() {
	yes("steps.xml", "steps.xml");
	no("steps.xml", "steps.xm");
	no("steps.xml", "Steps.xml");
	no("steps.xml", "dir/steps.xml");
	no("steps.xml", "steps.xmlx");
}

#[test]
fn star_stays_inside_one_segment() {
	yes("*-test-results*", "rust-test-results");
	yes("*-test-results*", "rust-test-results-linux");
	yes("steps-test-results-*", "steps-test-results-");
	yes("*", "");
	yes("*.xml", "a.xml");
	yes("nextest-*.xml", "nextest-lib.xml");
	no("*.xml", "dir/a.xml");
	no("*-test-results*", "rust-results");
	no("a*b", "a/b");
}

#[test]
fn question_mark_is_one_character_that_is_not_a_slash() {
	yes("a?c", "abc");
	yes("a?c", "a\u{e9}c");
	no("a?c", "ac");
	no("a?c", "abbc");
	no("a?c", "a/c");
}

#[test]
fn double_star_crosses_segments() {
	yes("**/*.xml", "a.xml");
	yes("**/*.xml", "a/b/c.xml");
	yes("a/**/z", "a/z");
	yes("a/**/z", "a/b/c/z");
	yes("a/**", "a");
	yes("a/**", "a/b/c");
	yes("**", "anything/at/all");
	yes("**", "");
	no("**/*.xml", "a/b/c.txt");
	no("a/**/z", "b/a/z");
	no("a/**/z", "a/b/zz");
	yes("**/junit.xml", "target/nextest/ci/junit.xml");
}

#[test]
fn double_star_inside_a_segment_is_one_star() {
	yes("a**b", "aXb");
	no("a**b", "a/b");
}

#[test]
fn brackets_and_braces_are_literal() {
	yes("a[1].xml", "a[1].xml");
	no("a[1].xml", "a1.xml");
	yes("{a,b}", "{a,b}");
	no("{a,b}", "a");
}

#[test]
fn empty_segments_match_what_they_say() {
	yes("a//b", "a//b");
	no("a//b", "a/b");
	yes("a/", "a/");
	no("a/", "a");
}

#[test]
fn a_pattern_is_checked_when_made() {
	assert_eq!(Glob::new("").unwrap_err(), GlobError::Empty);
	assert_eq!(Glob::new(&"a".repeat(257)).unwrap_err(), GlobError::TooLong);
	assert!(Glob::new(&"a".repeat(256)).is_ok());
	assert_eq!(Glob::new("a\\b").unwrap_err(), GlobError::BadCharacter);
	assert_eq!(Glob::new("a\nb").unwrap_err(), GlobError::BadCharacter);
	assert_eq!(Glob::new("a\0b").unwrap_err(), GlobError::BadCharacter);
}

#[test]
fn hostile_patterns_do_not_blow_up() {
	// Many stars against a long text that nearly matches: a backtracking matcher would stall.
	let pattern = format!("{}b", "a*".repeat(100));
	let g = Glob::new(&pattern).unwrap();
	let text = "a".repeat(5000);
	assert!(!g.is_match(&text));
	let deep = Glob::new(&"**/".repeat(80)).unwrap();
	assert!(deep.is_match(&"x/".repeat(500)));
}

#[test]
fn it_keeps_its_pattern() {
	let g = Glob::new("a/**/*.xml").unwrap();
	assert_eq!(g.pattern(), "a/**/*.xml");
	assert_eq!(g.to_string(), "a/**/*.xml");
}
