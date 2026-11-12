// The CA file: what `CaBundle` accepts, and that a client built with one trusts that CA and no other.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The TLS tests run a real handshake against a server on the loopback interface, with a test
//! certificate authority from `fixtures/tls/`. They prove three things: a client with a CA file
//! completes a handshake with a server that CA signed, a client without one (the webpki roots)
//! does not, and a client with a different CA does not; and in the two refusals the server never
//! sees a request, so the token never leaves the client.

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use pr_results_forge::{
	ApiPath, ApiRequest, CaBundle, CaError, Forge, ForgeError, Token, UreqForge,
};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

const SECRET: &str = "fjt_SECRETTOKENVALUE0123456789";

fn pem(name: &str) -> Vec<u8> {
	std::fs::read(
		Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/fixtures/tls")
			.join(name),
	)
	.unwrap()
}

/// A TLS server on 127.0.0.1 that answers `connections` connections with a JSON body, and keeps
/// the requests it read.
struct Tls {
	port: u16,
	seen: Arc<Mutex<Vec<String>>>,
	handle: JoinHandle<()>,
}

fn serve(connections: usize) -> Tls {
	let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(&pem("leaf.pem"))
		.collect::<Result<_, _>>()
		.unwrap();
	let key = PrivateKeyDer::from_pem_slice(&pem("leaf.key")).unwrap();
	let config = Arc::new(
		rustls::ServerConfig::builder_with_provider(Arc::new(
			rustls::crypto::ring::default_provider(),
		))
		.with_safe_default_protocol_versions()
		.unwrap()
		.with_no_client_auth()
		.with_single_cert(certs, key)
		.unwrap(),
	);
	let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
	let port = listener.local_addr().unwrap().port();
	let seen = Arc::new(Mutex::new(Vec::new()));
	let log = Arc::clone(&seen);
	let handle = std::thread::spawn(move || {
		for _ in 0..connections {
			let Ok((tcp, _)) = listener.accept() else {
				return;
			};
			tcp.set_read_timeout(Some(std::time::Duration::from_secs(5)))
				.unwrap();
			let Ok(conn) = rustls::ServerConnection::new(Arc::clone(&config)) else {
				continue;
			};
			let mut stream = rustls::StreamOwned::new(conn, tcp);
			let mut raw = Vec::new();
			let mut buf = [0u8; 2048];
			// A refused handshake ends here with an error and nothing read.
			while !raw.windows(4).any(|w| w == b"\r\n\r\n") {
				match stream.read(&mut buf) {
					Ok(0) | Err(_) => break,
					Ok(n) => raw.extend_from_slice(&buf[..n]),
				}
			}
			if raw.is_empty() {
				continue;
			}
			log.lock()
				.unwrap()
				.push(String::from_utf8_lossy(&raw).into_owned());
			let body = br#"{"id":7,"login":"ci-bot"}"#;
			let head = format!(
				"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\nX-Total-Count: 1\r\n\r\n",
				body.len()
			);
			let _ = stream.write_all(head.as_bytes());
			let _ = stream.write_all(body);
			let _ = stream.flush();
			stream.conn.send_close_notify();
			let _ = stream.flush();
		}
	});
	Tls { port, seen, handle }
}

impl Tls {
	fn base(&self) -> String {
		format!("https://127.0.0.1:{}/api/v1", self.port)
	}

	fn finish(self) -> Vec<String> {
		self.handle.join().unwrap();
		let seen = self.seen.lock().unwrap().clone();
		seen
	}
}

fn get_user(forge: &UreqForge) -> Result<pr_results_forge::ApiResponse, ForgeError> {
	forge.send(&ApiRequest::get(ApiPath::parse("/user").unwrap()))
}

fn client(base: &str) -> UreqForge {
	UreqForge::forgejo(base, Token::new(SECRET).unwrap()).unwrap()
}

#[test]
fn a_client_with_the_ca_file_trusts_a_server_that_ca_signed() {
	let server = serve(1);
	let ca = CaBundle::from_pem(&pem("test-ca.pem")).unwrap();
	let forge = client(&server.base()).with_ca(&ca);
	let resp = get_user(&forge).unwrap();
	assert_eq!(resp.status, 200);
	assert_eq!(resp.total_count, Some(1), "X-Total-Count is read");
	assert_eq!(resp.json().unwrap()["login"], "ci-bot");
	let seen = server.finish();
	assert_eq!(seen.len(), 1);
	let request = &seen[0];
	assert!(
		request.starts_with("GET /api/v1/user HTTP/1.1"),
		"{request}"
	);
	assert!(
		request.contains(&format!("authorization: Bearer {SECRET}"))
			|| request.contains(&format!("Authorization: Bearer {SECRET}"))
	);
	let lower = request.to_ascii_lowercase();
	assert!(lower.contains("accept: application/json"));
	assert!(
		!lower.contains("x-github-api-version"),
		"a GitHub header went to Forgejo"
	);
}

#[test]
fn a_client_without_a_ca_file_does_not_trust_a_private_ca() {
	let server = serve(1);
	let forge = client(&server.base());
	let err = get_user(&forge).unwrap_err();
	assert!(
		matches!(err, ForgeError::Transport(ref why) if why == "TLS failed"),
		"{err:?}"
	);
	assert!(!err.to_string().contains(SECRET));
	assert!(
		server.finish().is_empty(),
		"the server saw a request, and with it the token"
	);
}

