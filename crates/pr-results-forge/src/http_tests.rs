// Loopback tests for the real client: where the token goes, what a redirect may reach, limits.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use pr_results_core::RepoSlug;

use crate::api::{ApiPath, ApiRequest, BlobUrl, Forge, ForgeError, Method, Token};
use crate::github::GitHub;
use crate::host::{Artifact, Host};
use crate::ureq_forge::{next_from_link, UreqForge};

const SECRET: &str = "ghs_SECRETTOKENVALUE0123456789";

/// A one-thread HTTP server that answers each connection with the next canned reply and keeps
/// the raw request it read.
struct Server {
	port: u16,
	seen: Arc<Mutex<Vec<String>>>,
	handle: Option<JoinHandle<()>>,
}

fn response(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
	let mut out = format!(
		"HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n",
		body.len()
	);
	for (k, v) in headers {
		out.push_str(&format!("{k}: {v}\r\n"));
	}
	out.push_str("\r\n");
	let mut bytes = out.into_bytes();
	bytes.extend_from_slice(body);
	bytes
}

fn serve(host: &str, replies: Vec<Vec<u8>>) -> Server {
	let listener = TcpListener::bind((host, 0)).unwrap();
	let port = listener.local_addr().unwrap().port();
	listener.set_nonblocking(true).unwrap();
	let seen = Arc::new(Mutex::new(Vec::new()));
	let log = Arc::clone(&seen);
	let handle = std::thread::spawn(move || {
		for reply in replies {
			let deadline = Instant::now() + Duration::from_secs(10);
			let mut stream = loop {
				match listener.accept() {
					Ok((s, _)) => break s,
					Err(_) if Instant::now() < deadline => {
						std::thread::sleep(Duration::from_millis(5))
					}
					Err(_) => return,
				}
			};
			stream.set_nonblocking(false).unwrap();
			stream
				.set_read_timeout(Some(Duration::from_secs(5)))
				.unwrap();
			let mut raw = Vec::new();
			let mut buf = [0u8; 4096];
			loop {
				let n = stream.read(&mut buf).unwrap_or(0);
				if n == 0 {
					break;
				}
				raw.extend_from_slice(&buf[..n]);
				let text = String::from_utf8_lossy(&raw).into_owned();
				if let Some((head, body)) = text.split_once("\r\n\r\n") {
					let want: usize = head
						.lines()
						.find_map(|l| {
							l.to_ascii_lowercase()
								.strip_prefix("content-length:")
								.map(|v| v.trim().parse().unwrap_or(0))
						})
						.unwrap_or(0);
					if body.len() >= want {
						break;
					}
				}
			}
			log.lock()
				.unwrap()
				.push(String::from_utf8_lossy(&raw).into_owned());
			let _ = stream.write_all(&reply);
			let _ = stream.flush();
		}
	});
	Server {
		port,
		seen,
		handle: Some(handle),
	}
}

impl Server {
	fn url(&self, host: &str, path: &str) -> String {
		format!("http://{host}:{}{path}", self.port)
	}

	/// The requests read so far, once the server has finished answering.
	fn finish(mut self) -> Vec<String> {
		if let Some(h) = self.handle.take() {
			h.join().unwrap();
		}
		self.seen.lock().unwrap().clone()
	}
}

fn has_header(request: &str, name: &str) -> bool {
	request.lines().any(|l| {
		l.to_ascii_lowercase()
			.starts_with(&format!("{}:", name.to_ascii_lowercase()))
	})
}

fn header_value(request: &str, name: &str) -> Option<String> {
	request.lines().find_map(|l| {
		let (k, v) = l.split_once(':')?;
		k.eq_ignore_ascii_case(name).then(|| v.trim().to_owned())
	})
}

fn forge(base: &str) -> UreqForge {
	UreqForge::loopback(base, Token::new(SECRET).unwrap()).unwrap()
}

fn get(path: &str) -> ApiRequest {
	ApiRequest::get(ApiPath::parse(path).unwrap())
}

