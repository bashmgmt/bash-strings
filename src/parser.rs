//! The four forms bash prints a value in, and nothing else.
//!
//! Each is strict: it accepts what bash itself writes and refuses the rest.
//! Where a word begins and ends is [`quoting`](super::quoting)'s; this is what
//! surrounds one.
//!
//! | | |
//! |---|---|
//! | scalar | one word |
//! | q_words | words separated by exactly one space |
//! | indexed | `([0]=… [5]=…)` — subscripts are data, and sparse |
//! | assoc | `([k]=… )` — the key is a word too |

use indexmap::IndexMap;
use winnow::Parser;
use winnow::token::take_while;

use super::error::{cut, ParseError};
use super::quoting::{run, word, ws0};

/// Where a value ends. `)` closes a compound, so it stops a word inside one.
const VALUE_STOPS: &[char] = &[' ', '\t', '\n', ')'];

/// Where a subscript ends, inside its brackets.
const KEY_STOPS: &[char] = &[']'];

pub fn parse_scalar(s: &str) -> Result<String, ParseError> {
    run(|i| word(VALUE_STOPS, i), s.trim_end_matches('\n'))
}

pub fn parse_q_words(s: &str) -> Result<Vec<String>, ParseError> {
    run(q_words, s.trim_end_matches('\n'))
}

pub fn parse_indexed(s: &str) -> Result<IndexMap<usize, String>, ParseError> {
    run(indexed_compound, s.trim_end_matches('\n'))
}

pub fn parse_assoc(s: &str) -> Result<IndexMap<String, String>, ParseError> {
    run(assoc_compound, s.trim_end_matches('\n'))
}

/// Exactly one space between words: bash writes no more, so more is not its
/// output.
fn q_words(input: &mut &str) -> winnow::ModalResult<Vec<String>> {
    let mut out = Vec::new();
    if input.is_empty() { return Ok(out); }
    out.push(word(VALUE_STOPS, input)?);
    while !input.is_empty() {
        " ".parse_next(input)?;
        out.push(word(VALUE_STOPS, input)?);
    }
    Ok(out)
}

fn indexed_compound(input: &mut &str) -> winnow::ModalResult<IndexMap<usize, String>> {
    "(".parse_next(input)?;
    ws0(input);
    let mut out = IndexMap::new();
    while !input.starts_with(')') {
        let n = bracket_index(input)?;
        "=".parse_next(input)?;
        let v = word(VALUE_STOPS, input)?;
        out.insert(n, v);
        ws0(input);
    }
    ")".parse_next(input)?;
    Ok(out)
}

fn assoc_compound(input: &mut &str) -> winnow::ModalResult<IndexMap<String, String>> {
    "(".parse_next(input)?;
    ws0(input);
    let mut out = IndexMap::new();
    while !input.starts_with(')') {
        "[".parse_next(input)?;
        let k = word(KEY_STOPS, input)?;
        "]".parse_next(input)?;
        "=".parse_next(input)?;
        let v = word(VALUE_STOPS, input)?;
        out.insert(k, v);
        ws0(input);
    }
    ")".parse_next(input)?;
    Ok(out)
}

fn bracket_index(input: &mut &str) -> winnow::ModalResult<usize> {
    "[".parse_next(input)?;
    let digits: &str = take_while(1.., |c: char| c.is_ascii_digit()).parse_next(input)?;
    "]".parse_next(input)?;

    // A bash subscript is a machine integer. One too wide to be one was not
    // printed by bash, so it is rejected rather than wrapped or truncated.
    digits.parse().map_err(|_| cut())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ix<I: IntoIterator<Item = (usize, &'static str)>>(it: I) -> IndexMap<usize, String> {
        it.into_iter().map(|(k, v)| (k, v.to_string())).collect()
    }
    fn ax<I: IntoIterator<Item = (&'static str, &'static str)>>(it: I) -> IndexMap<String, String> {
        it.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn scalar_canonical_forms() {
        assert_eq!(parse_scalar("'hello world'").unwrap(), "hello world");
        assert_eq!(parse_scalar(r#""hello \$VAR""#).unwrap(), "hello $VAR");
        assert_eq!(parse_scalar(r"$'a\nb'").unwrap(), "a\nb");
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
        assert_eq!(parse_q_words("'a' 'b'").unwrap(),         vec!["a", "b"]);
        assert_eq!(parse_q_words("'a b' $'c\\nd'").unwrap(),  vec!["a b", "c\nd"]);
        assert_eq!(parse_q_words("").unwrap(),                Vec::<String>::new());
    }

    #[test]
    fn q_words_rejects_non_canonical_spacing() {
        assert!(parse_q_words("'a'  'b'").is_err());
        assert!(parse_q_words(" 'a' 'b'").is_err());
        assert!(parse_q_words("'a' 'b' ").is_err());
    }

    #[test]
    fn ansi_c_escapes() {
        assert_eq!(parse_q_words(r"$'\t\r\\\''").unwrap(), vec!["\t\r\\'"]);
        assert_eq!(parse_q_words(r"$'\x41' $'\101'").unwrap(), vec!["A", "A"]);
    }

    #[test]
    fn indexed_canonical() {
        assert_eq!(parse_indexed("([0]='a' [1]='b')").unwrap(),  ix([(0, "a"), (1, "b")]));
        assert_eq!(parse_indexed("()").unwrap(),                  ix([]));
        assert_eq!(parse_indexed(r#"([0]="a" [1]="b c" [2]=$'d\ne')"#).unwrap(),
                   ix([(0, "a"), (1, "b c"), (2, "d\ne")]));
    }

    #[test]
    fn indexed_sparse_ascending() {
        assert_eq!(parse_indexed(r#"([0]="zero" [2]="two" [5]="five")"#).unwrap(),
                   ix([(0, "zero"), (2, "two"), (5, "five")]));
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
        assert_eq!(parse_assoc(r#"([k]="v")"#).unwrap(),  ax([("k", "v")]));
        assert_eq!(parse_assoc("()").unwrap(),             ax([]));
        assert_eq!(parse_assoc(r#"([foo]="1" [c]="3" )"#).unwrap(),
                   ax([("foo", "1"), ("c", "3")]));
    }

    #[test]
    fn assoc_quoted_key_ansi_value() {
        assert_eq!(parse_assoc(r#"([foo]="1" ["k 2"]=$'v\n2' [c]="3" )"#).unwrap(),
                   ax([("foo", "1"), ("k 2", "v\n2"), ("c", "3")]));
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
        assert_eq!(parse_q_words(r"$'\377'").unwrap(), vec!["\u{ff}"]);
        assert_eq!(parse_q_words(r"$'\0'").unwrap(), vec!["\0"]);
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
        assert!(long.contains(&failed.snippet), "{failed}");
    }
}
