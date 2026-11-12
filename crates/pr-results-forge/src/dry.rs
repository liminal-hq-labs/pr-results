// A forge that reads for real and writes nothing, keeping the writes it would have made.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::api::{ApiRequest, ApiResponse, BlobUrl, Forge, ForgeError, Method};

/// Wraps a forge: reads go through, writes are recorded and answered as if they had worked, with
/// ids counted from 1 so that they are not mistaken for GitHub's.
#[derive(Clone)]
pub struct DryRun<F: Forge> {
	inner: F,
	log: Rc<RefCell<Vec<ApiRequest>>>,
}

impl<F: Forge> DryRun<F> {
	pub fn new(inner: F) -> Self {
		Self {
			inner,
			log: Rc::new(RefCell::new(Vec::new())),
		}
	}

	/// The writes that were not sent, in order.
	pub fn requests(&self) -> Vec<ApiRequest> {
		self.log.borrow().clone()
	}

	/// The recorded writes as JSON, for a person to read: the method, the path with its query,
	/// and the body as JSON where it is JSON. No header is recorded, because none is part of a
	/// request here.
	pub fn requests_json(&self) -> String {
		requests_to_json(&self.log.borrow())
	}
}

/// The request list as pretty JSON, for the dry-run output and the golden request files.
pub fn requests_to_json(requests: &[ApiRequest]) -> String {
	let items: Vec<Value> = requests
		.iter()
		.map(|r| {
			let body = r
				.body
				.as_deref()
				.map(|b| serde_json::from_slice(b).unwrap_or_else(|_| Value::from("(not JSON)")));
			json!({
				"method": r.method.as_str(),
				"target": r.target(),
				"body": body,
			})
		})
		.collect();
	let mut out = serde_json::to_string_pretty(&items).unwrap_or_default();
	out.push('\n');
	out
}

impl<F: Forge> Forge for DryRun<F> {
	fn send(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		if !req.method.is_write() {
			return self.inner.send(req);
		}
		let mut log = self.log.borrow_mut();
		log.push(req.clone());
		let (status, body) = match req.method {
			Method::Post => (201, json!({ "id": log.len() })),
			Method::Patch => (200, json!({ "id": log.len() })),
			_ => (204, Value::Null),
		};
		Ok(ApiResponse {
			status,
			body: if status == 204 {
				Vec::new()
			} else {
				body.to_string().into_bytes()
			},
			..ApiResponse::default()
		})
	}

	fn fetch_blob(&self, url: &BlobUrl, max_bytes: u64) -> Result<Vec<u8>, ForgeError> {
		self.inner.fetch_blob(url, max_bytes)
	}

	fn sleep(&self, d: Duration) {
		self.inner.sleep(d);
	}

	fn now_epoch(&self) -> u64 {
		self.inner.now_epoch()
	}
}
