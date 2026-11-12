// The real `Forge`: HTTPS to a forge's API with `ureq`, `rustls` (ring) and the webpki roots or one private CA.
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

use ureq::tls::{Certificate, PemItem, RootCerts, TlsConfig};
use ureq::Agent;

use crate::api::{ApiPath, ApiRequest, ApiResponse, BlobUrl, Forge, ForgeError, Method, Token};
use crate::host::HostKind;

/// API responses are read to at most this many bytes: a large repository's tree is the biggest.
const MAX_API_BODY: u64 = 64 * 1024 * 1024;
/// A blob redirect may itself redirect this many times.
const MAX_BLOB_HOPS: usize = 3;
const USER_AGENT: &str = concat!("liminal-hq-pr-results/", env!("CARGO_PKG_VERSION"));

pub struct UreqForge {
	kind: HostKind,
	/// `https://api.github.com` or `https://git.example/api/v1`, with no trailing slash.
	base: String,
	token: Token,
	api: Agent,
	blob: Agent,
	/// The hosts a blob may be fetched from: a host name, or `*.suffix` for any name under it.
	blob_hosts: Vec<String>,
	/// Tests only: plain `http://` to a loopback address is allowed.
	#[cfg(test)]
	allow_loopback_http: bool,
}

impl std::fmt::Debug for UreqForge {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("UreqForge")
			.field("kind", &self.kind)
			.field("base", &self.base)
			.field("token", &self.token)
			.finish_non_exhaustive()
	}
}

/// Where GitHub keeps the artefacts and logs it redirects to: Azure blob storage
/// (`productionresultssa*.blob.core.windows.net` and its relatives) and `githubusercontent.com`
/// (`pipelines.actions.githubusercontent.com` and its relatives).
pub const GITHUB_BLOB_HOSTS: [&str; 2] = ["*.blob.core.windows.net", "*.githubusercontent.com"];

fn default_blob_hosts(api_host: &str) -> Vec<String> {
	if api_host == "api.github.com" {
		GITHUB_BLOB_HOSTS.iter().map(|h| (*h).to_owned()).collect()
	} else {
		vec![api_host.to_owned()]
	}
}

/// Whether `host` matches one of `patterns`: equal to it, or, for `*.suffix`, a name with at least
/// one more label in front of `.suffix`. An IP address matches only by being listed.
pub(crate) fn host_allowed(patterns: &[String], host: &str) -> bool {
	patterns.iter().any(|p| match p.strip_prefix("*.") {
		Some(suffix) => {
			host.strip_suffix(suffix)
				.and_then(|front| front.strip_suffix('.'))
				.is_some_and(|front| !front.is_empty())
				&& !host.contains(':')
				&& !host.bytes().all(|b| b.is_ascii_digit() || b == b'.')
		}
		None => p == host,
	})
}

/// A private certificate authority for a forge on a LAN: the PEM certificates a person chose, and
/// nothing else. A client built with one trusts these roots and not the webpki roots.
#[derive(Clone)]
pub struct CaBundle {
	certs: Vec<Certificate<'static>>,
}

impl std::fmt::Debug for CaBundle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CaBundle")
			.field("certificates", &self.certs.len())
			.finish()
	}
}

/// Why a CA file cannot be used. Each is a fixed phrase and none quotes the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaError {
	/// More than [`CaBundle::MAX_BYTES`].
	TooLarge,
	/// No PEM certificate in it: empty, or not PEM.
	NoCertificate,
	/// A PEM section that is not a certificate, such as a private key.
	NotOnlyCertificates,
	/// A certificate section that is not valid base64 or not a DER certificate.
	Malformed,
	/// More than [`CaBundle::MAX_CERTS`] certificates.
	TooMany,
}

impl std::fmt::Display for CaError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::TooLarge => "it is larger than 1 MiB",
			Self::NoCertificate => "it holds no PEM certificate",
			Self::NotOnlyCertificates => {
				"it holds something other than certificates (a private key does not belong in it)"
			}
			Self::Malformed => "a certificate in it is damaged",
			Self::TooMany => "it holds more than 32 certificates",
		})
	}
}

