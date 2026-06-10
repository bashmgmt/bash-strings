//! Bash value primitives — parse and emit the RHS of bash assignments.
//!
//! Four typed value forms; each has a strict parser (accepting only bash's
//! canonical output) and an emitter (producing canonical single-quoted form):
//!
//! | Form    | Wire shape                | Type                             |
//! |---------|---------------------------|----------------------------------|
//! | scalar  | `'foo'`                   | `String`                         |
//! | q_words | `'a' 'b' 'c'`             | `Vec<String>`                    |
//! | indexed | `([0]='a' [5]='b')`       | `IndexMap<usize, String>`        |
//! | assoc   | `(['k']='v')`             | `IndexMap<String, String>`       |
//!
//! Recursive trees of strings live in [`BashVal`] + [`Schema`]; codecs
//! ([`QuotedNest`], [`LinkedArr`]) flatten them into `Vec<String>` of bash
//! words per a chosen layout.

pub mod tree;
pub mod codec;
pub mod parser;
pub mod emit;

pub use tree::{BashVal, Schema};
pub use codec::{BashCodec, QuotedNest, LinkedArr, EmitError, CodecParseError};
pub use parser::{parse_scalar, parse_q_words, parse_indexed, parse_assoc, ParseError};
pub use emit::{emit_scalar, emit_q_words, emit_indexed, emit_assoc};
