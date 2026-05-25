//! Three-tier bash value model:
//!
//! - `primitives/` — single-bash-word encode/parse + ParseError.
//! - `raw.rs` — `BashRaw`: bash's three native variable shapes
//!   (String/Array/AssocArray) with literal emit/parse and pack/unpack.
//! - `value.rs` — `BashVal` + `Schema`: recursive Arr/Str model used by codecs.
//! - `codec/` — `BashCodec` trait + `QuotedNest` + `LinkedArr` impls
//!   mapping `BashVal` ↔ `BashRaw::Array` given a `Schema`.

pub mod primitives;
pub mod raw;
#[allow(clippy::module_inception)] // submodule mirrors the module's central concept
pub mod value;
pub mod codec;

pub use primitives::{ParseError, encode_scalar, parse_one_word, parse_words};
pub use raw::{BashRaw, ConversionError};
pub use value::{BashVal, Schema};
pub use codec::{BashCodec, QuotedNest, LinkedArr, EmitError, CodecParseError};
