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
//! A value may nest. [`BashVal`] is one of any depth, and [`QuotedNest`] and
//! [`LinkedArr`] are the two ways to flatten it into bash words.
//! [`BashCodec::words`] and [`BashCodec::rows`] read one back — a payload word
//! that is itself a literal is decoded with those.
//!
//! Inside: `quoting` is how bash spells one word, `parser` the four forms
//! above, `emit` the canonical form each is written in, `codec` the nesting,
//! and `error` what a refusal says.

pub(crate) mod codec;
pub(crate) mod emit;
pub(crate) mod error;
pub(crate) mod parser;
pub(crate) mod quoting;

pub use codec::{BashCodec, BashVal, LinkedArr, QuotedNest};
pub use emit::{emit_assoc, emit_indexed, emit_q_words, emit_scalar};
pub use error::ParseError;
pub use parser::{parse_assoc, parse_indexed, parse_q_words, parse_scalar};
