//! Bash value primitives — parse and emit the right-hand side of a bash
//! assignment.
//!
//! A general utility about bash, standing on its own: nothing here knows about
//! the rig, the wire, or any tool. Three levels, each publicly reachable.
//!
//! # The shapes bash prints
//!
//! One call each, no codec and no schema. Every parser is strict, accepting
//! only what bash itself writes; every emitter produces the canonical
//! single-quoted form.
//!
//! | form | wire shape | type | written in bash by |
//! |---|---|---|---|
//! | scalar | `'foo'` | `String` | `${x@Q}` |
//! | q_words | `'a' 'b c'` | `Vec<String>` | `${x[*]@Q}` |
//! | array | `('a' 'b c')` | `Vec<String>` | `"(${x[*]@Q})"` |
//! | rows | `("'a' 'b'" "'c'")` | `Vec<Vec<String>>` | one level of nesting |
//! | indexed | `([0]='a' [5]='b')` | `IndexMap<usize, String>` | `${x[*]@A}`, `declare -a` |
//! | assoc | `(['k']='v')` | `IndexMap<String, String>` | `${x[*]@A}`, `declare -A` |
//!
//! ```
//! use mb_resolver::bash::value::{emit_array, parse_array, ParseError};
//!
//! let words = vec!["compiled".to_string(), "a file.rs".to_string()];
//!
//! assert_eq!(emit_array(&words), "('compiled' 'a file.rs')");
//! assert_eq!(parse_array("('compiled' 'a file.rs')")?, words);
//! # Ok::<(), ParseError>(())
//! ```
//!
//! # Any depth, either encoding
//!
//! Bash arrays are flat, so a value with structure is encoded textually.
//! [`BashVal`] is one of any depth and [`Schema`] is how deep to read it back
//! — which the text alone does not say. Two [`BashCodec`]s flatten one:
//!
//! - [`QuotedNest`] — each inner array is one bash-literal word at the outer
//!   level: `[[a,b],[c]]` → `["('a' 'b')", "('c')"]`. The receiver unquotes
//!   one layer per level. This is what `array` and `rows` above use.
//!
//! - [`LinkedArr`] — one flat word stream, each group prefixed by its width:
//!   `[[a,b],[c]]` → `[2, a, b, 1, c]`. The shape `glue-core`'s bash-side
//!   walker reads.
//!
//! ```
//! use mb_resolver::bash::value::{BashCodec, BashVal, LinkedArr, ParseError, Schema};
//!
//! let value = BashVal::Arr(vec![BashVal::row(["a", "b"]), BashVal::row(["c"])]);
//! let text = LinkedArr.emit_literal(&value);
//!
//! assert_eq!(text, "('2' 'a' 'b' '1' 'c')");
//! assert_eq!(LinkedArr.parse_literal(&text, &Schema::n_d(2))?, value);
//! # Ok::<(), ParseError>(())
//! ```
//!
//! Emitting takes the depth from the value and so cannot fail; parsing takes
//! it from the `Schema` the caller states.
//!
//! # A grammar over other syntax
//!
//! [`Cursor`] is the word lexer on its own — bash's quoting rules with the
//! stop characters left to the caller — and [`parse_with`] runs a grammar over
//! a whole input. See [`quoting`](self#a-grammar-over-other-syntax) for a
//! worked one.

mod codec;
mod emit;
mod error;
mod parser;
mod quoting;

pub use codec::{BashCodec, BashVal, LinkedArr, QuotedNest, Schema};

pub use emit::{emit_array, emit_assoc, emit_indexed, emit_q_words, emit_scalar};
pub use error::ParseError;
pub use parser::{parse_array, parse_assoc, parse_indexed, parse_scalar};
pub use quoting::{parse_with, Cursor};
