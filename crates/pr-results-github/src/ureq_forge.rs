// The real `Forge`: HTTPS to the GitHub API with `ureq`, `rustls` (ring) and the webpki roots.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Redirects are never followed by the library. The API answers an artefact download with a 302
//! to a presigned blob URL, which [`Forge::fetch_blob`] fetches in a separate request that has no
//! `Authorization` header and is allowed to redirect only to other `https://` URLs. A token is
//! added in one place, [`UreqForge::send`], and only to a URL built from the API base and an
//! [`ApiPath`], which cannot hold a host. Errors are described by fixed words, never by the text
//! of an error from the library, which may quote a URL or a header.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ureq::Agent;

use crate::api::{ApiPath, ApiRequest, ApiResponse, BlobUrl, Forge, ForgeError, Method, Token};

/// API responses are read to at most this many bytes: a large repository's tree is the biggest.
const MAX_API_BODY: u64 = 64 * 1024 * 1024;
/// A blob redirect may itself redirect this many times.
const MAX_BLOB_HOPS: usize = 3;
const USER_AGENT: &str = concat!("liminal-hq-pr-results/", env!("CARGO_PKG_VERSION"));

pub struct UreqForge {
	/// `https://api.github.com`, with no trailing slash.
	base: String,
	token: Token,
	api: Agent,
	blob: Agent,
	/// Tests only: plain `http://` to a loopback address is allowed.
	#[cfg(test)]
	allow_loopback_http: bool,
}

impl std::fmt::Debug for UreqForge {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("UreqForge")
			.field("base", &self.base)
			.field("token", &self.token)
			.finish_non_exhaustive()
	}
}

fn agent(timeout: Duration, https_only: bool) -> Agent {
	Agent::config_builder()
		.max_redirects(0)
		.http_status_as_error(false)
		.https_only(https_only)
		.user_agent(USER_AGENT)
		.timeout_connect(Some(Duration::from_secs(15)))
		.timeout_global(Some(timeout))
		.build()
		.into()
}

impl UreqForge {
	/// `api_base` is `https://api.github.com`, or a GitHub Enterprise Server's
	/// `https://host/api/v3`: HTTPS, no user information, no query or fragment.
	pub fn new(api_base: &str, token: Token) -> Result<Self, ForgeError> {
		Self::build(api_base, token, false)
	}

	/// Tests only: allows `http://127.0.0.1` and `http://localhost` bases and blobs.
	#[cfg(test)]
	pub(crate) fn loopback(api_base: &str, token: Token) -> Result<Self, ForgeError> {
		Self::build(api_base, token, true)
	}

	fn build(api_base: &str, token: Token, loopback: bool) -> Result<Self, ForgeError> {
		let trimmed = api_base.trim_end_matches('/');
		let valid = if loopback {
			is_loopback_http(trimmed) || BlobUrl::parse(trimmed).is_some()
		} else {
			BlobUrl::parse(trimmed).is_some()
		};
		if !valid || trimmed.contains(['?', '#']) {
			return Err(ForgeError::Refused("the API base must be an https:// URL"));
		}
		Ok(Self {
			base: trimmed.to_owned(),
			token,
			api: agent(Duration::from_secs(60), !loopback),
			blob: agent(Duration::from_secs(180), !loopback),
			#[cfg(test)]
			allow_loopback_http: loopback,
		})
	}

	#[cfg(test)]
	fn blob_url(&self, location: &str) -> Option<BlobUrl> {
		if self.allow_loopback_http && is_loopback_http(location) {
			return BlobUrl::parse_http_for_tests(location);
		}
		BlobUrl::parse(location)
	}

	#[cfg(not(test))]
	fn blob_url(&self, location: &str) -> Option<BlobUrl> {
		BlobUrl::parse(location)
	}
}

#[cfg(test)]
fn is_loopback_http(url: &str) -> bool {
	url.strip_prefix("http://").is_some()
		&& matches!(
			crate::api::host_of(url).as_str(),
			"127.0.0.1" | "localhost" | "::1"
		)
}

#[cfg(not(test))]
fn is_loopback_http(_url: &str) -> bool {
	false
}

/// A short, fixed description of a transport failure. The library's own text can quote a URL.
fn describe(e: &ureq::Error) -> &'static str {
	match e {
		ureq::Error::Timeout(_) => "timed out",
		ureq::Error::HostNotFound => "host not found",
		ureq::Error::ConnectionFailed => "could not connect",
		ureq::Error::Tls(_) | ureq::Error::Rustls(_) => "TLS failed",
		ureq::Error::Io(_) => "connection failed",
		ureq::Error::BodyStalled => "the response stalled",
		ureq::Error::LargeResponseHeader(..) => "the response headers are too large",
		_ => "the request could not be completed",
	}
}

