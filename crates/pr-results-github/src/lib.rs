// The pr-results GitHub side: the artefact reader and, behind the `Forge` trait, the API client.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Nothing outside this crate talks to the network, and nothing in it trusts what it reads.

#![forbid(unsafe_code)]

mod archive;

pub use archive::{read_zip, ArchiveError, ArchiveLimits, Entry};
