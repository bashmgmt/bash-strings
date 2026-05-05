//! Tier 3 — recursive value model for trees of strings.
//!
//! `BashVal` is the recursive structure we maintain in Rust; bash itself
//! has no native nesting. Codecs (`bash/value/codec/`) flatten `BashVal`
//! into `BashRaw::Array` according to a chosen convention and a `Schema`.
//!
//! Dicts (assocs) live only at `BashRaw`; bash doesn't recursively nest
//! them, so `BashVal` deliberately doesn't have a Dict variant.

#[derive(Debug, Clone, PartialEq)]
pub enum BashVal {
    Str(String),
    Arr(Vec<BashVal>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Schema {
    Scalar,
    Arr(Box<Schema>),
}

impl Schema {
    pub fn one_d() -> Self { Schema::Arr(Box::new(Schema::Scalar)) }
    pub fn two_d() -> Self { Schema::Arr(Box::new(Self::one_d())) }
    pub fn n_d(n: usize) -> Self {
        let mut s = Schema::Scalar;
        for _ in 0..n { s = Schema::Arr(Box::new(s)); }
        s
    }
}

// ── Builders ─────────────────────────────────────────────

impl BashVal {
    pub fn s(s: impl Into<String>) -> Self { Self::Str(s.into()) }
    pub fn arr() -> Self { Self::Arr(Vec::new()) }

    /// Build an `Arr` of `Str`s from any iterable of stringy items.
    pub fn cmd<I, S>(args: I) -> Self
        where I: IntoIterator<Item = S>, S: Into<String>
    {
        Self::Arr(args.into_iter().map(|s| Self::s(s)).collect())
    }

    pub fn push(mut self, v: BashVal) -> Self {
        match &mut self {
            Self::Arr(es) => es.push(v),
            Self::Str(_)  => panic!("BashVal::push called on Str"),
        }
        self
    }
}
