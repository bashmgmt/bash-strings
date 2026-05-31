//! Tier 2 — what bash natively stores: a scalar string, an indexed array of
//! strings, or an associative array of strings. No nesting; bash itself can't.
//!
//! `BashRaw` is the canonical Rust mirror of bash's three native variable
//! shapes. It carries:
//!  - bidirectional bash-literal-string conversion (`to_bash_literal` /
//!    `parse_bash_literal`),
//!  - pack/unpack-as-single-word (so any raw shape can be stuffed into one
//!    bash word and unstuffed later),
//!  - builder methods (`array`, `string`, `assoc`, `arg`, `args`, `put`),
//!  - `From` for the underlying Rust types and `try_into_*` for extraction.

use std::fmt;
use indexmap::IndexMap;

use super::primitives::{ParseError, encode_scalar, parse_one_word, parse_words};

#[derive(Debug, Clone, PartialEq)]
pub enum BashRaw {
    String(String),
    Array(Vec<String>),
    AssocArray(IndexMap<String, String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConversionError {
    WrongVariant { expected: &'static str, got: &'static str },
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongVariant { expected, got } =>
                write!(f, "BashRaw conversion: expected variant {expected}, got {got}"),
        }
    }
}

impl std::error::Error for ConversionError {}

// ── Builders ─────────────────────────────────────────────

impl BashRaw {
    pub fn string(s: impl Into<String>) -> Self { Self::String(s.into()) }
    pub fn array() -> Self { Self::Array(Vec::new()) }
    pub fn assoc() -> Self { Self::AssocArray(IndexMap::new()) }

    /// Append a single argument to an `Array` (panics if not an Array — use
    /// `BashRaw::array()` first).
    pub fn arg(mut self, s: impl Into<String>) -> Self {
        match &mut self {
            Self::Array(v) => v.push(s.into()),
            _ => panic!("BashRaw::arg called on {}", self.variant_name()),
        }
        self
    }

    pub fn args<I, S>(mut self, iter: I) -> Self
        where I: IntoIterator<Item = S>, S: Into<String>
    {
        match &mut self {
            Self::Array(v) => v.extend(iter.into_iter().map(Into::into)),
            _ => panic!("BashRaw::args called on {}", self.variant_name()),
        }
        self
    }

    pub fn put(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        match &mut self {
            Self::AssocArray(m) => { m.insert(k.into(), v.into()); }
            _ => panic!("BashRaw::put called on {}", self.variant_name()),
        }
        self
    }

    fn variant_name(&self) -> &'static str {
        match self {
            Self::String(_) => "String",
            Self::Array(_) => "Array",
            Self::AssocArray(_) => "AssocArray",
        }
    }
}

// ── Bash-literal codec (the bash-side syntax) ────────────

impl BashRaw {
    /// Emit as a bash literal expression suitable for `eval` or assignment:
    ///   String → `'foo'`
    ///   Array → `('w1' 'w2')`
    ///   AssocArray → `(['k']='v' ['k2']='v2')`
    pub fn to_bash_literal(&self) -> String {
        match self {
            Self::String(s) => encode_scalar(s),
            Self::Array(v) => {
                let inner: Vec<String> = v.iter().map(|w| encode_scalar(w)).collect();
                format!("({})", inner.join(" "))
            }
            Self::AssocArray(m) => {
                let inner: Vec<String> = m.iter()
                    .map(|(k, v)| format!("[{}]={}", encode_scalar(k), encode_scalar(v)))
                    .collect();
                format!("({})", inner.join(" "))
            }
        }
    }

    /// Pack any raw shape as a single bash word — the literal expression
    /// itself wrapped in one more layer of bash quoting.
    /// Inverse of `unpack_from_string`.
    pub fn pack_as_string(&self) -> String {
        encode_scalar(&self.to_bash_literal())
    }
}

// ── Reading bash output (transport-side) ─────────────────
//
// Inputs here come from `${var@Q}` / `${arr[*]@Q}` / per-line assoc
// dump in `__ctl_assoc`. They are *not* bash literal expressions in
// the `(...)` form — they're streams of @Q-encoded words.

