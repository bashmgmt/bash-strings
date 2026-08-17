//! The forms bash prints a value in, and nothing else.
//!
//! Each is strict: it accepts what bash itself writes and refuses the rest.
//! Where a word begins and ends is [`quoting`](super::quoting)'s; this is what
//! surrounds one.
//!
//! | | |
//! |---|---|
//! | scalar | one word |
//! | q_words | words separated by exactly one space |
//! | array | `('a' 'b c')` — q_words in parentheses |
//! | rows | `("'a' 'b'" "'c'")` — an array of arrays, one level of nesting |
//! | indexed | `([0]=… [5]=…)` — subscripts are data, and sparse |
//! | assoc | `([k]=… )` — the key is a word too |

use indexmap::IndexMap;

use super::error::ParseError;
use super::quoting::{Cursor, parse_with};

/// Where a value ends. `)` closes a compound, so it stops a word inside one.
const VALUE_STOPS: &[char] = &[' ', '\t', '\n', ')'];

/// Where a subscript ends, inside its brackets.
const KEY_STOPS: &[char] = &[']'];

pub fn parse_scalar(text: &str) -> Result<String, ParseError> {
    parse_with(trimmed(text), |c| c.word(VALUE_STOPS))
}

pub fn parse_q_words(text: &str) -> Result<Vec<String>, ParseError> {
    parse_with(trimmed(text), q_words)
}

/// One bash array literal as its words: `('a' 'b c')` → `["a", "b c"]`.
///
/// Codec-independent — at one dimension [`QuotedNest`](super::QuotedNest) and
/// [`LinkedArr`](super::LinkedArr) write the same text, so there is nothing to
/// choose. Deeper values go through [`BashCodec`](super::BashCodec), where the choice is real.
pub fn parse_array(text: &str) -> Result<Vec<String>, ParseError> {
    parse_q_words(inside(text)?)
}

pub fn parse_indexed(text: &str) -> Result<IndexMap<usize, String>, ParseError> {
    parse_with(trimmed(text), indexed_compound)
}

pub fn parse_assoc(text: &str) -> Result<IndexMap<String, String>, ParseError> {
    parse_with(trimmed(text), assoc_compound)
}

/// The body of a `(…)` array literal, which is the one shape that surrounds
/// every other. Spelled here and nowhere else on the reading side;
/// [`emit_array`](super::emit_array) is its inverse.
pub(super) fn inside(text: &str) -> Result<&str, ParseError> {
    let trimmed = text.trim();

    trimmed
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or_else(|| {
            ParseError::new(
                trimmed,
                0,
                "expected a (…) array literal",
            )
        })
}

/// Bash writes no trailing newline inside a value; one appended by a `$( )`
/// or a file read is not part of it.
fn trimmed(text: &str) -> &str {
    text.trim_end_matches('\n')
}

/// Exactly one space between words: bash writes no more, so more is not its
/// output.
fn q_words(c: &mut Cursor<'_>) -> Result<Vec<String>, ParseError> {
    let mut out = Vec::new();
    if c.at_end() {
        return Ok(out);
    }

    out.push(c.word(VALUE_STOPS)?);
    while !c.at_end() {
        c.lit(" ")?;
        out.push(c.word(VALUE_STOPS)?);
    }
    Ok(out)
}

fn indexed_compound(c: &mut Cursor<'_>) -> Result<IndexMap<usize, String>, ParseError> {
    c.lit("(")?;
    c.ws0();

    let mut out = IndexMap::new();
    while !c.starts_with(")") {
        let index = bracket_index(c)?;
        c.lit("=")?;
        out.insert(index, c.word(VALUE_STOPS)?);
        c.ws0();
    }
    c.lit(")")?;

    Ok(out)
}

fn assoc_compound(c: &mut Cursor<'_>) -> Result<IndexMap<String, String>, ParseError> {
    c.lit("(")?;
    c.ws0();

    let mut out = IndexMap::new();
    while !c.starts_with(")") {
        c.lit("[")?;
        let key = c.word(KEY_STOPS)?;
        c.lit("]")?;
        c.lit("=")?;
        out.insert(key, c.word(VALUE_STOPS)?);
        c.ws0();
    }
    c.lit(")")?;

    Ok(out)
}