impl std::error::Error for CaError {}

impl CaBundle {
	pub const MAX_BYTES: usize = 1024 * 1024;
	pub const MAX_CERTS: usize = 32;

	/// Reads the certificates of a PEM file. Anything but certificates is refused, so a file that
	/// also holds a private key is an error and not a quiet success.
	pub fn from_pem(pem: &[u8]) -> Result<Self, CaError> {
		if pem.len() > Self::MAX_BYTES {
			return Err(CaError::TooLarge);
		}
		let mut certs = Vec::new();
		for item in ureq::tls::parse_pem(pem) {
			match item {
				Ok(PemItem::Certificate(c)) => {
					if !looks_like_der(c.der()) {
						return Err(CaError::Malformed);
					}
					certs.push(c);
					if certs.len() > Self::MAX_CERTS {
						return Err(CaError::TooMany);
					}
				}
				Ok(_) => return Err(CaError::NotOnlyCertificates),
				Err(_) => return Err(CaError::Malformed),
			}
		}
		if certs.is_empty() {
			return Err(CaError::NoCertificate);
		}
		Ok(Self { certs })
	}

	/// How many certificates it trusts.
	pub fn len(&self) -> usize {
		self.certs.len()
	}

	pub fn is_empty(&self) -> bool {
		self.certs.is_empty()
	}

	fn roots(&self) -> RootCerts {
		RootCerts::new_with_certs(&self.certs)
	}
}

/// Whether `der` is one DER SEQUENCE with a definite length that covers it exactly: all that is
/// checked here. Whether it is a certificate the TLS library decides at the handshake.
fn looks_like_der(der: &[u8]) -> bool {
	let [0x30, rest @ ..] = der else {
		return false;
	};
	let Some((&first, tail)) = rest.split_first() else {
		return false;
	};
	let (len, header) = if first < 0x80 {
		(usize::from(first), 1usize)
	} else {
		let n = usize::from(first & 0x7f);
		if !(1..=4).contains(&n) {
			return false;
		}
		let Some(bytes) = tail.get(..n) else {
			return false;
		};
		(
			bytes
				.iter()
				.fold(0usize, |acc, b| (acc << 8) | usize::from(*b)),
			1 + n,
		)
	};
	rest.len() == header.saturating_add(len)
}

