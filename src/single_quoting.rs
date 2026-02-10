use super::types::ParseError;

/// Parse the interior of a `'...'` single-quoted string.
/// Input starts AFTER the opening `'`. Returns `(literal_content, rest_after_closing_quote)`.
/// No escapes — everything is literal until the closing `'`.
pub fn parse_single_quoted_body(input: &str) -> Result<(String, &str), ParseError> {
    match input.find('\'') {
        Some(pos) => Ok((input[..pos].to_string(), &input[pos + 1..])),
        None => Err(ParseError::UnterminatedSingleQuote),
    }
}