#[test]
fn an_api_call_carries_the_token_a_user_agent_and_nothing_private() {
	let api = serve("127.0.0.1", vec![response("200 OK", &[], b"{\"ok\":true}")]);
	let f = forge(&api.url("127.0.0.1", ""));
	let resp = f
		.send(&get("/repos/o/r/pulls/5").with_query("state", "open"))
		.unwrap();
	assert_eq!(resp.status, 200);
	assert_eq!(resp.body, b"{\"ok\":true}");
	let seen = api.finish();
	let req = &seen[0];
	assert!(
		req.starts_with("GET /repos/o/r/pulls/5?state=open HTTP/1.1"),
		"{req}"
	);
	assert_eq!(
		header_value(req, "authorization"),
		Some(format!("Bearer {SECRET}"))
	);
	let ua = header_value(req, "user-agent").unwrap();
	assert_eq!(
		ua,
		concat!("liminal-hq-pr-results/", env!("CARGO_PKG_VERSION"))
	);
	assert!(!ua.contains('@'), "no email address in the user agent");
	assert_eq!(
		header_value(req, "accept").as_deref(),
		Some("application/vnd.github+json")
	);
	assert_eq!(
		header_value(req, "x-github-api-version").as_deref(),
		Some("2022-11-28")
	);
}

#[test]
fn writes_send_their_json_and_the_query_is_percent_encoded() {
	let api = serve(
		"127.0.0.1",
		vec![
			response("201 Created", &[], b"{\"id\":1}"),
			response("200 OK", &[], b"{}"),
			response("204 No Content", &[], b""),
		],
	);
	let f = forge(&api.url("127.0.0.1", ""));
	let post = ApiRequest::json(
		Method::Post,
		ApiPath::parse("/repos/o/r/issues/5/comments").unwrap(),
		&serde_json::json!({ "body": "hi \u{e9}" }),
	);
	assert_eq!(f.send(&post).unwrap().status, 201);
	let patch = ApiRequest::json(
		Method::Patch,
		ApiPath::parse("/repos/o/r/issues/comments/9").unwrap(),
		&serde_json::json!({ "body": "x" }),
	);
	assert_eq!(f.send(&patch).unwrap().status, 200);
	let delete = ApiRequest {
		method: Method::Delete,
		..get("/repos/o/r/issues/comments/9").with_query("head", "forkuser:a b&c")
	};
	assert_eq!(f.send(&delete).unwrap().status, 204);
	let seen = api.finish();
	assert!(seen[0].starts_with("POST /repos/o/r/issues/5/comments HTTP/1.1"));
	assert_eq!(
		header_value(&seen[0], "content-type").as_deref(),
		Some("application/json")
	);
	assert!(seen[0].ends_with("{\"body\":\"hi \u{e9}\"}"), "{}", seen[0]);
	assert!(seen[1].starts_with("PATCH /repos/o/r/issues/comments/9 HTTP/1.1"));
	assert!(
		seen[2]
			.starts_with("DELETE /repos/o/r/issues/comments/9?head=forkuser%3Aa%20b%26c HTTP/1.1"),
		"{}",
		seen[2]
	);
}

#[test]
fn a_redirect_to_another_host_name_reaches_it_without_the_token() {
	// The blob server is reached as `localhost`, the API as `127.0.0.1`: two host names.
	let blob = serve(
		"localhost",
		vec![response("200 OK", &[], b"PK-the-zip-bytes")],
	);
	let api = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", &blob.url("localhost", "/artifact/zip?sig=abc"))],
			b"",
		)],
	);
	let f = forge(&api.url("127.0.0.1", ""));
	let gh = GitHub::new(f, RepoSlug::parse("o/r").unwrap(), "github-actions[bot]");
	let artifact = Artifact {
		id: 77,
		name: "rust-test-results".to_owned(),
		size_in_bytes: 16,
		expired: false,
	};
	let bytes = gh.download(&artifact, 1024).unwrap();
	assert_eq!(bytes, b"PK-the-zip-bytes");

	let api_seen = api.finish();
	assert!(api_seen[0].starts_with("GET /repos/o/r/actions/artifacts/77/zip HTTP/1.1"));
	assert!(
		has_header(&api_seen[0], "authorization"),
		"the API call is authorized"
	);
	let blob_seen = blob.finish();
	assert_eq!(blob_seen.len(), 1);
	assert!(
		blob_seen[0].starts_with("GET /artifact/zip?sig=abc HTTP/1.1"),
		"{}",
		blob_seen[0]
	);
	assert!(
		!has_header(&blob_seen[0], "authorization"),
		"{}",
		blob_seen[0]
	);
	assert!(!blob_seen[0].contains(SECRET));
	assert!(!blob_seen[0].to_ascii_lowercase().contains("bearer"));
}

