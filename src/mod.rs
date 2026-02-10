pub mod types;
pub mod single_quoting;
pub mod ansi_c_quoting;
pub mod parse;
pub mod encode;

pub use types::{BashType, BashValue, ParseError};
pub use parse::{parse_typed_value, parse_one_word};
pub use encode::encode_scalar;
