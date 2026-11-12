// Tests for the values the tool controls and the text built from them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use pr_results_core::{
	CommentKey, Marker, RepoPath, RepoSlug, RunId, Safe, ServerUrl, Sha, TrustedUrl, Untrusted,
};

const SHA: &str = "3f9c2e1d4b5a69788796a5b4c3d2e1f001122334";

fn marker() -> Marker {
	Marker {
		key: CommentKey::parse("ci").unwrap(),
		sha: Sha::parse(SHA).unwrap(),
		run: RunId(19_283_746),
	}
}

#[test]
fn sha_is_forty_lowercase_hex_digits() {
	assert!(Sha::parse(SHA).is_some());
	assert_eq!(Sha::parse(SHA).unwrap().short(), "3f9c2e1");
	for bad in [
		"",
		"3f9c2e1",
		&SHA.to_uppercase(),
		&format!("{SHA}0"),
		&SHA.replace('3', "g"),
		&format!(" {}", &SHA[1..]),
	] {
		assert!(Sha::parse(bad).is_none(), "{bad:?}");
	}
}

#[test]
fn repo_slug_allows_what_github_allows() {
	let slug = RepoSlug::parse("liminal-hq-labs/pr-results").unwrap();
	assert_eq!(slug.owner(), "liminal-hq-labs");
	assert_eq!(slug.name(), "pr-results");
	assert_eq!(slug.to_string(), "liminal-hq-labs/pr-results");
	for bad in [
		"",
		"owner",
		"/name",
		"owner/",
		"owner/name/extra",
		"-owner/name",
		"own er/name",
		"owner/na me",
		"owner/..",
		"owner/.",
		"owner/na`me",
		"owner/na@me",
		"ow_ner/name",
	] {
		assert!(RepoSlug::parse(bad).is_none(), "{bad:?}");
	}
	assert!(RepoSlug::parse("o/a.b_c-d").is_some());
}

#[test]
fn comment_key_rules() {
	for ok in ["ci", "e2e", "0", "a-b-c", &"a".repeat(32)] {
		assert!(CommentKey::parse(ok).is_some(), "{ok:?}");
	}
	for bad in ["", "-a", "A", "a b", "a_b", "a=b", &"a".repeat(33), "ci\n"] {
		assert!(CommentKey::parse(bad).is_none(), "{bad:?}");
	}
}

#[test]
fn server_url_is_https_and_a_host_only() {
	assert_eq!(
		ServerUrl::parse("https://github.com").unwrap().as_str(),
		"https://github.com"
	);
	assert_eq!(
		ServerUrl::parse("https://GHE.Example.com:8443")
			.unwrap()
			.as_str(),
		"https://ghe.example.com:8443"
	);
	for bad in [
		"",
		"http://github.com",
		"https://",
		"https://github.com/",
		"https://github.com/x",
		"https://user@github.com",
		"https://github.com?x=1",
		"https://github.com#x",
		"https://git hub.com",
		"https://github.com:0",
		"https://github.com:65536",
		"https://github.com:",
		"https://github.com:08",
		"https://-a.com",
		"https://a..com",
		"https://a.com.",
		"https://[::1]",
		"https://git\u{ff20}hub.com",
		"https://github.com\n",
	] {
		assert!(ServerUrl::parse(bad).is_none(), "{bad:?}");
	}
}

#[test]
fn repo_path_follows_the_location_rules() {
	assert_eq!(
		RepoPath::parse("./src//lib.rs").unwrap().as_str(),
		"src/lib.rs"
	);
	assert_eq!(RepoPath::parse("a\\b.ts").unwrap().as_str(), "a/b.ts");
	for bad in [
		"",
		"/etc/passwd",
		"../x",
		"a/../../x",
		"C:\\x",
		"file:///etc/passwd",
		"a\nb",
		"a\u{202e}b",
		"a\u{a0}b",
		"a\u{200b}b",
	] {
		assert!(RepoPath::parse(bad).is_none(), "{bad:?}");
	}
	assert!(
		RepoPath::parse(&"a/".repeat(600)).is_none(),
		"over the length cap"
	);
}

