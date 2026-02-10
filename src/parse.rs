use std::collections::HashMap;
use super::types::{BashType, BashValue, ParseError};
use super::single_quoting::parse_single_quoted_body;
use super::ansi_c_quoting::parse_ansi_c_body;

/// The governing parse function. Interprets raw `@Q` output according to the known type.
///
/// - `Scalar`: one quoted word from `echo "${var@Q}"`
/// - `IndexedArray`: space-separated quoted words from `echo "${arr[@]@Q}"`
/// - `AssocArray`: newline-separated `key value` pairs, each line two quoted words
///   (produced by: `for k in "${!m[@]}"; do printf '%s %s\n' "${k@Q}" "${m[$k]@Q}"; done`)
pub fn parse_typed_value(bash_type: BashType, input: &str) -> Result<BashValue, ParseError> {
    let input = input.trim();
    match bash_type {
        BashType::Scalar => {
            if input.is_empty() {
                return Ok(BashValue::String(String::new()));
            }
            let (word, rest) = parse_one_word(input)?;
            let rest = rest.trim();
            if !rest.is_empty() {
                return Err(ParseError::TrailingContent(rest.to_string()));
            }
            Ok(BashValue::String(word))
        }
        BashType::IndexedArray => {
            let mut words = Vec::new();
            let mut rest = input;
            loop {
                rest = rest.trim_start();
                if rest.is_empty() { break; }
                let (word, remaining) = parse_one_word(rest)?;
                words.push(word);
                rest = remaining;
            }
            Ok(BashValue::IndexedArray(words))
        }
        BashType::AssocArray => {
            let mut map = HashMap::new();
            for line in input.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }
                let (key, rest) = parse_one_word(line)?;
                let rest = rest.trim_start();
                if rest.is_empty() {
                    return Err(ParseError::ExpectedPair(
                        format!("got key {key:?} with no value")
                    ));
                }
                let (value, rest) = parse_one_word(rest)?;
                let rest = rest.trim();
                if !rest.is_empty() {
                    return Err(ParseError::TrailingContent(rest.to_string()));
                }
                map.insert(key, value);
            }
            Ok(BashValue::AssocArray(map))
        }
    }
}

/// Parse one bash `@Q`-produced word: `'...'`, `$'...'`, `\x` escape, or unquoted.
/// Handles the `'\''` pattern that bash @Q uses for embedded single quotes.
/// Returns `(parsed_string, remaining_input)`.
pub fn parse_one_word(input: &str) -> Result<(String, &str), ParseError> {
    let mut result = String::new();
    let mut rest = input;

    loop {
        if rest.is_empty() { break; }

        if rest.starts_with("$'") {
            let (s, remaining) = parse_ansi_c_body(&rest[2..])?;
            result.push_str(&s);
            rest = remaining;
        } else if rest.starts_with('\'') {
            let (s, remaining) = parse_single_quoted_body(&rest[1..])?;
            result.push_str(&s);
            rest = remaining;
        } else if rest.starts_with('\\') && rest.len() > 1 {
            let escaped = rest[1..].chars().next().unwrap();
            result.push(escaped);
            rest = &rest[1 + escaped.len_utf8()..];
        } else if is_word_char(rest.as_bytes()[0]) {
            let end = rest.find(|c: char| !is_word_char(c as u8)).unwrap_or(rest.len());
            result.push_str(&rest[..end]);
            rest = &rest[end..];
        } else {
            break;
        }
    }

    Ok((result, rest))
}

fn is_word_char(b: u8) -> bool {
    !b.is_ascii_whitespace() && b != b'(' && b != b')' && b != b'['
        && b != b']' && b != b'\'' && b != b'"' && b != b'$' && b != b'\\'
}
