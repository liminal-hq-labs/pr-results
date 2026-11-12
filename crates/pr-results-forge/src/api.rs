// The shape of a request and a response, and the `Forge` trait that carries them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The only place a token is added to a request is inside a [`Forge`] implementation, and only to
//! an [`ApiPath`] under its API base. A presigned blob URL is a [`BlobUrl`], a different type that
//! [`Forge::fetch_blob`] sends without any credentials, so the two cannot be mixed up.

use std::fmt;
use std::time::Duration;

/// The HTTP verbs the client uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
	Get,
	Post,
	Patch,
	Delete,
}

impl Method {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Get => "GET",
			Self::Post => "POST",
			Self::Patch => "PATCH",
			Self::Delete => "DELETE",
		}
	}

	/// Whether it changes anything on the server.
	pub fn is_write(self) -> bool {
		self != Self::Get
	}

	/// Whether sending it twice is the same as sending it once.
	pub fn is_idempotent(self) -> bool {
		self != Self::Post
	}
}

/// A path under the API base, with an optional query: `/repos/o/r/pulls/5?state=open`. Built from
/// parts the tool controls, or read from a `Link` header on the API base, so it holds only the
/// characters of such a path and never `..`, a second `/`, a `#` or a percent-encoded dot or slash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiPath(String);

const MAX_PATH_BYTES: usize = 2048;

impl ApiPath {
	pub fn parse(s: &str) -> Option<Self> {
		if s.len() > MAX_PATH_BYTES || !s.starts_with('/') {
			return None;
		}
		let (path, query) = match s.split_once('?') {
			Some((p, q)) => (p, Some(q)),
			None => (s, None),
		};
		let path_ok = path.bytes().all(|b| {
			b.is_ascii_alphanumeric()
				|| matches!(
					b,
					b'-' | b'.' | b'_' | b'~' | b'/' | b':' | b'@' | b'+' | b','
				)
		}) && path.strip_prefix('/').is_some_and(|p| {
			p.split('/')
				.all(|seg| !seg.is_empty() && seg != "." && seg != "..")
		});
		let query_ok = query.is_none_or(|q| {
			!q.contains('?')
				&& q.bytes().all(|b| {
					b.is_ascii_alphanumeric()
						|| matches!(
							b,
							b'-' | b'.'
								| b'_' | b'~' | b'%' | b':'
								| b'@' | b'+' | b',' | b'='
								| b'&' | b'/'
						)
				})
		});
		(path_ok && query_ok).then(|| Self(s.to_owned()))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}

	/// The part before the query.
	pub fn path(&self) -> &str {
		self.0.split_once('?').map_or(self.0.as_str(), |(p, _)| p)
	}

	pub fn query(&self) -> Option<&str> {
		self.0.split_once('?').map(|(_, q)| q)
	}
}

impl fmt::Display for ApiPath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.0)
	}
}

/// An `https://` URL that a response named, for the one download the API redirects: no user
/// information, no spaces or control characters. It is fetched without credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobUrl(String);

impl BlobUrl {
	pub fn parse(s: &str) -> Option<Self> {
		let rest = s.strip_prefix("https://")?;
		Self::check(s, rest)
	}

	/// For loopback tests only: the same rules for a plain `http://` URL.
	#[cfg(test)]
	pub(crate) fn parse_http_for_tests(s: &str) -> Option<Self> {
		let rest = s.strip_prefix("http://")?;
		Self::check(s, rest)
	}

	fn check(whole: &str, rest: &str) -> Option<Self> {
		if whole.len() > 4096 {
			return None;
		}
		let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
		let printable = whole.bytes().all(|b| {
			b.is_ascii_graphic()
				&& !matches!(
					b,
					b'"' | b'<' | b'>' | b'\\' | b'^' | b'`' | b'{' | b'|' | b'}'
				)
		});
		let host_ok =
			!authority.is_empty() && !authority.contains('@') && !authority.starts_with(':');
		(printable && host_ok).then(|| Self(whole.to_owned()))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}

	/// The host name, lower-cased, without a port.
	pub fn host(&self) -> String {
		host_of(&self.0)
	}
}

/// The host of an absolute URL, lower-cased, without a port or brackets.
pub(crate) fn host_of(url: &str) -> String {
	let rest = url.split_once("://").map_or(url, |(_, r)| r);
	let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
	let host = if let Some(v6) = authority.strip_prefix('[') {
		v6.split(']').next().unwrap_or("")
	} else {
		authority.split(':').next().unwrap_or("")
	};
	host.to_ascii_lowercase()
}

/// Percent-encodes everything but the unreserved characters.
pub fn percent_encode(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	for b in s.bytes() {
		if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
			out.push(char::from(b));
		} else {
			out.push_str(&format!("%{b:02X}"));
		}
	}
	out
}

/// The inverse of [`percent_encode`], for the fake. A malformed escape is kept as it is.
pub fn percent_decode(s: &str) -> String {
	let bytes = s.as_bytes();
	let mut out = Vec::with_capacity(bytes.len());
	let mut i = 0;
	while let Some(&b) = bytes.get(i) {
		if b == b'%' {
			let hex = bytes
				.get(i + 1..i + 3)
				.and_then(|h| std::str::from_utf8(h).ok())
				.and_then(|h| u8::from_str_radix(h, 16).ok());
			if let Some(v) = hex {
				out.push(v);
				i += 3;
				continue;
			}
		}
		out.push(if b == b'+' { b' ' } else { b });
		i += 1;
	}
	String::from_utf8_lossy(&out).into_owned()
}

