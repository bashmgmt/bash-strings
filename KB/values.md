# Values — bash's own quoted forms

`src/bash/value/` — `quoting.rs`, `parser.rs`, `emit.rs`, `codec.rs`, `error.rs`

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

One error type covers all of it: `ParseError` carries the message, the byte
offset, and the text around it. A codec failure is one of these too — a layout
that is not a literal, or a depth that wanted one word and found several.

### Where the forms stop

Bash's output is the whole of what is accepted, and two places in it are wider
than the type behind them. Both refuse rather than wrap:

- **An octal escape is a byte.** `$'\377'` is the widest bash prints — that is
  how a byte it cannot show crosses the wire, as ASCII, keeping the frame
  valid UTF-8. Three octal digits reach 511, so `$'\400'` and above are not
  bash's output and are an error.
- **A subscript is a machine integer.** `[0]`, `[5]`, up to `usize::MAX`. One
  too wide to be one was never printed by bash.

The parsers are the crate's only reader of text it did not write, so the
boundary is a `ParseError` at each of them and never a panic.

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
    fn parse(&self, words: &[String], schema: &Schema) -> Result<BashVal, ParseError>;

    fn emit_literal(&self, val: &BashVal) -> String;
    fn parse_literal(&self, input: &str, schema: &Schema) -> Result<BashVal, ParseError>;

    fn words(&self, input: &str) -> Result<Vec<String>, ParseError>;
    fn rows(&self, input: &str)  -> Result<Vec<Vec<String>>, ParseError>;
}
```

A bash array is flat, so nesting is encoded textually and the depth is carried
by a `Schema` alongside the data.

`emit` takes no `Schema`: the value's own depth decides the encoding, so there
is nothing to disagree with and nothing to fail. `parse` needs one, because
the text alone does not say how many layers of quoting to peel.

`words` and `rows` are `parse_literal` at `n_d(1)` and `n_d(2)` with the tree
already walked back down to strings — the two depths this crate asks for. A
`BashVal` and a `Schema` are what deeper or irregular shapes go through.

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
