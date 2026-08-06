//! Recursive values, and the two ways to flatten one into bash words.
//!
//! `BashVal` is a tree of strings and `Schema` its depth. Bash arrays are
//! flat, so nesting is encoded textually:
//!
//! - [`QuotedNest`] — each inner array is one bash-literal word at the outer
//!   level: `[[a,b],[c]] → ["('a' 'b')", "('c')"]`. The receiver unquotes one
//!   layer per level.
//!
//! - [`LinkedArr`] — one flat word stream, each group prefixed by its width:
//!   `[[a,b],[c]] → [2, a, b, 1, c]`. The shape `glue-core`'s bash-side
//!   walker reads.
//!
//! Emitting takes the depth from the value; parsing takes it from a `Schema`,
//! which is what the text alone does not say. Scalar leaves are raw strings —
//! bash quoting is applied by [`emit_q_words`] when an assignment is built.

use std::fmt;

use super::emit::emit_q_words;
use super::parser::{parse_q_words, ParseError};

#[derive(Debug, Clone, PartialEq)]
pub enum BashVal {
    Str(String),
    Arr(Vec<BashVal>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Schema {
    Scalar,
    Arr(Box<Schema>),
}

impl Schema {
    pub fn n_d(n: usize) -> Self {
        let mut schema = Schema::Scalar;
        for _ in 0..n {
            schema = Schema::Arr(Box::new(schema));
        }
        schema
    }
}

impl BashVal {
    /// One flat row of words.
    pub fn row(words: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::Arr(words.into_iter().map(|word| Self::Str(word.into())).collect())
    }

    /// The words of a one-dimensional value; `None` if it is any other shape.
    pub fn words(self) -> Option<Vec<String>> {
        let Self::Arr(items) = self else { return None };

        items
            .into_iter()
            .map(|item| match item {
                Self::Str(word) => Some(word),
                Self::Arr(_) => None,
            })
            .collect()
    }

    /// The rows of a two-dimensional value; `None` if it is any other shape.
    pub fn rows(self) -> Option<Vec<Vec<String>>> {
        let Self::Arr(rows) = self else { return None };

        rows.into_iter().map(Self::words).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CodecParseError {
    LayoutError(String),
    ExpectedScalar,
    Word(ParseError),
}

impl fmt::Display for CodecParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LayoutError(what) => write!(f, "layout: {what}"),
            Self::ExpectedScalar => write!(f, "schema requires Scalar; got multiple words"),
            Self::Word(cause) => write!(f, "word: {cause}"),
        }
    }
}

impl std::error::Error for CodecParseError {}

impl From<ParseError> for CodecParseError {
    fn from(cause: ParseError) -> Self {
        Self::Word(cause)
    }
}

pub trait BashCodec {
    /// The value's own depth decides the encoding, so there is nothing to
    /// disagree with and nothing to fail.
    fn emit(&self, val: &BashVal) -> Vec<String>;

    /// A `Schema` says how many layers of quoting to peel, which the text
    /// alone does not.
    fn parse(&self, words: &[String], schema: &Schema) -> Result<BashVal, CodecParseError>;

    /// A complete bash array literal: `(w1 w2 …)`, each scalar single-quoted.
    fn emit_literal(&self, val: &BashVal) -> String {
        format!("({})", emit_q_words(&self.emit(val)))
    }

    fn parse_literal(&self, input: &str, schema: &Schema) -> Result<BashVal, CodecParseError> {
        let trimmed = input.trim();
        let inner = trimmed
            .strip_prefix('(')
            .and_then(|rest| rest.strip_suffix(')'))
            .ok_or_else(|| {
                CodecParseError::LayoutError(format!("expected (...) array literal: {trimmed:?}"))
            })?;

        self.parse(&parse_q_words(inner)?, schema)
    }

    /// A one-dimensional literal as its words: `('a' 'b')` → `["a", "b"]`.
    fn words(&self, input: &str) -> Result<Vec<String>, CodecParseError> {
        shaped(self.parse_literal(input, &Schema::n_d(1))?.words(), "words", input)
    }

    /// A two-dimensional literal as its rows: `("'a' 'b'" "'c'")` →
    /// `[["a", "b"], ["c"]]`.
    fn rows(&self, input: &str) -> Result<Vec<Vec<String>>, CodecParseError> {
        shaped(self.parse_literal(input, &Schema::n_d(2))?.rows(), "rows", input)
    }
}

fn shaped<T>(got: Option<T>, wanted: &str, input: &str) -> Result<T, CodecParseError> {
    got.ok_or_else(|| CodecParseError::LayoutError(format!("expected {wanted}: {input:?}")))
}

pub struct QuotedNest;

impl BashCodec for QuotedNest {
    fn emit(&self, val: &BashVal) -> Vec<String> {
        match val {
            BashVal::Str(word) => vec![word.clone()],
            BashVal::Arr(items) => items
                .iter()
                .map(|item| match item {
                    BashVal::Str(word) => word.clone(),
                    BashVal::Arr(_) => self.emit_literal(item),
                })
                .collect(),
        }
    }