#[test]
fn a_cut_path_is_no_path() {
	let whole = Untrusted::new("src/lib.rs");
	assert!(RepoPath::from_case(&whole).is_some());
	let cut = Untrusted::capped("src/lib.rs/and/more", 10);
	assert!(RepoPath::from_case(&cut).is_none());
}

#[test]
fn urls_come_only_from_trusted_parts() {
	let server = ServerUrl::parse("https://github.com").unwrap();
	let repo = RepoSlug::parse("example/app").unwrap();
	let sha = Sha::parse(SHA).unwrap();
	assert_eq!(
		TrustedUrl::commit(&server, &repo, &sha).as_str(),
		format!("https://github.com/example/app/commit/{SHA}")
	);
	assert_eq!(
		TrustedUrl::run(&server, &repo, RunId(7)).as_str(),
		"https://github.com/example/app/actions/runs/7"
	);
	assert_eq!(
		TrustedUrl::run_artifacts(&server, &repo, RunId(7)).as_str(),
		"https://github.com/example/app/actions/runs/7#artifacts"
	);
	assert_eq!(
		TrustedUrl::forgejo_run(&server, &repo, 12).as_str(),
		"https://github.com/example/app/actions/runs/12"
	);
	assert_eq!(
		TrustedUrl::comment_anchor(&server, &repo, 3, 45).as_str(),
		"https://github.com/example/app/pulls/3#issuecomment-45"
	);
	let url = TrustedUrl::run(&server, &repo, RunId(7));
	let label = Safe::trusted("workflow run");
	assert_eq!(
		Safe::link(&url, &label).as_str(),
		"[workflow run](https://github.com/example/app/actions/runs/7)"
	);
}

#[test]
fn marker_round_trips() {
	let m = marker();
	let line = m.render();
	assert_eq!(
		line.as_str(),
		format!("<!-- pr-results:1 key=ci sha={SHA} run=19283746 -->")
	);
	assert_eq!(Marker::parse(line.as_str()), Some(m.clone()));
	let body = format!("{}\n### Title: All 3 passed\n", line);
	assert_eq!(Marker::parse(&body), Some(m));
}

#[test]
fn marker_accepts_a_crlf_line_ending_and_nothing_else_after_the_arrow() {
	let m = marker();
	let good = m.render().as_str().to_owned();
	assert_eq!(
		Marker::parse(&format!("{good}\r\nbody\r\n")),
		Some(m.clone())
	);
	assert_eq!(Marker::parse(&format!("{good}\r")), Some(m));
	assert_eq!(Marker::parse(&format!("{good}\r\r\nbody")), None);
	assert_eq!(Marker::parse(&format!("{good} \r\nbody")), None);
	assert_eq!(Marker::parse(&format!("\r{good}")), None);
	let ci = CommentKey::parse("ci").unwrap();
	assert!(Marker::prefix_matches(&format!("{good}\r\nbody"), &ci));
}

#[test]
fn marker_refuses_forgeries() {
	let good = format!("<!-- pr-results:1 key=ci sha={SHA} run=19283746 -->");
	let forged = [
		// Not the first line.
		format!("hello\n{good}"),
		format!("\n{good}"),
		format!(" {good}"),
		format!("\u{feff}{good}"),
		// Wrong version, shape or order.
		good.replace("pr-results:1", "pr-results:2"),
		good.replace("<!-- ", "<!--"),
		good.replace(" -->", "-->"),
		good.replace(" -->", " --> trailing"),
		format!("<!-- pr-results:1 sha={SHA} key=ci run=19283746 -->"),
		// Bad parts.
		good.replace("key=ci", "key=CI"),
		good.replace("key=ci", "key="),
		good.replace(SHA, &SHA[..39]),
		good.replace(SHA, &SHA.to_uppercase()),
		good.replace("run=19283746", "run="),
		good.replace("run=19283746", "run=0123"),
		good.replace("run=19283746", "run=-1"),
		good.replace("run=19283746", "run=99999999999999999999999"),
		good.replace("run=19283746", "run=12 34"),
	];
	for body in forged {
		assert_eq!(Marker::parse(&body), None, "{body:?}");
	}
	assert!(Marker::parse(&good).is_some());
}

