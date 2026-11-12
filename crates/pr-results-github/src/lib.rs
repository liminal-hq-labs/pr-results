// The pr-results GitHub side: the artefact reader and, behind the `Forge` trait, the API client.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Nothing outside this crate talks to the network, and nothing in it trusts what it reads.
//! [`Forge`] is the seam: [`UreqForge`] is the real client, [`FakeForge`] an in-memory GitHub for
//! tests, and [`DryRun`] reads for real and keeps the writes it would have made. [`GitHub`] is the
//! set of operations built on any of them, and [`publish`] is the whole flow.

#![forbid(unsafe_code)]

mod api;
mod archive;
mod dry;
mod fake;
mod github;
mod publish;

pub use api::{
	percent_decode, percent_encode, ApiPath, ApiRequest, ApiResponse, BlobUrl, Forge, ForgeError,
	Method,
};
pub use archive::{read_zip, ArchiveError, ArchiveLimits, Entry};
pub use dry::{requests_to_json, DryRun};
pub use fake::{FakeCheck, FakeComment, FakeForge, FakePull, Fault};
pub use github::{
	Artifact, CheckResult, CheckSpec, Comment, CommentAction, CommentResult, CommentSpec, GitHub,
	Pull, Tree, DEFAULT_BOT_LOGIN,
};
pub use publish::{
	publish, CheckReport, CommentReport, Failure, Note, Plan, PublishError, PublishReport,
	SectionPlan, WorkflowRun,
};
