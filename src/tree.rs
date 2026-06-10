//! Recursive tree model and shape.
//!
//! `BashVal` = tree of strings; `Schema` = depth descriptor.
//! Codecs flatten `BashVal` into `Vec<String>` according to a `Schema`.

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
    pub fn n_d(n: usize) -> Self {
        let mut s = Schema::Scalar;
        for _ in 0..n { s = Schema::Arr(Box::new(s)); }
        s
    }
}

impl BashVal {
    pub fn arr() -> Self { Self::Arr(Vec::new()) }

    pub fn cmd<I, S>(args: I) -> Self
        where I: IntoIterator<Item = S>, S: Into<String>
    {
        args.into_iter().map(|s| s.into()).collect::<Vec<String>>().into()
    }

    pub fn push(mut self, v: impl Into<BashVal>) -> Self {
        match &mut self {
            Self::Arr(es) => es.push(v.into()),
            Self::Str(_)  => panic!("BashVal::push on Str"),
        }
        self
    }
}

impl From<&str>   for BashVal { fn from(s: &str)   -> Self { Self::Str(s.into()) } }
impl From<String> for BashVal { fn from(s: String) -> Self { Self::Str(s) } }

impl<T: Into<BashVal>> From<Vec<T>> for BashVal {
    fn from(v: Vec<T>) -> Self { Self::Arr(v.into_iter().map(Into::into).collect()) }
}

impl<T: Into<BashVal>> FromIterator<T> for BashVal {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::Arr(iter.into_iter().map(Into::into).collect())
    }
}
