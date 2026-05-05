//! Tier 3 ↔ Tier 2 codecs: flatten a recursive `BashVal` into a
//! `BashRaw::Array` (and back) according to a chosen convention.

use std::fmt;

use super::primitives::ParseError as PrimitiveParseError;
use super::raw::BashRaw;
use super::value::{BashVal, Schema};

pub mod quoted_nest;
pub mod linked_arr;

pub use quoted_nest::QuotedNest;
pub use linked_arr::LinkedArr;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum EmitError {
    /// BashVal shape doesn't match the supplied Schema.
    SchemaMismatch { expected: &'static str, got: &'static str },
    /// LinkedArr cannot encode N>2-deep schemas (no native nesting).
    /// Compose explicitly via `BashRaw::pack_as_string`.
    DepthExceeded,
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch { expected, got } =>
                write!(f, "BashVal does not match Schema: expected {expected}, got {got}"),
            Self::DepthExceeded =>
                write!(f, "LinkedArr cannot encode beyond depth 2"),
        }
    }
}

impl std::error::Error for EmitError {}

#[derive(Debug, Clone, PartialEq)]
pub enum CodecParseError {
    Primitive(PrimitiveParseError),
    /// Codec input must be a `BashRaw::Array`; got another variant.
    NotArray { got: &'static str },
    /// Element layout violates the codec invariants (e.g. LinkedArr length
    /// prefix not numeric, or run extends past array end).
    LayoutError(String),
    /// Schema requires Scalar but multiple words were present.
    ExpectedScalar,
}

impl fmt::Display for CodecParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(e) => write!(f, "{e}"),
            Self::NotArray { got } => write!(f, "codec expects BashRaw::Array; got {got}"),
            Self::LayoutError(s) => write!(f, "layout: {s}"),
            Self::ExpectedScalar => write!(f, "schema requires Scalar; got multiple words"),
        }
    }
}

impl std::error::Error for CodecParseError {}

impl From<PrimitiveParseError> for CodecParseError {
    fn from(e: PrimitiveParseError) -> Self { Self::Primitive(e) }
}

pub trait BashCodec {
    fn emit(&self, val: &BashVal, schema: &Schema) -> Result<BashRaw, EmitError>;
    fn parse(&self, raw: &BashRaw, schema: &Schema) -> Result<BashVal, CodecParseError>;
}