    fn parse(&self, words: &[String], schema: &Schema) -> Result<BashVal, CodecParseError> {
        match schema {
            Schema::Scalar => match words {
                [only] => Ok(BashVal::Str(only.clone())),
                _ => Err(CodecParseError::ExpectedScalar),
            },
            Schema::Arr(inner) => words
                .iter()
                .map(|word| match **inner {
                    Schema::Scalar => Ok(BashVal::Str(word.clone())),
                    Schema::Arr(_) => self.parse_literal(word, inner),
                })
                .collect::<Result<_, _>>()
                .map(BashVal::Arr),
        }
    }
}

/// A group is prefixed by its width — the full inner word stream, nested
/// prefixes included — exactly where its elements are themselves groups,
/// since a scalar is already one bash word.
///
/// | value | words |
/// |---|---|
/// | `[a, b]` | `a b` |
/// | `[[a,b],[c,d,e]]` | `2 a b 3 c d e` |
/// | `[[[a,b],[c]]]` | `5 2 a b 1 c` |
/// | `[[[a,b]],[[c]]]` | `3 2 a b 2 1 c` |
///
/// Matches `glue-core/src/data/linked_arr.bash::LinkedArr__Add` /
/// `LinkedArr__Call`.
pub struct LinkedArr;

impl BashCodec for LinkedArr {
    fn emit(&self, val: &BashVal) -> Vec<String> {
        match val {
            BashVal::Str(word) => vec![word.clone()],
            BashVal::Arr(items) => {
                let nested = matches!(items.first(), Some(BashVal::Arr(_)));
                let mut out = Vec::new();
                for item in items {
                    let body = self.emit(item);
                    if nested {
                        out.push(body.len().to_string());
                    }
                    out.extend(body);
                }
                out
            }
        }
    }

    fn parse(&self, words: &[String], schema: &Schema) -> Result<BashVal, CodecParseError> {
        match schema {
            Schema::Scalar => match words {
                [only] => Ok(BashVal::Str(only.clone())),
                _ => Err(CodecParseError::ExpectedScalar),
            },
            Schema::Arr(_) => {
                let (val, consumed) = parse_body(words, schema)?;
                if consumed != words.len() {
                    return Err(CodecParseError::LayoutError(format!(
                        "trailing words: consumed {consumed} of {}",
                        words.len()
                    )));
                }
                Ok(val)
            }
        }
    }
}

fn parse_body(words: &[String], schema: &Schema) -> Result<(BashVal, usize), CodecParseError> {
    let Schema::Arr(inner) = schema else {
        return match words.first() {
            Some(word) => Ok((BashVal::Str(word.clone()), 1)),
            None => Err(CodecParseError::LayoutError("scalar position; no word".into())),
        };
    };

    let grouped = matches!(**inner, Schema::Arr(_));
    let mut items = Vec::new();
    let mut at = 0;

    while at < words.len() {
        if !grouped {
            items.push(BashVal::Str(words[at].clone()));
            at += 1;
            continue;
        }

        let width: usize = words[at].parse().map_err(|_| {
            CodecParseError::LayoutError(format!(
                "length prefix not numeric at pos {at}: {:?}",
                words[at]
            ))
        })?;
        at += 1;

        let end = at + width;
        if end > words.len() {
            return Err(CodecParseError::LayoutError(format!(
                "group claims {width} words; only {} available",
                words.len() - at
            )));
        }

        let (item, consumed) = parse_body(&words[at..end], inner)?;
        if consumed != end - at {
            return Err(CodecParseError::LayoutError(format!(
                "nested group: consumed {consumed} of {} body words",
                end - at
            )));
        }
        items.push(item);
        at = end;
    }

    Ok((BashVal::Arr(items), at))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(words: &[&str]) -> BashVal {
        BashVal::row(words.iter().copied())
    }

    fn arr(items: Vec<BashVal>) -> BashVal {
        BashVal::Arr(items)
    }

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(|word| word.to_string()).collect()
    }

    /// Each inner array becomes one quoted word at the outer level, and comes
    /// back through the schema that says how deep to look.
    #[test]
    fn quoted_nest_wraps_a_level_per_dimension() {
        let two_d = arr(vec![row(&["a", "b"]), row(&["c", "d", "e"])]);

        assert_eq!(QuotedNest.emit(&two_d), words(&["('a' 'b')", "('c' 'd' 'e')"]));
        assert_eq!(QuotedNest.parse(&QuotedNest.emit(&two_d), &Schema::n_d(2)).unwrap(), two_d);
    }

    #[test]
    fn linked_arr_prefixes_each_group_with_its_width() {
        assert_eq!(
            LinkedArr.emit(&arr(vec![row(&["a", "b"]), row(&["c", "d", "e"])])),
            words(&["2", "a", "b", "3", "c", "d", "e"])
        );
        assert_eq!(
            LinkedArr.emit(&arr(vec![arr(vec![row(&["a", "b"]), row(&["c"])])])),
            words(&["5", "2", "a", "b", "1", "c"])
        );
        assert_eq!(
            LinkedArr.emit(&arr(vec![arr(vec![row(&["a", "b"])]), arr(vec![row(&["c"])])])),
            words(&["3", "2", "a", "b", "2", "1", "c"])
        );
    }

    #[test]
    fn linked_arr_round_trips_at_three_dimensions() {
        let three_d =
            arr(vec![arr(vec![row(&["a", "b"]), row(&["c"])]), arr(vec![row(&["d", "e"])])]);

        assert_eq!(LinkedArr.parse(&LinkedArr.emit(&three_d), &Schema::n_d(3)).unwrap(), three_d);
    }
}
