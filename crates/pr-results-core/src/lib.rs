// The pr-results core: the normalized model, the `Safe` escaping type and the JUnit parsers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Everything here is pure: no network, no clocks, no environment. Every string that comes from a
//! test report is held as [`Untrusted`] and reaches output only through [`Safe`].

#![forbid(unsafe_code)]

mod error;
mod flavour;
mod junit;
mod model;
mod safe;

pub use error::ParseError;
pub use junit::{parse_junit, Limits};
pub use model::{Attempt, Case, Flavour, Report, Status, Suite, Untrusted, REPORT_SCHEMA};
pub use safe::{Safe, Source, DEFAULT_MAX_CHARS};
