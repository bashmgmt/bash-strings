//! Codecs — flatten `BashVal` trees into bash word streams.
//!
//! Bash arrays are flat: nesting is encoded textually. Two strategies:
//!
//! - [`QuotedNest`] — each inner array is one bash-literal word at the outer
//!   level: `[[a,b],[c]] → ["('a' 'b')", "('c')"]`. Receiver unquotes one
//!   layer per level.
//!
//! - [`LinkedArr`] — flat word stream prefixed by group lengths:
//!   `[[a,b],[c]] → [2, a, b, 1, c]`. Matches glue-core's bash-side walker.
//!
//! Both are guided by a [`Schema`] tree mirroring `BashVal` depth. Scalar
//! leaves are RAW strings; the consumer applies bash quoting via
//! [`emit_q_words`] when constructing an assignment.

use std::fmt;

use super::tree::{BashVal, Schema};
use super::emit::emit_q_words;
use super::parser::{parse_q_words, ParseError};

pub mod quoted_nest;
pub mod linked_arr;

pub use quoted_nest::QuotedNest;
pub use linked_arr::LinkedArr;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum EmitError {
    SchemaMismatch { expected: &'static str, got: &'static str },
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch { expected, got } =>
                write!(f, "BashVal does not match Schema: expected {expected}, got {got}"),
        }
    }
}

impl std::error::Error for EmitError {}

#[derive(Debug, Clone, PartialEq)]
pub enum CodecParseError {
    LayoutError(String),
    ExpectedScalar,
    Word(ParseError),
}

impl fmt::Display for CodecParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LayoutError(s) => write!(f, "layout: {s}"),
            Self::ExpectedScalar => write!(f, "schema requires Scalar; got multiple words"),
            Self::Word(e)        => write!(f, "word: {e}"),
        }
    }
}

impl std::error::Error for CodecParseError {}

impl From<ParseError> for CodecParseError {
    fn from(e: ParseError) -> Self { Self::Word(e) }
}

pub trait BashCodec {
    fn emit(&self, val: &BashVal, schema: &Schema) -> Result<Vec<String>, EmitError>;
    fn parse(&self, words: &[String], schema: &Schema) -> Result<BashVal, CodecParseError>;

    /// Emit as a complete bash array literal: `(w1 w2 ...)`. Each scalar
    /// is single-quoted via `emit_scalar`.
    fn emit_literal(&self, val: &BashVal, schema: &Schema) -> Result<String, EmitError> {
        let words = self.emit(val, schema)?;
        Ok(format!("({})", emit_q_words(&words)))
    }

    /// Parse a complete bash array literal: `(w1 w2 ...)`. Strips parens
    /// then dispatches to `parse`.
    fn parse_literal(&self, input: &str, schema: &Schema) -> Result<BashVal, CodecParseError> {
        let trimmed = input.trim();
        let inner = trimmed.strip_prefix('(').and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| CodecParseError::LayoutError(
                format!("expected (...) array literal: {trimmed:?}")))?;
        let words = parse_q_words(inner)?;
        self.parse(&words, schema)
    }
}
