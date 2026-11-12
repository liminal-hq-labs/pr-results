// Tests for the request and response types: what an API path and a blob URL can hold.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use pr_results_github::{
	percent_decode, percent_encode, ApiPath, ApiRequest, BlobUrl, ForgeError, Method, Token,
};

#[test]
fn an_api_path_is_a_path_and_nothing_else() {
	for good in [
		"/repos/o/r",
		"/repos/o/r/pulls/5",
		"/repos/o/r/commits/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334/check-runs?check_name=Example%20Rust%20tests&filter=latest",
		"/repos/o/r/issues/5/comments?per_page=100&page=2",
		"/repos/o/r/pulls?head=forkuser%3Apatch-1&state=open",
	] {
		let p = ApiPath::parse(good).unwrap();
		assert_eq!(p.as_str(), good);
	}
	for bad in [
		"",
		"repos/o/r",
		"//evil.example/x",
		"/repos//r",
		"/repos/o/../r",
		"/repos/o/./r",
		"/repos/o/r#frag",
		"/repos/o/r?a=1#frag",
		"/repos/o/r?a=1?b=2",
		"/repos/o r",
		"/repos/o/r\n",
		"/repos/o/r\\x",
		"/repos/%2e%2e/r",
		"/repos/o%2fr",
		"/repos/o/r?x=<",
		"https://evil.example/repos",
		"/\u{e9}",
	] {
		assert_eq!(ApiPath::parse(bad), None, "{bad:?}");
	}
	assert_eq!(ApiPath::parse(&format!("/{}", "a".repeat(2100))), None);
	let p = ApiPath::parse("/repos/o/r/pulls?state=open").unwrap();
	assert_eq!(p.path(), "/repos/o/r/pulls");
	assert_eq!(p.query(), Some("state=open"));
	assert_eq!(ApiPath::parse("/repos/o/r").unwrap().query(), None);
}

#[test]
fn a_blob_url_is_https_with_a_host_and_no_user() {
	for good in [
		"https://pipelines.actions.githubusercontent.com/abc/zip?sig=x%3D&se=2026",
		"https://productionresultssa1.blob.core.windows.net/a/b?sv=1",
		"https://example.test:8443/x",
	] {
		assert_eq!(BlobUrl::parse(good).unwrap().as_str(), good);
	}
	for bad in [
		"http://example.test/x",
		"ftp://example.test/x",
		"https://",
		"https:///x",
		"https://user@example.test/x",
		"https://user:pw@example.test/x",
		"https://example.test /x",
		"https://example.test/x\r\nX-Evil: 1",
		"https://example.test/\"x",
		"https://exam\\ple.test/x",
		"https://:443/x",
		"//example.test/x",
		"/x",
		"",
	] {
		assert_eq!(BlobUrl::parse(bad), None, "{bad:?}");
	}
	assert_eq!(
		BlobUrl::parse("https://Example.TEST:8443/x")
			.unwrap()
			.host(),
		"example.test"
	);
	assert_eq!(
		BlobUrl::parse("https://[::1]:8443/x").unwrap().host(),
		"::1"
	);
	let long = format!("https://example.test/{}", "a".repeat(5000));
	assert_eq!(BlobUrl::parse(&long), None);
}

#[test]
fn percent_encoding_round_trips_and_keeps_only_unreserved_characters() {
	assert_eq!(
		percent_encode("forkuser:feature/a b&c=d"),
		"forkuser%3Afeature%2Fa%20b%26c%3Dd"
	);
	assert_eq!(percent_encode("AZaz09-._~"), "AZaz09-._~");
	for s in [
		"plain",
		"a b",
		"\u{e9}\u{1f600}",
		"a%b",
		"x=1&y=2",
		"../etc",
	] {
		assert_eq!(percent_decode(&percent_encode(s)), s);
	}
	assert_eq!(percent_decode("%zz%4"), "%zz%4", "a bad escape is kept");
}

#[test]
fn a_request_target_joins_path_and_query() {
	let path = ApiPath::parse("/repos/o/r/pulls").unwrap();
	let req = ApiRequest::get(path)
		.with_query("head", "forkuser:patch-1")
		.with_query("state", "open");
	assert_eq!(
		req.target(),
		"/repos/o/r/pulls?head=forkuser%3Apatch-1&state=open"
	);
	let paged = ApiPath::parse("/repos/o/r/pulls?page=2").unwrap();
	assert_eq!(
		ApiRequest::get(paged).with_query("x", "y").target(),
		"/repos/o/r/pulls?page=2&x=y"
	);
	assert!(Method::Post.is_write() && !Method::Get.is_write());
	assert!(!Method::Post.is_idempotent() && Method::Patch.is_idempotent());
}

#[test]
fn errors_say_what_failed_without_quoting_a_response() {
	let all = [
		ForgeError::Transport("timed out".to_owned()),
		ForgeError::Refused("no"),
		ForgeError::TooLarge { limit: 5 },
		ForgeError::RateLimited { wait_s: 90 },
		ForgeError::Status {
			status: 403,
			op: "writing",
		},
		ForgeError::Decode("a list was expected"),
	];
	for e in all {
		assert!(!e.to_string().is_empty());
	}
	assert_eq!(
		ForgeError::Status {
			status: 403,
			op: "x"
		}
		.status(),
		Some(403)
	);
	assert_eq!(ForgeError::Decode("x").status(), None);
	assert!(Token::new("ghs_abc").is_some());
}