#[test]
fn the_library_does_not_follow_an_api_redirect_by_itself() {
	// If it did, the second request would need a second reply and the first would not be seen.
	let api = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", "http://localhost:1/never")],
			b"",
		)],
	);
	let f = forge(&api.url("127.0.0.1", ""));
	let resp = f.send(&get("/repos/o/r/actions/artifacts/1/zip")).unwrap();
	assert_eq!(resp.status, 302);
	assert_eq!(api.finish().len(), 1);
}

#[test]
fn a_redirect_to_http_is_refused_and_never_followed() {
	// From the API: the Location is dropped, so the download fails before any fetch.
	let api = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", "http://blob.example.invalid/zip")],
			b"",
		)],
	);
	let f = forge(&api.url("127.0.0.1", ""));
	let resp = f.send(&get("/repos/o/r/actions/artifacts/1/zip")).unwrap();
	assert_eq!(resp.status, 302);
	assert_eq!(resp.location, None);
	api.finish();

	// Through the client the download fails at the redirect, before any fetch.
	let api = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", "http://blob.example.invalid/zip")],
			b"",
		)],
	);
	let gh = GitHub::new(
		forge(&api.url("127.0.0.1", "")),
		RepoSlug::parse("o/r").unwrap(),
		"x",
	);
	let artifact = Artifact {
		id: 1,
		name: "n".to_owned(),
		size_in_bytes: 1,
		expired: false,
	};
	assert!(matches!(
		gh.download(&artifact, 10),
		Err(ForgeError::Refused(_))
	));
	api.finish();

	// From a blob: a hop to http:// on another host is refused without a request.
	let blob = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", "http://elsewhere.example.invalid/zip")],
			b"",
		)],
	);
	let f = forge("http://127.0.0.1:1");
	let url = BlobUrl::parse_http_for_tests(&blob.url("127.0.0.1", "/z")).unwrap();
	let err = f.fetch_blob(&url, 1024).unwrap_err();
	assert!(matches!(err, ForgeError::Refused(_)), "{err:?}");
	assert_eq!(blob.finish().len(), 1);
}

#[test]
fn a_blob_may_redirect_only_a_few_times_and_never_with_the_token() {
	let last = serve("localhost", vec![response("200 OK", &[], b"done")]);
	let third = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", &last.url("localhost", "/c"))],
			b"",
		)],
	);
	let second = serve(
		"localhost",
		vec![response(
			"302 Found",
			&[("Location", &third.url("127.0.0.1", "/b"))],
			b"",
		)],
	);
	let first = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", &second.url("localhost", "/a"))],
			b"",
		)],
	);
	let f = forge("http://127.0.0.1:1");
	let url = BlobUrl::parse_http_for_tests(&first.url("127.0.0.1", "/start")).unwrap();
	assert_eq!(f.fetch_blob(&url, 1024).unwrap(), b"done");
	for s in [first, second, third, last] {
		for request in s.finish() {
			assert!(!has_header(&request, "authorization"));
		}
	}

	// Four redirects in a row is one too many.
	let end = serve("127.0.0.1", vec![response("200 OK", &[], b"x")]);
	let d = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", &end.url("127.0.0.1", "/"))],
			b"",
		)],
	);
	let c = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", &d.url("127.0.0.1", "/"))],
			b"",
		)],
	);
	let b = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", &c.url("127.0.0.1", "/"))],
			b"",
		)],
	);
	let a = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", &b.url("127.0.0.1", "/"))],
			b"",
		)],
	);
	let url = BlobUrl::parse_http_for_tests(&a.url("127.0.0.1", "/")).unwrap();
	let err = f.fetch_blob(&url, 1024).unwrap_err();
	assert_eq!(err, ForgeError::Refused("too many redirects"));
	drop((a, b, c, d, end));
}

