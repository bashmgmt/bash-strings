//! Quoted-nest (QN) codec.
//!
//! Each `Arr` child occupies exactly one bash word in its parent's array.
//! For nested `Arr`s, the inner array's *complete bash literal*
//! (`('a' 'b')` form) becomes the parent's single word — recursive
//! single-word quoting per nesting level.
//!
//! This mirrors `BashRaw::pack_as_string` minus the outer scalar
//! quoting layer: at each level, the inner `BashRaw::Array` emits its
//! `to_bash_literal()`, and the parent's `to_bash_literal()` adds the
//! per-element scalar quoting that makes it one bash word.
//!
//! Receiver side: each outer word is a complete bash literal, so
//! `BashRaw::parse_bash_literal_array` recovers the inner array. In
//! bash, the equivalent recovery is `declare -a inner="${outer[i]}"`
//! — no `eval` needed.

use super::{BashCodec, EmitError, CodecParseError};
use super::super::raw::BashRaw;
use super::super::value::{BashVal, Schema};

pub struct QuotedNest;

impl BashCodec for QuotedNest {
    fn emit(&self, val: &BashVal, schema: &Schema) -> Result<BashRaw, EmitError> {
        match (val, schema) {
            (BashVal::Str(s), Schema::Scalar) => Ok(BashRaw::String(s.clone())),
            (BashVal::Arr(elems), Schema::Arr(inner)) => {
                let words: Vec<String> = elems.iter()
                    .map(|e| emit_word(e, inner))
                    .collect::<Result<_, _>>()?;
                Ok(BashRaw::Array(words))
            }
            (BashVal::Str(_), Schema::Arr(_)) =>
                Err(EmitError::SchemaMismatch { expected: "Arr", got: "Str" }),
            (BashVal::Arr(_), Schema::Scalar) =>
                Err(EmitError::SchemaMismatch { expected: "Scalar", got: "Arr" }),
        }
    }

    fn parse(&self, raw: &BashRaw, schema: &Schema) -> Result<BashVal, CodecParseError> {
        match (raw, schema) {
            (BashRaw::String(s), Schema::Scalar) => Ok(BashVal::Str(s.clone())),
            (BashRaw::Array(words), Schema::Arr(inner)) => {
                let elems: Vec<BashVal> = words.iter()
                    .map(|w| parse_word(w, inner))
                    .collect::<Result<_, _>>()?;
                Ok(BashVal::Arr(elems))
            }
            (BashRaw::String(_), Schema::Arr(_)) =>
                Err(CodecParseError::NotArray { got: "String" }),
            (BashRaw::Array(_), Schema::Scalar) =>
                Err(CodecParseError::ExpectedScalar),
            (BashRaw::AssocArray(_), _) =>
                Err(CodecParseError::NotArray { got: "AssocArray" }),
        }
    }
}

/// One outer-array word: for `Scalar` it's the string itself; for `Arr`
/// it's the inner's full bash literal (`('a' 'b')`-form).
fn emit_word(val: &BashVal, schema: &Schema) -> Result<String, EmitError> {
    match (val, schema) {
        (BashVal::Str(s), Schema::Scalar) => Ok(s.clone()),
        (BashVal::Arr(_), Schema::Arr(_)) => {
            let inner = QuotedNest.emit(val, schema)?;
            Ok(inner.to_bash_literal())
        }
        (BashVal::Str(_), Schema::Arr(_)) =>
            Err(EmitError::SchemaMismatch { expected: "Arr", got: "Str" }),
        (BashVal::Arr(_), Schema::Scalar) =>
            Err(EmitError::SchemaMismatch { expected: "Scalar", got: "Arr" }),
    }
}

/// Inverse of `emit_word`: parse one outer-array word against its child
/// schema. Scalar passes through; Arr expects the bash literal `(...)`
/// form and recurses via `BashRaw::parse_bash_literal_array`.
fn parse_word(word: &str, schema: &Schema) -> Result<BashVal, CodecParseError> {
    match schema {
        Schema::Scalar => Ok(BashVal::Str(word.to_string())),
        Schema::Arr(_) => {
            let inner = BashRaw::parse_bash_literal_array(word)?;
            QuotedNest.parse(&inner, schema)
        }
    }
}