impl BashRaw {
    /// Parse the @Q output for a scalar variable: one quoted word.
    pub fn parse_string_at_q(input: &str) -> Result<Self, ParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(Self::String(String::new()));
        }
        let (word, rest) = parse_one_word(input)?;
        let rest = rest.trim();
        if !rest.is_empty() {
            return Err(ParseError::TrailingContent(rest.to_string()));
        }
        Ok(Self::String(word))
    }

    /// Parse the @Q output for an indexed array: space-separated quoted words.
    pub fn parse_array_at_q(input: &str) -> Result<Self, ParseError> {
        Ok(Self::Array(parse_words(input)?))
    }

    /// Parse the per-line assoc dump: each line is `key value`, both @Q-encoded.
    pub fn parse_assoc_at_q(input: &str) -> Result<Self, ParseError> {
        let mut map = IndexMap::new();
        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let (key, rest) = parse_one_word(line)?;
            let rest = rest.trim_start();
            if rest.is_empty() {
                return Err(ParseError::ExpectedPair(format!("got key {key:?} with no value")));
            }
            let (value, rest) = parse_one_word(rest)?;
            let rest = rest.trim();
            if !rest.is_empty() {
                return Err(ParseError::TrailingContent(rest.to_string()));
            }
            map.insert(key, value);
        }
        Ok(Self::AssocArray(map))
    }

    /// Parses `([k]='v' [k2]=v2 ...)` -- with and without
    pub fn parse_assoc_righthandside(input: &str) -> Result<Self, ParseError> {
        let pattern = regex::Regex::new(r"^\(.*\)$").unwrap();
        let inner = match pattern.captures(input.trim()) {
            Some(caps) => caps.get(0).unwrap().as_str(),
            None => return Err(ParseError::InvalidFormat(format!("expected associative array literal with parentheses, but got: {input:?}"))),
        };
        
    }

    /// Parse a bash literal array expression: `('w1' 'w2')` form.
    /// (Inverse of `to_bash_literal` for the `Array` variant.)
    pub fn parse_bash_literal_array(input: &str) -> Result<Self, ParseError> {
        let trimmed = input.trim();
        let inner = trimmed.strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| ParseError::InvalidFormat(
                format!("expected (...) array literal: {trimmed:?}")
            ))?;
        Self::parse_array_at_q(inner)
    }

    /// Inverse of `pack_as_string`: take a single quoted word and re-parse
    /// the unwrapped contents as a bash literal array expression.
    pub fn unpack_from_string(packed: &str) -> Result<Self, ParseError> {
        let inner = Self::parse_string_at_q(packed)?;
        let s = inner.into_string().expect("parse_string_at_q returns String");
        Self::parse_bash_literal_array(&s)
    }
}

// ── Conversions to/from underlying Rust types ────────────

impl From<String> for BashRaw {
    fn from(s: String) -> Self { Self::String(s) }
}

impl From<&str> for BashRaw {
    fn from(s: &str) -> Self { Self::String(s.to_string()) }
}

impl From<Vec<String>> for BashRaw {
    fn from(v: Vec<String>) -> Self { Self::Array(v) }
}

impl From<IndexMap<String, String>> for BashRaw {
    fn from(m: IndexMap<String, String>) -> Self { Self::AssocArray(m) }
}

impl BashRaw {
    pub fn into_string(self) -> Result<String, ConversionError> {
        match self {
            Self::String(s) => Ok(s),
            other => Err(ConversionError::WrongVariant {
                expected: "String", got: other.variant_name(),
            }),
        }
    }

    pub fn into_array(self) -> Result<Vec<String>, ConversionError> {
        match self {
            Self::Array(v) => Ok(v),
            other => Err(ConversionError::WrongVariant {
                expected: "Array", got: other.variant_name(),
            }),
        }
    }

    pub fn into_assoc(self) -> Result<IndexMap<String, String>, ConversionError> {
        match self {
            Self::AssocArray(m) => Ok(m),
            other => Err(ConversionError::WrongVariant {
                expected: "AssocArray", got: other.variant_name(),
            }),
        }
    }

    pub fn as_string(&self) -> Result<&str, ConversionError> {
        match self {
            Self::String(s) => Ok(s),
            other => Err(ConversionError::WrongVariant {
                expected: "String", got: other.variant_name(),
            }),
        }
    }

    pub fn as_array(&self) -> Result<&[String], ConversionError> {
        match self {
            Self::Array(v) => Ok(v),
            other => Err(ConversionError::WrongVariant {
                expected: "Array", got: other.variant_name(),
            }),
        }
    }

    pub fn as_assoc(&self) -> Result<&IndexMap<String, String>, ConversionError> {
        match self {
            Self::AssocArray(m) => Ok(m),
            other => Err(ConversionError::WrongVariant {
                expected: "AssocArray", got: other.variant_name(),
            }),
        }
    }
}