#[test]
fn a_blob_is_read_to_the_limit_and_no_further() {
	let blob = serve("127.0.0.1", vec![response("200 OK", &[], b"0123456789")]);
	let f = forge("http://127.0.0.1:1");
	let url = BlobUrl::parse_http_for_tests(&blob.url("127.0.0.1", "/z")).unwrap();
	assert_eq!(
		f.fetch_blob(&url, 5).unwrap_err(),
		ForgeError::TooLarge { limit: 5 }
	);
	blob.finish();

	let blob = serve("127.0.0.1", vec![response("200 OK", &[], b"0123456789")]);
	let url = BlobUrl::parse_http_for_tests(&blob.url("127.0.0.1", "/z")).unwrap();
	assert_eq!(f.fetch_blob(&url, 10).unwrap().len(), 10);
	blob.finish();

	let blob = serve("127.0.0.1", vec![response("404 Not Found", &[], b"no")]);
	let url = BlobUrl::parse_http_for_tests(&blob.url("127.0.0.1", "/z")).unwrap();
	assert_eq!(
		f.fetch_blob(&url, 10).unwrap_err(),
		ForgeError::Status {
			status: 404,
			op: "fetching a blob"
		}
	);
	blob.finish();
}

#[test]
fn statuses_and_rate_limit_headers_come_back_as_values() {
	let api = serve(
		"127.0.0.1",
		vec![
			response(
				"403 Forbidden",
				&[
					("Retry-After", "17"),
					("X-RateLimit-Remaining", "0"),
					("X-RateLimit-Reset", "1700000123"),
				],
				b"{\"message\":\"slow down\"}",
			),
			response("500 Internal Server Error", &[], b""),
			response("404 Not Found", &[], b"{}"),
		],
	);
	let f = forge(&api.url("127.0.0.1", ""));
	let r = f.send(&get("/repos/o/r/pulls")).unwrap();
	assert_eq!(
		(r.status, r.retry_after_s, r.rate_remaining, r.rate_reset),
		(403, Some(17), Some(0), Some(1_700_000_123))
	);
	assert_eq!(f.send(&get("/repos/o/r/pulls")).unwrap().status, 500);
	assert_eq!(f.send(&get("/repos/o/r/pulls")).unwrap().status, 404);
	api.finish();
}

#[test]
fn the_link_header_is_followed_only_under_the_api_base() {
	let base = "https://api.github.com";
	let link = |url: &str| {
		format!("<{url}>; rel=\"next\", <https://api.github.com/x?page=9>; rel=\"last\"")
	};
	let ok = next_from_link(
		base,
		&link("https://api.github.com/repos/o/r/issues/5/comments?per_page=100&page=2"),
	);
	assert_eq!(
		ok.unwrap().as_str(),
		"/repos/o/r/issues/5/comments?per_page=100&page=2"
	);
	for bad in [
		"https://evil.example/repos/o/r?page=2",
		"http://api.github.com/repos/o/r?page=2",
		"https://api.github.com.evil.example/repos/o/r?page=2",
		"https://api.github.com@evil.example/repos/o/r",
		"https://api.github.com/../etc?page=2",
		"https://api.github.com//evil.example/x",
		"https://api.github.com/repos/o/r#frag",
		"/repos/o/r?page=2",
	] {
		assert_eq!(next_from_link(base, &link(bad)), None, "{bad}");
	}
	assert_eq!(
		next_from_link(base, "<https://api.github.com/x?page=1>; rel=\"prev\""),
		None
	);
	assert_eq!(next_from_link(base, "garbage"), None);
	assert_eq!(next_from_link(base, ""), None);
	// The `last` link alone is not a next page.
	assert_eq!(
		next_from_link(base, "<https://api.github.com/x?page=9>; rel=\"last\""),
		None
	);
	// A Server base with a path.
	let ghes = "https://ghe.example/api/v3";
	let got = next_from_link(
		ghes,
		"<https://ghe.example/api/v3/repos/o/r/pulls?page=2>; rel=\"next\"",
	);
	assert_eq!(got.unwrap().as_str(), "/repos/o/r/pulls?page=2");
}

#[test]
fn the_base_must_be_https() {
	let t = || Token::new(SECRET).unwrap();
	assert!(UreqForge::new("https://api.github.com", t()).is_ok());
	assert!(UreqForge::new("https://ghe.example/api/v3/", t()).is_ok());
	for bad in [
		"http://api.github.com",
		"api.github.com",
		"https://user@api.github.com",
		"https://api.github.com?x=1",
		"ftp://api.github.com",
		"",
		"http://127.0.0.1:8080",
	] {
		assert!(UreqForge::new(bad, t()).is_err(), "{bad}");
	}
}