#[test]
fn marker_prefix_is_found_by_key() {
	let ci = CommentKey::parse("ci").unwrap();
	let e2e = CommentKey::parse("e2e").unwrap();
	let c = CommentKey::parse("c").unwrap();
	let body = marker().render().as_str().to_owned();
	assert!(Marker::prefix_matches(&body, &ci));
	assert!(!Marker::prefix_matches(&body, &e2e), "another key");
	assert!(
		!Marker::prefix_matches(&body, &c),
		"a prefix of the key is not the key"
	);
	assert!(!Marker::prefix_matches(&format!("x{body}"), &ci));
	assert!(
		!Marker::prefix_matches("<!-- pr-results:1 key=ci", &ci),
		"no space after the key"
	);
}

#[test]
fn counts_are_grouped_with_a_narrow_no_break_space() {
	let table = [
		(0, "0"),
		(7, "7"),
		(999, "999"),
		(1000, "1\u{202f}000"),
		(1978, "1\u{202f}978"),
		(696_000, "696\u{202f}000"),
		(1_234_567, "1\u{202f}234\u{202f}567"),
		(
			u64::MAX,
			"18\u{202f}446\u{202f}744\u{202f}073\u{202f}709\u{202f}551\u{202f}615",
		),
	];
	for (n, want) in table {
		assert_eq!(Safe::grouped(n).as_str(), want);
	}
}

#[test]
fn durations_read_at_a_glance() {
	let table = [
		(0, "0.0 s"),
		(400, "0.4 s"),
		(449, "0.4 s"),
		(450, "0.5 s"),
		(18_200, "18.2 s"),
		(59_949, "59.9 s"),
		(59_950, "1 m 0 s"),
		(60_000, "1 m 0 s"),
		(107_000, "1 m 47 s"),
		(372_000, "6 m 12 s"),
		(3_599_499, "59 m 59 s"),
		(3_599_500, "1 h 0 m"),
		(7_500_000, "2 h 5 m"),
		(86_400_000, "24 h 0 m"),
		(u64::MAX, "5124095576030 h 25 m"),
	];
	for (ms, want) in table {
		assert_eq!(Safe::duration_ms(ms).as_str(), want, "{ms} ms");
	}
}

#[test]
fn short_hashes_and_paths_are_code() {
	let sha = Sha::parse(SHA).unwrap();
	assert_eq!(Safe::sha_short(&sha).as_str(), "`3f9c2e1`");
	let path = RepoPath::parse("src/a`b.rs").unwrap();
	assert_eq!(Safe::path(&path).as_str(), "``src/a`b.rs``");
}

#[test]
fn plain_text_is_not_escaped_but_is_cleaned() {
	let text = Safe::plain_text("a *b* @c #1 <d>\nline\u{202e}x\u{1b}y", 100);
	assert_eq!(text.as_str(), "a *b* @c #1 <d> line\\u{202e}xy");
	let cut = Safe::plain_text("0123456789", 4);
	assert_eq!(cut.as_str(), "0123\u{2026} (6 more characters omitted)");
	let block = Safe::plain_block("one\r\ntwo\rthree\u{7}\nfour", 100);
	assert_eq!(block.as_str(), "one\ntwo\nthree\nfour");
	let cut_block = Safe::plain_block("one\ntwo\nthree", 5);
	assert_eq!(
		cut_block.as_str(),
		"one\nt\n\u{2026} (8 more characters omitted)"
	);
}

#[test]
fn after_first_line_drops_the_marker() {
	let whole = Safe::concat(&[marker().render(), Safe::trusted("\n### T\n\ntext")]);
	assert_eq!(whole.after_first_line().as_str(), "### T\n\ntext");
	assert_eq!(marker().render().after_first_line().as_str(), "");
}

#[test]
fn lines_are_cut_whole_and_a_fence_closer_is_only_backticks() {
	let text = Safe::trusted("a\nb\nc");
	assert_eq!(text.first_lines(0).as_str(), "");
	assert_eq!(text.first_lines(2).as_str(), "a\nb\n");
	assert_eq!(text.first_lines(9).as_str(), "a\nb\nc");
	assert_eq!(Safe::backticks(4).as_str(), "````");
	assert_eq!(Safe::backticks(100_000).as_str().len(), 65_536);
}
