// The pr-results forge side: the transport, the hosts behind it, the artefact reader and the publish flow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Nothing outside this crate talks to the network, and nothing in it trusts what it reads.
//! [`Forge`] is the seam: [`UreqForge`] is the real client, [`FakeForge`] an in-memory GitHub for
//! tests, and [`DryRun`] reads for real and keeps the writes it would have made. [`Client`] adds
//! spacing, retries and paging to any of them, a [`Host`] is one forge's routes and shapes on a
//! client ([`GitHub`] today), and [`publish`] is the whole flow, generic over the host.

#![forbid(unsafe_code)]

mod api;
mod archive;
mod client;
mod dry;
mod fake;
mod github;
mod host;
mod publish;
mod ureq_forge;

#[cfg(test)]
mod http_tests;

pub use api::{
	percent_decode, percent_encode, ApiPath, ApiRequest, ApiResponse, BlobUrl, Forge, ForgeError,
	Method, Token,
};
pub use archive::{read_zip, read_zip_filtered, ArchiveError, ArchiveLimits, Entry};
pub use client::{Client, Paging, RETRY_BUDGET_S};
pub use dry::{requests_to_json, DryRun};
pub use fake::{FakeCheck, FakeComment, FakeForge, FakePull, Fault};
pub use github::{GitHub, DEFAULT_BOT_LOGIN, DEFAULT_CHECK_APP};
pub use host::{
	Artifact, CheckResult, CheckSpec, Comment, CommentAction, CommentResult, CommentSpec, Host,
	HostKind, Pull, ResultSpec, ResultWritten, Tree, BODY_PREFIX_BYTES,
};
pub use publish::{
	publish, CheckReport, CommentReport, Failure, Note, Plan, PublishError, PublishReport,
	SectionPlan, WorkflowRun,
};
pub use ureq_forge::{UreqForge, GITHUB_BLOB_HOSTS};