fn agent(timeout: Duration, https_only: bool, ca: Option<&CaBundle>) -> Agent {
	let mut config = Agent::config_builder();
	if let Some(ca) = ca {
		config = config.tls_config(TlsConfig::builder().root_certs(ca.roots()).build());
	}
	config
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
		Self::build(HostKind::GitHub, api_base, token, false)
	}

	/// The client for a Forgejo whose API is at `api_base`, such as `https://git.example/api/v1`:
	/// HTTPS only, no user information, no query or fragment.
	pub fn forgejo(api_base: &str, token: Token) -> Result<Self, ForgeError> {
		Self::build(HostKind::Forgejo, api_base, token, false)
	}

	/// Trusts the certificates of `ca` and nothing else, for this client's API and blob requests:
	/// the webpki roots are not consulted. For a forge on a LAN whose certificate a private
	/// authority signed.
	pub fn with_ca(mut self, ca: &CaBundle) -> Self {
		self.api = agent(Duration::from_secs(60), self.https_only(), Some(ca));
		self.blob = agent(Duration::from_secs(180), self.https_only(), Some(ca));
		self
	}

	fn https_only(&self) -> bool {
		#[cfg(test)]
		{
			!self.allow_loopback_http
		}
		#[cfg(not(test))]
		{
			true
		}
	}

	/// Tests only: allows `http://127.0.0.1` and `http://localhost` bases and blobs.
	#[cfg(test)]
	pub(crate) fn loopback(api_base: &str, token: Token) -> Result<Self, ForgeError> {
		Self::build(HostKind::GitHub, api_base, token, true)
	}

	fn build(
		kind: HostKind,
		api_base: &str,
		token: Token,
		loopback: bool,
	) -> Result<Self, ForgeError> {
		let trimmed = api_base.trim_end_matches('/');
		let valid = if loopback {
			is_loopback_http(trimmed) || BlobUrl::parse(trimmed).is_some()
		} else {
			BlobUrl::parse(trimmed).is_some()
		};
		if !valid || trimmed.contains(['?', '#']) {
			return Err(ForgeError::Refused("the API base must be an https:// URL"));
		}
		let blob_hosts = if loopback {
			vec!["127.0.0.1".to_owned(), "localhost".to_owned()]
		} else {
			default_blob_hosts(&crate::api::host_of(trimmed))
		};
		Ok(Self {
			kind,
			blob_hosts,
			base: trimmed.to_owned(),
			token,
			api: agent(Duration::from_secs(60), !loopback, None),
			blob: agent(Duration::from_secs(180), !loopback, None),
			#[cfg(test)]
			allow_loopback_http: loopback,
		})
	}

	/// Replaces the hosts a blob may be fetched from. An entry is a host name or `*.suffix`, which
	/// matches any name under the suffix but not the suffix itself. The default for github.com is
	/// [`GITHUB_BLOB_HOSTS`], and for another API host that host alone, so a GitHub Enterprise
	/// Server whose artefacts live in other storage names it here.
	pub fn with_blob_hosts(mut self, hosts: &[&str]) -> Self {
		self.blob_hosts = hosts.iter().map(|h| h.to_ascii_lowercase()).collect();
		self
	}

	/// Whether `url`'s host is one a blob may come from.
	pub(crate) fn blob_host_allowed(&self, url: &BlobUrl) -> bool {
		host_allowed(&self.blob_hosts, &url.host())
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
		// `rustls` reports a refused certificate as invalid data wrapped in an I/O error.
		ureq::Error::Io(e)
			if e.kind() == std::io::ErrorKind::InvalidData && e.get_ref().is_some() =>
		{
			"TLS failed"
		}
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
fn common<B>(b: ureq::RequestBuilder<B>, auth: &str, kind: HostKind) -> ureq::RequestBuilder<B> {
	let b = b.header("Authorization", auth);
	match kind {
		HostKind::GitHub => b
			.header("Accept", "application/vnd.github+json")
			.header("X-GitHub-Api-Version", "2022-11-28"),
		HostKind::Forgejo => b.header("Accept", "application/json"),
	}
}

impl Forge for UreqForge {
	fn send(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		let url = format!("{}{}", self.base, req.target());
		let auth = format!("Bearer {}", self.token.expose());
		let body = req.body.as_deref();
		let result = match req.method {
			Method::Get => common(self.api.get(&url), &auth, self.kind).call(),
			Method::Delete => common(self.api.delete(&url), &auth, self.kind).call(),
			Method::Post => common(self.api.post(&url), &auth, self.kind)
				.content_type("application/json")
				.send(body.unwrap_or(b"")),
			Method::Patch => common(self.api.patch(&url), &auth, self.kind)
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
			total_count: number(&resp, "x-total-count"),
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
			if !self.blob_host_allowed(&current) {
				return Err(ForgeError::Refused("the blob host is not one GitHub uses"));
			}
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

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::panic)]

	use super::*;

	#[test]
	fn a_ca_file_replaces_the_webpki_roots_for_the_api_and_for_blobs() {
		let token = || Token::new("fjt_SECRET").unwrap();
		let plain = UreqForge::forgejo("https://git.example/api/v1", token()).unwrap();
		assert!(matches!(
			plain.api.config().tls_config().root_certs(),
			RootCerts::WebPki
		));

		let ca = CaBundle::from_pem(include_bytes!("../tests/fixtures/tls/test-ca.pem")).unwrap();
		let private = plain.with_ca(&ca);
		for agent in [&private.api, &private.blob] {
			match agent.config().tls_config().root_certs() {
				RootCerts::Specific(certs) => assert_eq!(certs.len(), 1),
				other => panic!("the roots are {other:?}, not the CA file's"),
			}
		}
	}
}
