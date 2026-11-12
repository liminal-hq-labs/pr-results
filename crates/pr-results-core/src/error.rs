// Errors from the parsers: every malformed or hostile input maps to one of these, never a panic.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt;

/// Why an input was rejected. The messages are plain English and contain no input text, so they
/// are safe to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
	/// The input has no content.
	Empty,
	/// The input is larger than the configured limit.
	TooLarge { len: usize, limit: usize },
	/// The input ends before the document is complete.
	Truncated,
	/// The input contains a document type declaration, which is never accepted.
	Dtd,
	/// The input declares an encoding other than UTF-8.
	UnsupportedEncoding,
	/// Elements are nested deeper than the limit.
	TooDeep { limit: usize },
	/// The input holds more test cases than the limit.
	TooManyCases { limit: usize },
	/// The input holds more suites than the limit.
	TooManySuites { limit: usize },
	/// The text kept from the input would use more memory than the limit.
	TooMuchText { limit: usize },
	/// An element has more attributes than the limit.
	TooManyAttributes { limit: usize },
	/// A named entity other than the five predefined ones, or an invalid character reference.
	BadEntity,
	/// The root element is not `testsuites` or `testsuite`.
	UnexpectedRoot,
	/// The elements are not arranged as JUnit requires (for example a test case outside a suite).
	Structure(&'static str),
	/// The XML is not well formed. The offset is a byte position in the input.
	Malformed { offset: u64 },
}

impl fmt::Display for ParseError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Empty => f.write_str("the input is empty"),
			Self::TooLarge { len, limit } => {
				write!(
					f,
					"the input is {len} bytes, over the limit of {limit} bytes"
				)
			}
			Self::Truncated => f.write_str("the input ends before the document is complete"),
			Self::Dtd => {
				f.write_str("the input has a document type declaration, which is not accepted")
			}
			Self::UnsupportedEncoding => {
				f.write_str("the input declares an encoding other than UTF-8")
			}
			Self::TooDeep { limit } => write!(f, "elements are nested deeper than {limit} levels"),
			Self::TooManyCases { limit } => write!(f, "the input has more than {limit} test cases"),
			Self::TooManySuites { limit } => write!(f, "the input has more than {limit} suites"),
			Self::TooMuchText { limit } => {
				write!(f, "the input holds more than {limit} bytes of text to keep")
			}
			Self::TooManyAttributes { limit } => {
				write!(f, "an element has more than {limit} attributes")
			}
			Self::BadEntity => {
				f.write_str("the input uses an entity or character reference that is not accepted")
			}
			Self::UnexpectedRoot => f.write_str("the root element is not testsuites or testsuite"),
			Self::Structure(what) => {
				write!(f, "the elements are not arranged as JUnit requires: {what}")
			}
			Self::Malformed { offset } => {
				write!(f, "the XML is not well formed near byte {offset}")
			}
		}
	}
}

impl std::error::Error for ParseError {}