fn bracket_index(c: &mut Cursor<'_>) -> Result<usize, ParseError> {
    c.lit("[")?;

    let digits = c.take_while(|d| d.is_ascii_digit());
    if digits.is_empty() {
        return Err(c.fail("expected a subscript"));
    }

    // A bash subscript is a machine integer. One too wide to be one was not
    // printed by bash, so it is rejected rather than wrapped or truncated.
    let index = digits.parse().map_err(|_| {
        c.fail(format!(
            "subscript {digits:?} is not an index"
        ))
    })?;
    c.lit("]")?;

    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::super::codec::{BashCodec, QuotedNest};
    use super::*;
    use crate::{BashVal, LinkedArr, emit_array};

    fn ix<I: IntoIterator<Item = (usize, &'static str)>>(it: I) -> IndexMap<usize, String> {
        it.into_iter().map(|(k, v)| (k, v.to_string())).collect()
    }
    fn ax<I: IntoIterator<Item = (&'static str, &'static str)>>(it: I) -> IndexMap<String, String> {
        it.into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn scalar_canonical_forms() {
        assert_eq!(
            parse_scalar("'hello world'").unwrap(),
            "hello world"
        );
        assert_eq!(
            parse_scalar(r#""hello \$VAR""#).unwrap(),
            "hello $VAR"
        );
        assert_eq!(
            parse_scalar(r"$'a\nb'").unwrap(),
            "a\nb"
        );
        assert_eq!(parse_scalar("''").unwrap(), "");
    }

    #[test]
    fn scalar_concat() {
        assert_eq!(parse_scalar("'a''b'").unwrap(), "ab");
    }

    #[test]
    fn scalar_rejects_non_canonical() {
        assert!(parse_scalar("").is_err());
        assert!(parse_scalar("'a' 'b'").is_err());
        assert!(parse_scalar(" 'a'").is_err());
    }

    #[test]
    fn q_words_canonical() {
        assert_eq!(
            parse_q_words("'a' 'b'").unwrap(),
            vec!["a", "b"]
        );
        assert_eq!(
            parse_q_words("'a b' $'c\\nd'").unwrap(),
            vec!["a b", "c\nd"]
        );
        assert_eq!(
            parse_q_words("").unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn q_words_rejects_non_canonical_spacing() {
        assert!(parse_q_words("'a'  'b'").is_err());
        assert!(parse_q_words(" 'a' 'b'").is_err());
        assert!(parse_q_words("'a' 'b' ").is_err());
    }

    #[test]
    fn ansi_c_escapes() {
        assert_eq!(
            parse_q_words(r"$'\t\r\\\''").unwrap(),
            vec!["\t\r\\'"]
        );
        assert_eq!(
            parse_q_words(r"$'\x41' $'\101'").unwrap(),
            vec!["A", "A"]
        );
    }

    #[test]
    fn array_round_trips_and_needs_its_parentheses() {
        let words = vec!["a".to_string(), "b c".into(), "d\ne".into(), String::new()];

        assert_eq!(
            emit_array(&words),
            "('a' 'b c' $'d\\ne' '')"
        );
        assert_eq!(
            parse_array(&emit_array(&words)).unwrap(),
            words
        );
        assert_eq!(
            parse_array("()").unwrap(),
            Vec::<String>::new()
        );

        let bare = parse_array("'a' 'b'").expect_err("no parentheses");
        assert!(
            bare.message.contains("array literal"),
            "{bare}"
        );
    }

    /// At one dimension the two codecs write the same text, which is what
    /// lets `parse_array` take no codec.
    #[test]
    fn one_dimension_is_the_same_under_either_codec() {
        let words = vec!["a".to_string(), "b c".into(), "2".into()];
        let value = BashVal::row(words.clone());

        assert_eq!(
            QuotedNest.emit_literal(&value),
            LinkedArr.emit_literal(&value)
        );
        assert_eq!(
            QuotedNest.emit_literal(&value),
            emit_array(&words)
        );
        assert_eq!(
            parse_array(&emit_array(&words)).unwrap(),
            words
        );
    }

    #[test]
    fn indexed_canonical() {
        assert_eq!(
            parse_indexed("([0]='a' [1]='b')").unwrap(),
            ix([(0, "a"), (1, "b")])
        );
        assert_eq!(parse_indexed("()").unwrap(), ix([]));
        assert_eq!(
            parse_indexed(r#"([0]="a" [1]="b c" [2]=$'d\ne')"#).unwrap(),
            ix([(0, "a"), (1, "b c"), (2, "d\ne")])
        );
    }

    #[test]
    fn indexed_sparse_ascending() {
        assert_eq!(
            parse_indexed(r#"([0]="zero" [2]="two" [5]="five")"#).unwrap(),
            ix([(0, "zero"), (2, "two"), (5, "five")])
        );
    }

    #[test]
    fn indexed_rejects_non_canonical() {
        assert!(parse_indexed("").is_err());
        assert!(parse_indexed("[0]='a' [1]='b'").is_err());
        assert!(parse_indexed("([0]=)").is_err());
        assert!(parse_indexed("([0]='a'").is_err());
    }

    #[test]
    fn assoc_canonical() {
        assert_eq!(
            parse_assoc(r#"([k]="v")"#).unwrap(),
            ax([("k", "v")])
        );
        assert_eq!(parse_assoc("()").unwrap(), ax([]));
        assert_eq!(
            parse_assoc(r#"([foo]="1" [c]="3" )"#).unwrap(),
            ax([("foo", "1"), ("c", "3")])
        );
    }

    #[test]
    fn assoc_quoted_key_ansi_value() {
        assert_eq!(
            parse_assoc(r#"([foo]="1" ["k 2"]=$'v\n2' [c]="3" )"#).unwrap(),
            ax([("foo", "1"), ("k 2", "v\n2"), ("c", "3")])
        );
    }

    #[test]
    fn assoc_rejects_non_canonical() {
        assert!(parse_assoc("").is_err());
        assert!(parse_assoc("[k]='v'").is_err());
        assert!(parse_assoc("([k]=)").is_err());
    }

    /// Bash prints a byte it cannot show as up to three octal digits, and the
    /// widest it ever prints is `\377`. Anything above that is not its output
    /// and is refused rather than wrapped — three digits reach 511, which is
    /// where an unchecked `u8` conversion would have given up.
    #[test]
    fn an_octal_escape_stops_at_a_byte() {
        assert_eq!(
            parse_q_words(r"$'\377'").unwrap(),
            vec!["\u{ff}"]
        );
        assert_eq!(
            parse_q_words(r"$'\0'").unwrap(),
            vec!["\0"]
        );
        assert!(parse_q_words(r"$'\400'").is_err());
        assert!(parse_q_words(r"$'\777'").is_err());
    }

    /// A subscript is a machine integer. One too wide to be one was never
    /// printed by bash, and is an error rather than a panic.
    #[test]
    fn a_subscript_too_wide_to_be_one_is_refused() {
        assert!(parse_indexed("([99999999999999999999999]='a')").is_err());
        assert_eq!(
            parse_indexed(&format!("([{}]='a')", usize::MAX)).unwrap(),
            ix([(usize::MAX, "a")]),
            "the widest one that is still an index"
        );
    }

    /// The snippet in an error is cut to character boundaries, so an input
    /// with multi-byte characters still reports one.
    #[test]
    fn an_error_reports_the_text_around_it() {
        let long = format!("'{}' trailing", "é".repeat(30));
        let failed = parse_scalar(&long).expect_err("trailing input");

        assert!(!failed.snippet.is_empty(), "{failed}");
        assert!(
            long.contains(&failed.snippet),
            "{failed}"
        );
    }
}
