# Values — bash's own quoted forms

`src/bash/value/` — `parser.rs`, `emit.rs`, `codec.rs`

The Rust side of the format bash already has.

## Four forms

| form | shape | type | produced by bash as |
|---|---|---|---|
| scalar | `'foo'` | `String` | `"${x@Q}"` |
| q_words | `'a' 'b' 'c'` | `Vec<String>` | `"${arr[*]@Q}"` |
| indexed | `([0]='a' [5]='b')` | `IndexMap<usize, String>` | `declare -p`, `@A` |
| assoc | `(['k']='v')` | `IndexMap<String, String>` | `declare -p`, `@A` |

```rust
pub fn parse_scalar(s: &str)  -> Result<String, ParseError>;
pub fn parse_q_words(s: &str) -> Result<Vec<String>, ParseError>;
pub fn parse_indexed(s: &str) -> Result<IndexMap<usize, String>, ParseError>;
pub fn parse_assoc(s: &str)   -> Result<IndexMap<String, String>, ParseError>;

pub fn emit_scalar(s: &str)           -> String;
pub fn emit_q_words(words: &[String]) -> String;
pub fn emit_indexed(m: &IndexMap<usize, String>) -> String;
pub fn emit_assoc(m: &IndexMap<String, String>)  -> String;
```

Parsers accept bash's canonical output and nothing else; emitters produce
canonical single-quoted form. `$'…'` ANSI-C strings are accepted on input,
which is how a value containing a newline or a tab survives: bash emits that
form for them, so a delimiter inside a value never resembles the frame around
it.

Insertion order is preserved (`IndexMap`), because a snapshot read back should
list variables in the order bash reported them.

All four are the crate's contract with bash, and the round trips are the
strongest test of the parsers `bashcap` depends on, whether or not a given
direction currently has a caller.

## Trees and codecs

`codec.rs` holds the recursive value, its depth, and the two ways to flatten
one — one subject, since a codec is meaningless without them.

```rust
pub enum BashVal { Str(String), Arr(Vec<BashVal>) }
pub enum Schema  { Scalar, Arr(Box<Schema>) }

impl Schema { pub fn n_d(n: usize) -> Self; }   // n_d(2) == Arr(Arr(Scalar))

impl BashVal {
    pub fn row(words: impl IntoIterator<Item = impl Into<String>>) -> Self;
    pub fn words(self) -> Option<Vec<String>>;       // one-dimensional
    pub fn rows(self)  -> Option<Vec<Vec<String>>>;  // two-dimensional
}

pub trait BashCodec {
    fn emit(&self, val: &BashVal) -> Vec<String>;
    fn parse(&self, words: &[String], schema: &Schema) -> Result<BashVal, CodecParseError>;

    fn emit_literal(&self, val: &BashVal) -> String;
    fn parse_literal(&self, input: &str, schema: &Schema) -> Result<BashVal, CodecParseError>;

    fn words(&self, input: &str) -> Result<Vec<String>, CodecParseError>;
    fn rows(&self, input: &str)  -> Result<Vec<Vec<String>>, CodecParseError>;
}
```

A bash array is flat, so nesting is encoded textually and the depth is carried
by a `Schema` alongside the data.

`emit` takes no `Schema`: the value's own depth decides the encoding, so there
is nothing to disagree with and nothing to fail. `parse` needs one, because
the text alone does not say how many layers of quoting to peel.

`words` and `rows` are `parse_literal` at `n_d(1)` and `n_d(2)` with the tree
already walked back down to strings — the two depths every caller in the crate
asks for. A `BashVal` is worth holding for deeper or irregular shapes.

**`QuotedNest`** makes each inner array one quoted word at the outer level:

```
[[a, b], [c]]   →   ("('a' 'b')" "('c')")
```

Bash reconstructs a level with `declare -a inner="$word"`, its own parser, and
Rust with `words`. Depth costs one parse per level. This is what the rig uses.

**`LinkedArr`** prefixes each group with its width:

```
[[a, b], [c]]   →   (2 a b 1 c)
```

Denser, and it matches `glue-core/src/data/linked_arr.bash` on the ManageBash
side, which is its only consumer.

## See also

- [wire.md](wire.md#messages) — the one message format, built on `QuotedNest`
- [bashcap.md](bashcap.md#the-decoder) — the deepest use, at `n_d(2)`
