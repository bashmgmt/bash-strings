//! Tier 1 — single-word level: encoding/parsing of one bash literal word.
//! Used by Tier 2 (`raw.rs`) and the codecs.

pub mod types;
pub mod single_quoting;
pub mod ansi_c_quoting;
pub mod encode;
pub mod parse;

pub use types::ParseError;
pub use encode::encode_scalar;
pub use parse::{parse_one_word, parse_words};
