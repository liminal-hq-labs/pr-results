// The host-neutral client: spacing between writes, retries, the retry budget and paging, over any `Forge`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Every call a host makes goes through [`Client::request`], which spaces writes, follows `Link`
//! headers for paging, and retries what is safe to retry: a read or an idempotent write on a server
//! error or a dropped connection, and any call the forge refused before acting on it (429 and the
//! rate limits). A POST that may have been applied is never repeated blindly: the caller looks for
//! what it would have created first. A [`Client`] knows nothing of any one forge's routes; the
//! [`Host`](crate::Host) implementations build those.

use std::cell::Cell;
use std::time::Duration;

use pr_results_core::RepoSlug;
use serde_json::Value;

use crate::api::{ApiPath, ApiRequest, ApiResponse, Forge, ForgeError, Method};

/// The default total time one publish may spend waiting to retry.
pub const RETRY_BUDGET_S: u64 = 600;

/// Writes are at least this far apart, which stays well inside the secondary rate limit.
const WRITE_GAP: Duration = Duration::from_millis(500);
const MAX_PAGES: usize = 100;
/// The pause before each retry of a server error.
pub(crate) const BACKOFF_S: [u64; 4] = [1, 2, 4, 8];
/// The longest wait a rate limit may ask for.
const MAX_WAIT_S: u64 = 60;

/// How a host's lists are paged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paging {
	/// GitHub: `per_page=100`, then the `Link` header's next page.
	GitHub,
	/// Forgejo: `limit=50`, then the `Link` header's next page.
	Forgejo,
	/// A list returned whole in one response.
	Whole,
}

impl Paging {
	/// The query parameter that sets the page size, if the list is paged by size.
	fn page_size(self) -> Option<(&'static str, &'static str)> {
		match self {
			Self::GitHub => Some(("per_page", "100")),
			Self::Forgejo => Some(("limit", "50")),
			Self::Whole => None,
		}
	}
}

pub struct Client<F: Forge> {
	forge: F,
	repo: RepoSlug,
	writes: Cell<u64>,
	/// When this run of the publisher began, by the forge's clock.
	started: Cell<Option<u64>>,
	retry_budget_s: u64,
	budget_spent: Cell<bool>,
}

impl<F: Forge> Client<F> {
	pub fn new(forge: F, repo: RepoSlug) -> Self {
		Self {
			forge,
			repo,
			writes: Cell::new(0),
			started: Cell::new(None),
			retry_budget_s: RETRY_BUDGET_S,
			budget_spent: Cell::new(false),
		}
	}

	/// How many seconds of waiting, in total, one publish may spend on retries before it stops
	/// retrying and reports what it has. The default is [`RETRY_BUDGET_S`].
	pub fn with_retry_budget(mut self, seconds: u64) -> Self {
		self.retry_budget_s = seconds;
		self
	}

	/// Starts the clock of the retry budget. `publish` calls it first.
	pub fn begin_run(&self) {
		self.started.set(Some(self.forge.now_epoch()));
		self.budget_spent.set(false);
	}

	/// Whether a retry was refused because the budget for waiting was used up.
	pub fn retry_budget_spent(&self) -> bool {
		self.budget_spent.get()
	}

	/// Whether waiting `wait` more seconds stays inside the budget; if not, says so for the report.
	pub(crate) fn may_wait(&self, wait: u64) -> bool {
		let now = self.forge.now_epoch();
		let started = self.started.get().unwrap_or_else(|| {
			self.started.set(Some(now));
			now
		});
		let ok = now.saturating_sub(started).saturating_add(wait) <= self.retry_budget_s;
		if !ok {
			self.budget_spent.set(true);
		}
		ok
	}

	pub fn forge(&self) -> &F {
		&self.forge
	}

	pub fn repo(&self) -> &RepoSlug {
		&self.repo
	}

	pub(crate) fn path(&self, tail: &str) -> Result<ApiPath, ForgeError> {
		ApiPath::parse(&format!("/repos/{}/{tail}", self.repo))
			.ok_or(ForgeError::Decode("a request path could not be built"))
	}

	pub(crate) fn get(&self, tail: &str) -> Result<ApiRequest, ForgeError> {
		Ok(ApiRequest::get(self.path(tail)?))
	}

	pub(crate) fn json_request(
		&self,
		method: Method,
		tail: &str,
		body: &Value,
	) -> Result<ApiRequest, ForgeError> {
		Ok(ApiRequest::json(method, self.path(tail)?, body))
	}

	/// Sends `req` with spacing between writes and the retries described above. A response with a
	/// failing status that is not worth retrying comes back as `Ok`, for the caller to judge.
	pub(crate) fn request(&self, req: &ApiRequest) -> Result<ApiResponse, ForgeError> {
		self.request_with(req, req.method.is_idempotent())
	}

