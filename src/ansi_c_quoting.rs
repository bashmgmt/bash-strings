use super::types::ParseError;

/// Parse the interior of a `$'...'` ANSI-C quoted string.
/// Input starts AFTER the opening `$'`. Returns `(parsed_string, rest_after_closing_quote)`.
pub fn parse_ansi_c_body(input: &str) -> Result<(String, &str), ParseError> {
    let mut result = String::new();
    let mut chars = input.char_indices();

    loop {
        match chars.next() {
            None => return Err(ParseError::UnterminatedAnsiCQuote),
            Some((_, '\'')) => return Ok((result, chars.as_str())),
            Some((i, '\\')) => match chars.next() {
                None => return Err(ParseError::UnterminatedAnsiCQuote),
                Some((_, 'a')) => result.push('\x07'),
                Some((_, 'b')) => result.push('\x08'),
                Some((_, 'e')) | Some((_, 'E')) => result.push('\x1B'),
                Some((_, 'f')) => result.push('\x0C'),
                Some((_, 'n')) => result.push('\n'),
                Some((_, 'r')) => result.push('\r'),
                Some((_, 't')) => result.push('\t'),
                Some((_, 'v')) => result.push('\x0B'),
                Some((_, '\\')) => result.push('\\'),
                Some((_, '\'')) => result.push('\''),
                Some((_, '"')) => result.push('"'),
                Some((_, '?')) => result.push('?'),
                Some((_, 'x')) => {
                    let hex = collect_hex_digits(&mut chars, 2);
                    if hex.is_empty() {
                        return Err(ParseError::InvalidHexEscape(
                            format!("\\x at position {i}: no hex digits")
                        ));
                    }
                    let val = u8::from_str_radix(&hex, 16)
                        .map_err(|_| ParseError::InvalidHexEscape(format!("\\x{hex}")))?;
                    result.push(val as char);
                }
                Some((_, 'u')) => {
                    let hex = collect_hex_digits(&mut chars, 4);
                    if hex.is_empty() {
                        return Err(ParseError::InvalidUnicodeEscape(
                            format!("\\u at position {i}: no hex digits")
                        ));
                    }
                    let codepoint = u32::from_str_radix(&hex, 16)
                        .map_err(|_| ParseError::InvalidUnicodeEscape(format!("\\u{hex}")))?;
                    result.push(char::from_u32(codepoint)
                        .ok_or_else(|| ParseError::InvalidUnicodeEscape(format!("\\u{hex}")))?);
                }
                Some((_, 'U')) => {
                    let hex = collect_hex_digits(&mut chars, 8);
                    if hex.is_empty() {
                        return Err(ParseError::InvalidUnicodeEscape(
                            format!("\\U at position {i}: no hex digits")
                        ));
                    }
                    let codepoint = u32::from_str_radix(&hex, 16)
                        .map_err(|_| ParseError::InvalidUnicodeEscape(format!("\\U{hex}")))?;
                    result.push(char::from_u32(codepoint)
                        .ok_or_else(|| ParseError::InvalidUnicodeEscape(format!("\\U{hex}")))?);
                }
                Some((_, c)) if c.is_ascii_digit() && c != '8' && c != '9' => {
                    let mut octal = String::from(c);
                    for _ in 0..2 {
                        let remaining = chars.as_str();
                        match remaining.chars().next() {
                            Some(ch) if ch.is_ascii_digit() && ch != '8' && ch != '9' => {
                                octal.push(ch);
                                chars.next();
                            }
                            _ => break,
                        }
                    }
                    let val = u8::from_str_radix(&octal, 8)
                        .map_err(|_| ParseError::InvalidOctalEscape(format!("\\{octal}")))?;
                    result.push(val as char);
                }
                Some((_, c)) => {
                    return Err(ParseError::InvalidEscapeSequence(format!("\\{c}")));
                }
            },
            Some((_, c)) => result.push(c),
        }
    }
}

fn collect_hex_digits(chars: &mut std::str::CharIndices, max: usize) -> String {
    let mut hex = String::new();
    for _ in 0..max {
        match chars.as_str().chars().next() {
            Some(ch) if ch.is_ascii_hexdigit() => {
                hex.push(ch);
                chars.next();
            }
            _ => break,
        }
    }
    hex
}
