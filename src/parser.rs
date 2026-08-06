//! Strict bash-value parsers — accept only canonical bash output.
//!
//! Word grammar (any quoting form, concatenable into one word):
//!   `'…'` single | `"…"` double | `$'…'` ANSI-C | `$"…"` locale | bare
//!
//! Adjacent forms concatenate: `a"b"c'd'$'e'` → `abcde`. Each word ends at
//! a context-defined stop char (whitespace, `)`, `]`, ...). Stops are
//! passed to `word` explicitly; the typed entry points use the canonical
//! `VALUE_STOPS` / `KEY_STOPS`.

use std::fmt;
use indexmap::IndexMap;
use winnow::Parser;
use winnow::error::{ContextError, ErrMode};
use winnow::token::{take_until, take_while};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub at: usize,
    pub snippet: String,
}

impl ParseError {
    pub fn new(input: &str, at: usize, message: impl Into<String>) -> Self {
        Self { message: message.into(), at, snippet: around(input, at) }
    }
    fn from_winnow<E: fmt::Display>(input: &str, at: usize, err: E) -> Self {
        Self::new(input, at, err.to_string())
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bash value parse error at byte {}: {} — near {:?}",
            self.at, self.message, self.snippet)
    }
}

impl std::error::Error for ParseError {}

/// A refusal that ends the parse. Once a quoted form's opener is read there
/// is only one way to finish it, so there is nothing to back off to — unlike
/// a bare segment, which `word` retries.
fn cut() -> ErrMode<ContextError> {
    ErrMode::Cut(ContextError::new())
}

/// The text around an offset, widened to character boundaries — so a snippet
/// is still a snippet when the input holds multi-byte characters.
fn around(input: &str, at: usize) -> String {
    let mut lo = at.saturating_sub(20).min(input.len());
    let mut hi = (at + 20).min(input.len());

    while !input.is_char_boundary(lo) {
        lo -= 1;
    }
    while !input.is_char_boundary(hi) {
        hi += 1;
    }
    input[lo..hi].to_string()
}

const VALUE_STOPS: &[char] = &[' ', '\t', '\n', ')'];
pub(crate) const KEY_STOPS:   &[char] = &[']'];

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

pub(crate) fn run<T>(
    mut p: impl FnMut(&mut &str) -> winnow::ModalResult<T>,
    input: &str,
) -> Result<T, ParseError> {
    let mut cursor = input;
    match p(&mut cursor) {
        Ok(v) if cursor.is_empty() => Ok(v),
        Ok(_) => Err(ParseError::new(input, input.len() - cursor.len(), "trailing input")),
        Err(e) => Err(ParseError::from_winnow(input, input.len() - cursor.len(), e)),
    }
}

pub(crate) fn ws0(input: &mut &str) {
    while let Some(c) = input.chars().next() {
        if c == ' ' || c == '\t' || c == '\n' { *input = &input[c.len_utf8()..]; } else { break; }
    }
}

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

pub(crate) fn word(stops: &[char], input: &mut &str) -> winnow::ModalResult<String> {
    let mut out = first_segment(stops, input)?;
    loop {
        let snap = *input;
        match next_segment(stops, input) {
            Ok(Some(s)) => out.push_str(&s),
            Ok(None) | Err(_) => { *input = snap; break; }
        }
    }
    Ok(out)
}

fn first_segment(stops: &[char], input: &mut &str) -> winnow::ModalResult<String> {
    if input.starts_with("$'") { return single_ansi_c(input); }
    if input.starts_with('\'') { return single_quoted(input); }
    if input.starts_with('"')  { return double_quoted(input); }
    if input.starts_with("$\"") { *input = &input[1..]; return double_quoted(input); }
    bare(stops, input)
}

fn next_segment(stops: &[char], input: &mut &str) -> winnow::ModalResult<Option<String>> {
    if input.starts_with("$'") { return single_ansi_c(input).map(Some); }
    if input.starts_with('\'') { return single_quoted(input).map(Some); }
    if input.starts_with('"')  { return double_quoted(input).map(Some); }
    if input.starts_with("$\"") { *input = &input[1..]; return double_quoted(input).map(Some); }
    match input.chars().next() {
        Some(c) if !stops.contains(&c) && c != '\\' => bare(stops, input).map(Some),
        Some('\\') => bare(stops, input).map(Some),
        _ => Ok(None),
    }
}

fn single_quoted(input: &mut &str) -> winnow::ModalResult<String> {
    "'".parse_next(input)?;
    let body = take_until(0.., "'").parse_next(input)?;
    "'".parse_next(input)?;
    Ok(body.to_string())
}

