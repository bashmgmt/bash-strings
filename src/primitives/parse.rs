use super::types::ParseError;
use super::single_quoting::parse_single_quoted_body;
use super::ansi_c_quoting::parse_ansi_c_body;

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

/// Repeatedly parse @Q-quoted words from a single-line string, skipping interior
/// whitespace. Errors on trailing non-whitespace.
pub fn parse_words(input: &str) -> Result<Vec<String>, ParseError> {
    let mut out = Vec::new();
    let mut rest = input.trim();
    while !rest.is_empty() {
        let (word, remaining) = parse_one_word(rest)?;
        out.push(word);
        rest = remaining.trim_start();
    }
    Ok(out)
}

fn is_word_char(b: u8) -> bool {
    !b.is_ascii_whitespace() && b != b'(' && b != b')' && b != b'['
        && b != b']' && b != b'\'' && b != b'"' && b != b'$' && b != b'\\'
}