#[test]
fn the_token_cannot_be_printed() {
	let t = Token::new(SECRET).unwrap();
	assert_eq!(format!("{t:?}"), "Token(<redacted>)");
	assert!(!format!("{:?}", forge("http://127.0.0.1:1")).contains(SECRET));
	assert_eq!(t.mask_command(), format!("::add-mask::{SECRET}"));
	assert_eq!(
		Token::new("a%b").unwrap().mask_command(),
		"::add-mask::a%25b"
	);
	for bad in [
		"",
		"has space",
		"line\nbreak",
		"tab\t",
		"\u{e9}",
		&"x".repeat(513),
	] {
		assert!(Token::new(bad).is_none(), "{bad:?}");
	}
	assert!(Token::new(&"x".repeat(512)).is_some());
	let e = forge("http://127.0.0.1:1")
		.send(&get("/repos/o/r"))
		.unwrap_err();
	assert!(!e.to_string().contains(SECRET), "{e}");
}

#[test]
fn a_dead_server_is_a_transport_error_with_fixed_words() {
	let port = {
		let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
		l.local_addr().unwrap().port()
	};
	let f = forge(&format!("http://127.0.0.1:{port}"));
	let e = f.send(&get("/repos/o/r")).unwrap_err();
	assert!(
		matches!(&e, ForgeError::Transport(w) if !w.contains("127.0.0.1")),
		"{e:?}"
	);
}

#[test]
fn a_blob_host_must_be_on_the_list() {
	use crate::ureq_forge::{host_allowed, GITHUB_BLOB_HOSTS};
	let github: Vec<String> = GITHUB_BLOB_HOSTS.iter().map(|h| (*h).to_owned()).collect();
	for good in [
		"productionresultssa1.blob.core.windows.net",
		"pipelines.actions.githubusercontent.com",
		"objects.githubusercontent.com",
		"a.b.githubusercontent.com",
	] {
		assert!(host_allowed(&github, good), "{good}");
	}
	for bad in [
		"githubusercontent.com",
		"blob.core.windows.net",
		"evilgithubusercontent.com",
		"githubusercontent.com.evil.example",
		"evil.example",
		"blob.core.windows.net.evil.example",
		"127.0.0.1",
		"::1",
		"",
		".githubusercontent.com",
	] {
		assert!(!host_allowed(&github, bad), "{bad:?}");
	}
	assert!(host_allowed(&["ghe.example".to_owned()], "ghe.example"));
	assert!(!host_allowed(&["ghe.example".to_owned()], "x.ghe.example"));
	assert!(host_allowed(&["10.0.0.5".to_owned()], "10.0.0.5"));
}

#[test]
fn the_default_hosts_follow_the_api_base() {
	let t = || Token::new(SECRET).unwrap();
	let gh = UreqForge::new("https://api.github.com", t()).unwrap();
	let url = |s: &str| BlobUrl::parse(s).unwrap();
	assert!(gh.blob_host_allowed(&url("https://pipelines.actions.githubusercontent.com/x")));
	assert!(!gh.blob_host_allowed(&url("https://evil.example/x")));
	let ghes = UreqForge::new("https://ghe.example/api/v3", t()).unwrap();
	assert!(ghes.blob_host_allowed(&url("https://ghe.example/x")));
	assert!(!ghes.blob_host_allowed(&url("https://pipelines.actions.githubusercontent.com/x")));
	let configured = ghes.with_blob_hosts(&["ghe.example", "*.Storage.Example"]);
	assert!(configured.blob_host_allowed(&url("https://a.storage.example/x")));
}

#[test]
fn a_blob_from_an_unexpected_host_is_refused_before_any_request() {
	// Straight to it.
	let f = UreqForge::new("https://api.github.com", Token::new(SECRET).unwrap()).unwrap();
	let url = BlobUrl::parse("https://evil.example/zip").unwrap();
	assert_eq!(
		f.fetch_blob(&url, 10).unwrap_err(),
		ForgeError::Refused("the blob host is not one GitHub uses")
	);
	// And by a redirect from an allowed host: the second hop is the one refused, unrequested.
	let first = serve(
		"127.0.0.1",
		vec![response(
			"302 Found",
			&[("Location", "https://evil.example/zip")],
			b"",
		)],
	);
	let f = forge("http://127.0.0.1:1");
	let url = BlobUrl::parse_http_for_tests(&first.url("127.0.0.1", "/start")).unwrap();
	assert!(matches!(
		f.fetch_blob(&url, 10),
		Err(ForgeError::Refused(_))
	));
	assert_eq!(first.finish().len(), 1, "only the allowed host was asked");
}