	/// The same, for a call that is `idempotent` or not whatever its verb: a PATCH that appends
	/// annotations is not, and is not repeated after a failure that may have applied it.
	pub(crate) fn request_with(
		&self,
		req: &ApiRequest,
		idempotent: bool,
	) -> Result<ApiResponse, ForgeError> {
		if req.method.is_write() {
			if self.writes.get() > 0 {
				self.forge.sleep(WRITE_GAP);
			}
			self.writes.set(self.writes.get().saturating_add(1));
		}
		let mut attempt = 0usize;
		loop {
			let resp = match self.forge.send(req) {
				Ok(r) => r,
				Err(e @ ForgeError::Transport(_)) if idempotent => {
					let Some(s) = BACKOFF_S.get(attempt) else {
						return Err(e);
					};
					if !self.may_wait(*s) {
						return Err(e);
					}
					self.forge.sleep(Duration::from_secs(*s));
					attempt += 1;
					continue;
				}
				Err(e) => return Err(e),
			};
			let wait = match self.wait_for(&resp, idempotent, attempt)? {
				Some(w) => w,
				None => return Ok(resp),
			};
			if attempt >= BACKOFF_S.len() || !self.may_wait(wait) {
				return Ok(resp);
			}
			self.forge.sleep(Duration::from_secs(wait));
			attempt += 1;
		}
	}

	/// How long to wait before repeating `req`, or `None` when the response is final. A rate limit
	/// that will not lift within a minute is an error.
	fn wait_for(
		&self,
		resp: &ApiResponse,
		idempotent: bool,
		attempt: usize,
	) -> Result<Option<u64>, ForgeError> {
		let backoff = BACKOFF_S.get(attempt).copied().unwrap_or(MAX_WAIT_S);
		let limited = matches!(resp.status, 403 | 429);
		if limited && resp.rate_remaining == Some(0) {
			// The primary limit: the request was not acted on, and the reset time is exact.
			let wait = resp
				.rate_reset
				.unwrap_or(0)
				.saturating_sub(self.forge.now_epoch());
			if wait > MAX_WAIT_S {
				return Err(ForgeError::RateLimited { wait_s: wait });
			}
			return Ok(Some(wait.max(1)));
		}
		let secondary = resp.status == 429
			|| (resp.status == 403
				&& (resp.retry_after_s.is_some()
					|| String::from_utf8_lossy(&resp.body)
						.to_ascii_lowercase()
						.contains("rate limit")));
		if secondary {
			return match resp.retry_after_s {
				Some(s) if s > MAX_WAIT_S => Err(ForgeError::RateLimited { wait_s: s }),
				Some(s) => Ok(Some(s.max(1))),
				None => Ok(Some(backoff)),
			};
		}
		if resp.status >= 500 && idempotent {
			return match resp.retry_after_s {
				Some(s) if s > MAX_WAIT_S => Err(ForgeError::RateLimited { wait_s: s }),
				Some(s) => Ok(Some(s.max(backoff))),
				None => Ok(Some(backoff)),
			};
		}
		Ok(None)
	}

	pub(crate) fn ok(resp: ApiResponse, op: &'static str) -> Result<ApiResponse, ForgeError> {
		if resp.ok() {
			Ok(resp)
		} else {
			Err(ForgeError::Status {
				status: resp.status,
				op,
			})
		}
	}

	/// Every item of a list, following `Link` headers for at most 100 pages (not at all for
	/// [`Paging::Whole`]). `key` names the array when the list is wrapped, as the artefacts are.
	/// Each item is reduced by `keep` as its page is read and the page is dropped, so what is held
	/// is what the caller asked to keep and not a hundred pages of other people's text.
	///
	/// A forge that reports how many items a list holds (Forgejo's `X-Total-Count`) is believed
	/// over the pages: a list that comes back shorter than its count, because a `Link` was
	/// dropped or a page was cut, is an error and not a quietly short list. For a list returned
	/// whole the count must match exactly.
	pub(crate) fn paged<T>(
		&self,
		first: ApiRequest,
		paging: Paging,
		key: Option<&str>,
		op: &'static str,
		keep: impl Fn(&Value) -> Option<T>,
	) -> Result<Vec<T>, ForgeError> {
		let mut items = Vec::new();
		let mut seen = 0u64;
		let mut req = match paging.page_size() {
			Some((name, size)) => first.with_query(name, size),
			None => first,
		};
		for _ in 0..MAX_PAGES {
			let resp = Self::ok(self.request(&req)?, op)?;
			let next = resp.next.clone();
			let total = resp.total_count;
			let body = resp.json()?;
			drop(resp);
			let page = match key {
				Some(k) => body.get(k),
				None => Some(&body),
			};
			let page = page
				.and_then(Value::as_array)
				.ok_or(ForgeError::Decode("a list was expected"))?;
			seen = seen.saturating_add(page.len() as u64);
			items.extend(page.iter().filter_map(&keep));
			match (paging, next) {
				(Paging::Whole, _) | (_, None) => {
					return match total {
						Some(t) if paging == Paging::Whole && seen != t => Err(ForgeError::Decode(
							"a list does not match its X-Total-Count",
						)),
						Some(t) if seen < t => Err(ForgeError::Decode(
							"a list is shorter than its X-Total-Count",
						)),
						_ => Ok(items),
					};
				}
				(_, Some(next)) => req = ApiRequest::get(next),
			}
		}
		Err(ForgeError::Decode("a list has more than 100 pages"))
	}
}