#[test]
fn a_client_with_another_ca_trusts_only_that_one() {
	let server = serve(1);
	let ca = CaBundle::from_pem(&pem("other-ca.pem")).unwrap();
	let forge = client(&server.base()).with_ca(&ca);
	let err = get_user(&forge).unwrap_err();
	assert!(
		matches!(err, ForgeError::Transport(ref why) if why == "TLS failed"),
		"{err:?}"
	);
	assert!(server.finish().is_empty());
}

#[test]
fn plain_http_and_other_bases_are_refused_for_forgejo() {
	for base in [
		"http://127.0.0.1:3000/api/v1",
		"https://u:p@git.example/api/v1",
		"https://git.example/api/v1?x=1",
		"ftp://git.example/api/v1",
		"git.example/api/v1",
	] {
		assert!(
			UreqForge::forgejo(base, Token::new(SECRET).unwrap()).is_err(),
			"{base}"
		);
	}
	assert!(UreqForge::forgejo(
		"https://git.example/git/api/v1",
		Token::new(SECRET).unwrap()
	)
	.is_ok());
}

// ---- what a CA file may hold ------------------------------------------------------------------

#[test]
fn a_bundle_holds_every_certificate_of_the_file() {
	let one = CaBundle::from_pem(&pem("test-ca.pem")).unwrap();
	assert_eq!(one.len(), 1);
	let mut two = pem("test-ca.pem");
	two.extend_from_slice(b"\n# the second one\n");
	two.extend_from_slice(&pem("other-ca.pem"));
	assert_eq!(CaBundle::from_pem(&two).unwrap().len(), 2);
	assert!(format!("{one:?}").contains("certificates"));
}

#[test]
fn garbage_and_empty_files_are_refused() {
	assert_eq!(CaBundle::from_pem(b"").unwrap_err(), CaError::NoCertificate);
	assert_eq!(
		CaBundle::from_pem(b"this is not a certificate\n").unwrap_err(),
		CaError::NoCertificate
	);
	assert_eq!(
		CaBundle::from_pem(&[0xff, 0xfe, 0x00, 0x01, 0x80]).unwrap_err(),
		CaError::NoCertificate
	);
	// A DER file, not PEM.
	assert_eq!(
		CaBundle::from_pem(&[0x30, 0x03, 0x02, 0x01, 0x00]).unwrap_err(),
		CaError::NoCertificate
	);
}

#[test]
fn a_damaged_certificate_is_refused() {
	// Bad base64 inside the markers.
	let bad = b"-----BEGIN CERTIFICATE-----\n!!!! not base64 !!!!\n-----END CERTIFICATE-----\n";
	assert_eq!(CaBundle::from_pem(bad).unwrap_err(), CaError::Malformed);
	// Valid base64, but not a DER SEQUENCE ("hello world").
	let text = b"-----BEGIN CERTIFICATE-----\naGVsbG8gd29ybGQ=\n-----END CERTIFICATE-----\n";
	assert_eq!(CaBundle::from_pem(text).unwrap_err(), CaError::Malformed);
	// A real certificate with its tail cut off: the length no longer covers the bytes.
	let whole = String::from_utf8(pem("test-ca.pem")).unwrap();
	let lines: Vec<&str> = whole.lines().collect();
	let cut = [lines[0], lines[1], lines[2], lines[lines.len() - 1]].join("\n");
	assert_eq!(
		CaBundle::from_pem(cut.as_bytes()).unwrap_err(),
		CaError::Malformed
	);
}

#[test]
fn a_private_key_in_the_file_is_refused_even_beside_a_good_certificate() {
	let mut both = pem("test-ca.pem");
	both.extend_from_slice(&pem("leaf.key"));
	assert_eq!(
		CaBundle::from_pem(&both).unwrap_err(),
		CaError::NotOnlyCertificates
	);
	assert_eq!(
		CaBundle::from_pem(&pem("leaf.key")).unwrap_err(),
		CaError::NotOnlyCertificates
	);
}

#[test]
fn a_file_over_one_mebibyte_is_refused_before_it_is_read() {
	let mut big = pem("test-ca.pem");
	big.resize(2 * 1024 * 1024, b'\n');
	assert_eq!(CaBundle::from_pem(&big).unwrap_err(), CaError::TooLarge);
	let mut edge = pem("test-ca.pem");
	edge.resize(CaBundle::MAX_BYTES, b'\n');
	assert_eq!(CaBundle::from_pem(&edge).unwrap().len(), 1);
	edge.push(b'\n');
	assert_eq!(CaBundle::from_pem(&edge).unwrap_err(), CaError::TooLarge);
}

#[test]
fn more_than_thirty_two_certificates_are_refused() {
	let one = pem("test-ca.pem");
	let many = |n: usize| one.repeat(n);
	assert_eq!(CaBundle::from_pem(&many(32)).unwrap().len(), 32);
	assert_eq!(CaBundle::from_pem(&many(33)).unwrap_err(), CaError::TooMany);
}

#[test]
fn the_errors_say_what_is_wrong_and_quote_nothing() {
	for e in [
		CaError::TooLarge,
		CaError::NoCertificate,
		CaError::NotOnlyCertificates,
		CaError::Malformed,
		CaError::TooMany,
	] {
		assert!(!e.to_string().is_empty());
	}
}