fn double_quoted(input: &mut &str) -> winnow::ModalResult<String> {
    "\"".parse_next(input)?;
    let mut out = String::new();
    loop {
        // Reading the character is what says there is one, so the body below
        // never has to ask again.
        let Some(c) = input.chars().next() else { return Err(cut()) };

        if c == '"' { *input = &input[1..]; return Ok(out); }
        if c == '\\' {
            let rest = &input[1..];
            match rest.chars().next() {
                Some(c @ ('$' | '"' | '\\' | '`')) => { out.push(c); *input = &rest[c.len_utf8()..]; }
                Some('\n') => { *input = &rest[1..]; }
                Some(c) => { out.push('\\'); out.push(c); *input = &rest[c.len_utf8()..]; }
                None => return Err(cut()),
            }
            continue;
        }
        out.push(c);
        *input = &input[c.len_utf8()..];
    }
}

fn single_ansi_c(input: &mut &str) -> winnow::ModalResult<String> {
    "$'".parse_next(input)?;
    let mut out = String::new();
    loop {
        let Some(c) = input.chars().next() else { return Err(cut()) };

        if c == '\'' { *input = &input[1..]; return Ok(out); }
        if c == '\\' {
            let rest = &input[1..];
            let escaped = rest.chars().next().ok_or_else(cut)?;

            *input = &rest[escaped.len_utf8()..];
            decode_ansi_c_escape(escaped, input, &mut out)?;
            continue;
        }
        out.push(c);
        *input = &input[c.len_utf8()..];
    }
}

fn decode_ansi_c_escape(c: char, input: &mut &str, out: &mut String) -> winnow::ModalResult<()> {
    match c {
        'a' => out.push('\x07'),
        'b' => out.push('\x08'),
        'e' | 'E' => out.push('\x1B'),
        'f' => out.push('\x0C'),
        'n' => out.push('\n'),
        'r' => out.push('\r'),
        't' => out.push('\t'),
        'v' => out.push('\x0B'),
        '\\' => out.push('\\'),
        '\'' => out.push('\''),
        '"' => out.push('"'),
        '?' => out.push('?'),
        'c' => {
            let cc = input.chars().next().ok_or_else(cut)?;
            *input = &input[cc.len_utf8()..];
            out.push(((cc as u32) & 0x1F) as u8 as char);
        }
        'x' => push_radix(input, out, 2, 16)?,
        'u' => push_unicode(input, out, 4)?,
        'U' => push_unicode(input, out, 8)?,
        d if d.is_ascii_digit() && d != '8' && d != '9' => {
            let mut oct = String::from(d);
            for _ in 0..2 {
                match input.chars().next() {
                    Some(dd) if dd.is_ascii_digit() && dd != '8' && dd != '9' => {
                        oct.push(dd);
                        *input = &input[dd.len_utf8()..];
                    }
                    _ => break,
                }
            }
            // Three octal digits reach 511. Bash prints no escape above
            // `\377`, so a wider one is not its output.
            let byte = u8::from_str_radix(&oct, 8)
                .map_err(|_| cut())?;

            out.push(byte as char);
        }
        _ => { out.push('\\'); out.push(c); }
    }
    Ok(())
}

fn push_radix(input: &mut &str, out: &mut String, max: usize, radix: u32) -> winnow::ModalResult<()> {
    let h = take_hex(input, max);
    if h.is_empty() { return Err(cut()); }
    let v = u8::from_str_radix(&h, radix).map_err(|_| cut())?;
    out.push(v as char);
    Ok(())
}

fn push_unicode(input: &mut &str, out: &mut String, max: usize) -> winnow::ModalResult<()> {
    let h = take_hex(input, max);
    if h.is_empty() { return Err(cut()); }
    let v = u32::from_str_radix(&h, 16).map_err(|_| cut())?;
    out.push(char::from_u32(v).ok_or_else(cut)?);
    Ok(())
}

fn take_hex(input: &mut &str, max: usize) -> String {
    let mut out = String::new();
    for _ in 0..max {
        match input.chars().next() {
            Some(c) if c.is_ascii_hexdigit() => { out.push(c); *input = &input[c.len_utf8()..]; }
            _ => break,
        }
    }
    out
}

fn bare(stops: &[char], input: &mut &str) -> winnow::ModalResult<String> {
    let mut out = String::new();
    while let Some(c) = input.chars().next() {
        if c == '\'' || c == '"' || c == '$' { break; }
        if c == '\\' {
            let rest = &input[1..];
            match rest.chars().next() {
                Some('\n') => { *input = &rest[1..]; continue; }
                Some(e) => { out.push(e); *input = &rest[e.len_utf8()..]; continue; }
                None => break,
            }
        }
        if stops.contains(&c) { break; }
        out.push(c);
        *input = &input[c.len_utf8()..];
    }
    if out.is_empty() { return Err(ErrMode::Backtrack(ContextError::new())); }
    Ok(out)
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
