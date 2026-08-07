//! What a refusal says, and the two ways a parser here produces one.

use std::fmt;

use winnow::error::{ContextError, ErrMode};

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

    pub(super) fn from_winnow<E: fmt::Display>(input: &str, at: usize, err: E) -> Self {
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

/// A refusal that ends the parse. Once a quoted form's opener is read there is
/// only one way to finish it, so there is nothing to back off to — unlike a
/// bare segment, which `word` retries.
pub(super) fn cut() -> ErrMode<ContextError> {
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