/// One call to the API. There is no header here and no URL: the [`Forge`] adds both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiRequest {
	pub method: Method,
	pub path: ApiPath,
	pub query: Vec<(&'static str, String)>,
	/// A JSON document.
	pub body: Option<Vec<u8>>,
}

impl ApiRequest {
	pub fn get(path: ApiPath) -> Self {
		Self {
			method: Method::Get,
			path,
			query: Vec::new(),
			body: None,
		}
	}

	pub fn with_query(mut self, key: &'static str, value: impl Into<String>) -> Self {
		self.query.push((key, value.into()));
		self
	}

	pub fn json(method: Method, path: ApiPath, body: &serde_json::Value) -> Self {
		Self {
			method,
			path,
			query: Vec::new(),
			body: Some(body.to_string().into_bytes()),
		}
	}

	/// The path and query as they go on the wire, the query values percent-encoded and added to
	/// any the path already carries.
	pub fn target(&self) -> String {
		let mut out = self.path.as_str().to_owned();
		let mut sep = if self.path.query().is_some() {
			'&'
		} else {
			'?'
		};
		for (k, v) in &self.query {
			out.push(sep);
			out.push_str(k);
			out.push('=');
			out.push_str(&percent_encode(v));
			sep = '&';
		}
		out
	}
}

/// What came back. A status outside 200 to 299 is not an error here: the caller decides.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApiResponse {
	pub status: u16,
	/// The `rel="next"` link, if it is under the API base.
	pub next: Option<ApiPath>,
	pub retry_after_s: Option<u64>,
	pub rate_remaining: Option<u64>,
	pub rate_reset: Option<u64>,
	/// The `Location` of a redirect, if it is an `https://` URL.
	pub location: Option<BlobUrl>,
	/// Forgejo's `X-Total-Count`: how many items a list holds in all.
	pub total_count: Option<u64>,
	pub body: Vec<u8>,
}

impl ApiResponse {
	pub fn ok(&self) -> bool {
		(200..300).contains(&self.status)
	}

	pub fn json(&self) -> Result<serde_json::Value, ForgeError> {
		serde_json::from_slice(&self.body).map_err(|_| ForgeError::Decode("the body is not JSON"))
	}
}

/// Why a call could not be completed. None of these carries text from a response body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeError {
	/// The connection, the TLS handshake, a timeout or a read failed.
	Transport(String),
	/// The client declined to send or follow something, such as an `http://` redirect.
	Refused(&'static str),
	/// A body went over the limit it was read with.
	TooLarge { limit: u64 },
	/// A rate limit that will not lift within a minute.
	RateLimited { wait_s: u64 },
	/// A status the caller did not expect, for the operation named.
	Status { status: u16, op: &'static str },
	/// A response that is not what the API documents.
	Decode(&'static str),
}

impl fmt::Display for ForgeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Transport(why) => write!(f, "the request failed: {why}"),
			Self::Refused(why) => write!(f, "the request was refused: {why}"),
			Self::TooLarge { limit } => write!(f, "the response is larger than {limit} bytes"),
			Self::RateLimited { wait_s } => {
				write!(f, "the rate limit lifts in {wait_s} s, too long to wait")
			}
			Self::Status { status, op } => write!(f, "the forge answered {status} to {op}"),
			Self::Decode(why) => write!(f, "unexpected response: {why}"),
		}
	}
}

impl std::error::Error for ForgeError {}

impl ForgeError {
	/// The HTTP status, if the error is one.
	pub fn status(&self) -> Option<u16> {
		match self {
			Self::Status { status, .. } => Some(*status),
			_ => None,
		}
	}
}

/// The token, held so that it cannot be printed. It has no `Display`, and `Debug` writes a
/// placeholder.
#[derive(Clone)]
pub struct Token(String);

impl Token {
	/// Printable ASCII with no spaces, 1 to 512 bytes: what a GitHub token is.
	pub fn new(s: &str) -> Option<Self> {
		(!s.is_empty() && s.len() <= 512 && s.bytes().all(|b| b.is_ascii_graphic()))
			.then(|| Self(s.to_owned()))
	}

	/// For the `Authorization` header and nothing else.
	pub(crate) fn expose(&self) -> &str {
		&self.0
	}

	/// The workflow command that makes the runner hide the token in its logs. `Token::new` accepts
	/// printable characters only, so there is no CR or LF to escape, and a `%` is written `%25`.
	pub fn mask_command(&self) -> String {
		format!("::add-mask::{}", self.0.replace('%', "%25"))
	}
}

impl fmt::Debug for Token {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("Token(<redacted>)")
	}
}

/// What the publisher needs from the network. The real one is [`crate::UreqForge`]; tests use
/// [`crate::FakeForge`].
pub trait Forge {
	/// One call to the API base. This is the only place the token is added, and it is added to
	/// nothing else. Redirects are not followed.
	fn send(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError>;

	/// GETs `url` over HTTPS without an `Authorization` header and returns at most `max_bytes`.
	fn fetch_blob(&self, url: &BlobUrl, max_bytes: u64) -> Result<Vec<u8>, ForgeError>;

	fn sleep(&self, d: Duration);

	/// Seconds since the Unix epoch, for comparing with a rate limit's reset time.
	fn now_epoch(&self) -> u64;
}
