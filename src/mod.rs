//! Bash value primitives — parse and emit the right-hand side of a bash
//! assignment.
//!
//! Four typed forms. Each has a strict parser, accepting only what bash
//! itself prints, and an emitter producing the canonical single-quoted form:
//!
//! | form    | wire shape          | type                       |
//! |---------|---------------------|----------------------------|
//! | scalar  | `'foo'`             | `String`                   |
//! | q_words | `'a' 'b' 'c'`       | `Vec<String>`              |
//! | indexed | `([0]='a' [5]='b')` | `IndexMap<usize, String>`  |
//! | assoc   | `(['k']='v')`       | `IndexMap<String, String>` |
//!
//! Recursive values are [`BashVal`], their depth is [`Schema`], and
//! [`QuotedNest`] and [`LinkedArr`] flatten one into bash words.

pub(crate) mod codec;
pub(crate) mod emit;
pub(crate) mod parser;

pub use codec::{BashCodec, BashVal, CodecParseError, LinkedArr, QuotedNest, Schema};
pub use emit::{emit_assoc, emit_indexed, emit_q_words, emit_scalar};
pub use parser::{parse_assoc, parse_indexed, parse_q_words, parse_scalar, ParseError};
