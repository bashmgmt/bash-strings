use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnterminatedSingleQuote,
    UnterminatedAnsiCQuote,
    InvalidEscapeSequence(String),
    InvalidHexEscape(String),
    InvalidUnicodeEscape(String),
    InvalidOctalEscape(String),
    ExpectedPair(String),
    TrailingContent(String),
    InvalidFormat(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnterminatedSingleQuote => write!(f, "unterminated single quote"),
            Self::UnterminatedAnsiCQuote => write!(f, "unterminated $'...' quote"),
            Self::InvalidEscapeSequence(s) => write!(f, "invalid escape: {s}"),
            Self::InvalidHexEscape(s) => write!(f, "invalid hex escape: {s}"),
            Self::InvalidUnicodeEscape(s) => write!(f, "invalid unicode escape: {s}"),
            Self::InvalidOctalEscape(s) => write!(f, "invalid octal escape: {s}"),
            Self::ExpectedPair(s) => write!(f, "expected key-value pair: {s}"),
            Self::TrailingContent(s) => write!(f, "trailing content: {s}"),
            Self::InvalidFormat(s) => write!(f, "invalid format: {s}"),
        }
    }
}

impl std::error::Error for ParseError {}
