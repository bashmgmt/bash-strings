use super::{BashCodec, EmitError, CodecParseError};
use super::super::tree::{BashVal, Schema};

pub struct QuotedNest;

impl BashCodec for QuotedNest {
    fn emit(&self, val: &BashVal, schema: &Schema) -> Result<Vec<String>, EmitError> {
        match (val, schema) {
            (BashVal::Str(s), Schema::Scalar) => Ok(vec![s.clone()]),
            (BashVal::Arr(elems), Schema::Arr(inner)) => {
                elems.iter().map(|e| emit_one(e, inner)).collect()
            }
            (BashVal::Str(_), Schema::Arr(_)) =>
                Err(EmitError::SchemaMismatch { expected: "Arr", got: "Str" }),
            (BashVal::Arr(_), Schema::Scalar) =>
                Err(EmitError::SchemaMismatch { expected: "Scalar", got: "Arr" }),
        }
    }

    fn parse(&self, words: &[String], schema: &Schema) -> Result<BashVal, CodecParseError> {
        match schema {
            Schema::Scalar => {
                if words.len() != 1 { return Err(CodecParseError::ExpectedScalar); }
                Ok(BashVal::Str(words[0].clone()))
            }
            Schema::Arr(inner) => {
                let parsed: Vec<BashVal> = words.iter()
                    .map(|w| parse_one(w, inner))
                    .collect::<Result<_, _>>()?;
                Ok(BashVal::Arr(parsed))
            }
        }
    }
}

fn emit_one(val: &BashVal, schema: &Schema) -> Result<String, EmitError> {
    match (val, schema) {
        (BashVal::Str(s), Schema::Scalar) => Ok(s.clone()),
        (BashVal::Arr(_), Schema::Arr(_)) => QuotedNest.emit_literal(val, schema),
        (BashVal::Str(_), Schema::Arr(_)) =>
            Err(EmitError::SchemaMismatch { expected: "Arr", got: "Str" }),
        (BashVal::Arr(_), Schema::Scalar) =>
            Err(EmitError::SchemaMismatch { expected: "Scalar", got: "Arr" }),
    }
}

fn parse_one(word: &str, schema: &Schema) -> Result<BashVal, CodecParseError> {
    match schema {
        Schema::Scalar => Ok(BashVal::Str(word.to_string())),
        Schema::Arr(_) => QuotedNest.parse_literal(word, schema),
    }
}
