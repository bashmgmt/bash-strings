//! LinkedArr (LA) codec.
//!
//! A 2D-or-deeper `Arr` is flattened into a single bash array using
//! length prefixes for sub-array elements. Per-level rule: when an `Arr`
//! at this level contains elements of type `Schema::Arr(_)`, each
//! element is prefixed by its width (recursively measured); when the
//! elements are `Schema::Scalar`, no prefix is needed (each scalar IS
//! one bash word).
//!
//! Examples (each prefix counts the FULL inner word stream including its
//! own nested prefixes):
//!   1D `Arr(Scalar)`        [a, b]         → `(a b)`
//!   2D `Arr(Arr(Scalar))`   [[a,b],[c,d,e]] → `(2 a b 3 c d e)`
//!   3D `Arr^3(Scalar)`      [[[a,b],[c]]]   → `(5 2 a b 1 c)`        (one outer; inner-2D = 5 words)
//!   3D `Arr^3(Scalar)`      [[[a,b]],[[c]]] → `(3 2 a b 2 1 c)`      (two outers; inner-2D widths 3 and 2)
//!
//! Matches `glue-core/src/data/linked_arr.bash::LinkedArr__Add` /
//! `LinkedArr__Call` semantics.

use super::{BashCodec, EmitError, CodecParseError};
use super::super::raw::BashRaw;
use super::super::value::{BashVal, Schema};

pub struct LinkedArr;

impl BashCodec for LinkedArr {
    fn emit(&self, val: &BashVal, schema: &Schema) -> Result<BashRaw, EmitError> {
        match (val, schema) {
            (BashVal::Str(s), Schema::Scalar) => Ok(BashRaw::String(s.clone())),
            (BashVal::Arr(_), Schema::Arr(_)) => {
                let words = emit_body(val, schema)?;
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
            (BashRaw::Array(words), Schema::Arr(_)) => {
                let (val, consumed) = parse_body(words, schema)?;
                if consumed != words.len() {
                    return Err(CodecParseError::LayoutError(format!(
                        "trailing words: consumed {consumed} of {}", words.len()
                    )));
                }
                Ok(val)
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

/// Emit the body of an `Arr` level (no enclosing length prefix). The caller
/// provides the prefix when nesting.
fn emit_body(val: &BashVal, schema: &Schema) -> Result<Vec<String>, EmitError> {
    match (val, schema) {
        (BashVal::Str(s), Schema::Scalar) => Ok(vec![s.clone()]),
        (BashVal::Arr(elems), Schema::Arr(inner)) => {
            let elem_is_arr = matches!(**inner, Schema::Arr(_));
            let mut out = Vec::new();
            for e in elems {
                let body = emit_body(e, inner)?;
                if elem_is_arr {
                    out.push(body.len().to_string());
                }
                out.extend(body);
            }
            Ok(out)
        }
        (BashVal::Str(_), Schema::Arr(_)) =>
            Err(EmitError::SchemaMismatch { expected: "Arr", got: "Str" }),
        (BashVal::Arr(_), Schema::Scalar) =>
            Err(EmitError::SchemaMismatch { expected: "Scalar", got: "Arr" }),
    }
}

/// Parse a slice of words as the body of an Arr at the given schema.
/// Returns `(parsed_val, consumed_word_count)`.
fn parse_body(words: &[String], schema: &Schema) -> Result<(BashVal, usize), CodecParseError> {
    match schema {
        Schema::Scalar => {
            if words.is_empty() {
                return Err(CodecParseError::LayoutError("scalar position; no word".into()));
            }
            Ok((BashVal::Str(words[0].clone()), 1))
        }
        Schema::Arr(inner) => {
            let elem_is_arr = matches!(**inner, Schema::Arr(_));
            let mut elems = Vec::new();
            let mut pos = 0;
            while pos < words.len() {
                if elem_is_arr {
                    let len: usize = words[pos].parse().map_err(|_| {
                        CodecParseError::LayoutError(format!(
                            "length prefix not numeric at pos {pos}: {:?}", words[pos]
                        ))
                    })?;
                    pos += 1;
                    let body_end = pos + len;
                    if body_end > words.len() {
                        return Err(CodecParseError::LayoutError(format!(
                            "group claims {len} words; only {} available",
                            words.len() - pos
                        )));
                    }
                    let body = &words[pos..body_end];
                    let (v, consumed) = parse_body(body, inner)?;
                    if consumed != body.len() {
                        return Err(CodecParseError::LayoutError(format!(
                            "nested group: consumed {consumed} of {} body words",
                            body.len()
                        )));
                    }
                    elems.push(v);
                    pos = body_end;
                } else {
                    elems.push(BashVal::Str(words[pos].clone()));
                    pos += 1;
                }
            }
            Ok((BashVal::Arr(elems), pos))
        }
    }
}