fn header<'a>(resp: &'a ureq::http::Response<ureq::Body>, name: &str) -> Option<&'a str> {
	resp.headers().get(name).and_then(|v| v.to_str().ok())
}

fn number(resp: &ureq::http::Response<ureq::Body>, name: &str) -> Option<u64> {
	header(resp, name).and_then(|v| v.trim().parse().ok())
}

/// The `rel="next"` URL of a `Link` header as a path under `base`, if it is one. A link to
/// another host, another scheme or a host that merely begins with the base's is dropped, so
/// paging cannot send the token elsewhere.
pub(crate) fn next_from_link(base: &str, link: &str) -> Option<ApiPath> {
	for part in link.split(',') {
		let mut pieces = part.split(';');
		let url = pieces.next()?.trim().strip_prefix('<')?.strip_suffix('>')?;
		let is_next = pieces.any(|p| {
			let p = p.trim();
			p == "rel=\"next\"" || p == "rel=next"
		});
		if is_next {
			let rest = url.strip_prefix(base)?;
			return ApiPath::parse(rest);
		}
	}
	None
}

/// The body, which may be exactly `limit` bytes and no more. The library's own limit errors at
/// the limit, so it is asked for one byte more and the length is checked here.
fn read_limited(
	resp: &mut ureq::http::Response<ureq::Body>,
	limit: u64,
) -> Result<Vec<u8>, ForgeError> {
	let too_large = ForgeError::TooLarge { limit };
	let body = resp
		.body_mut()
		.with_config()
		.limit(limit.saturating_add(1))
		.read_to_vec()
		.map_err(|e| match e {
			ureq::Error::BodyExceedsLimit(_) => too_large.clone(),
			other => ForgeError::Transport(describe(&other).to_owned()),
		})?;
	if body.len() as u64 > limit {
		return Err(too_large);
	}
	Ok(body)
}

/// The headers every API call carries. The token goes here and nowhere else.
fn common<B>(b: ureq::RequestBuilder<B>, auth: &str) -> ureq::RequestBuilder<B> {
	b.header("Authorization", auth)
		.header("Accept", "application/vnd.github+json")
		.header("X-GitHub-Api-Version", "2022-11-28")
}

impl Forge for UreqForge {
	fn send(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		let url = format!("{}{}", self.base, req.target());
		let auth = format!("Bearer {}", self.token.expose());
		let body = req.body.as_deref();
		let result = match req.method {
			Method::Get => common(self.api.get(&url), &auth).call(),
			Method::Delete => common(self.api.delete(&url), &auth).call(),
			Method::Post => common(self.api.post(&url), &auth)
				.content_type("application/json")
				.send(body.unwrap_or(b"")),
			Method::Patch => common(self.api.patch(&url), &auth)
				.content_type("application/json")
				.send(body.unwrap_or(b"")),
		};
		let mut resp = result.map_err(|e| ForgeError::Transport(describe(&e).to_owned()))?;
		let status = resp.status().as_u16();
		let next = header(&resp, "link").and_then(|l| next_from_link(&self.base, l));
		let location = if (300..400).contains(&status) {
			header(&resp, "location").and_then(|l| self.blob_url(l))
		} else {
			None
		};
		let out = ApiResponse {
			status,
			next,
			retry_after_s: number(&resp, "retry-after"),
			rate_remaining: number(&resp, "x-ratelimit-remaining"),
			rate_reset: number(&resp, "x-ratelimit-reset"),
			location,
			body: Vec::new(),
		};
		if (300..400).contains(&status) {
			return Ok(out);
		}
		let body = read_limited(&mut resp, MAX_API_BODY)?;
		Ok(ApiResponse { body, ..out })
	}

	fn fetch_blob(&self, url: &BlobUrl, max_bytes: u64) -> Result<Vec<u8>, ForgeError> {
		let mut current = url.clone();
		for _ in 0..=MAX_BLOB_HOPS {
			// No `Authorization`, and nothing else the API client adds: a presigned URL carries
			// its own permission and must not learn the token.
			let mut resp = self
				.blob
				.get(current.as_str())
				.call()
				.map_err(|e| ForgeError::Transport(describe(&e).to_owned()))?;
			let status = resp.status().as_u16();
			if (300..400).contains(&status) {
				let next = header(&resp, "location")
					.and_then(|l| self.blob_url(l))
					.ok_or(ForgeError::Refused("a redirect to a URL that is not https"))?;
				current = next;
				continue;
			}
			if !(200..300).contains(&status) {
				return Err(ForgeError::Status {
					status,
					op: "fetching a blob",
				});
			}
			return read_limited(&mut resp, max_bytes);
		}
		Err(ForgeError::Refused("too many redirects"))
	}

	fn sleep(&self, d: Duration) {
		std::thread::sleep(d);
	}

	fn now_epoch(&self) -> u64 {
		SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map(|d| d.as_secs())
			.unwrap_or(0)
	}
}
