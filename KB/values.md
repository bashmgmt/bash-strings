# Values — bash's own formats, decoded

`src/bash/value/`

Bash can already serialise its data. `${v@Q}` quotes a scalar so bash can read
it back; `${v[*]@Q}` does the same for each element of an array; `${v[*]@A}`
emits a complete `declare` statement, attributes included. This layer is the
Rust side of those forms, and nothing above it invents a format of its own.

## Four shapes, four parsers

```rust
pub fn parse_scalar(s: &str)  -> Result<String, ParseError>;                     // 'foo'
pub fn parse_q_words(s: &str) -> Result<Vec<String>, ParseError>;                // 'a' 'b' 'c'
pub fn parse_indexed(s: &str) -> Result<IndexMap<usize, String>, ParseError>;    // ([0]='a' [5]='b')
pub fn parse_assoc(s: &str)   -> Result<IndexMap<String, String>, ParseError>;   // (['k']='v')
```

with the exact inverses in `emit.rs`:

```rust
pub fn emit_scalar(s: &str) -> String;
pub fn emit_q_words(words: &[String]) -> String;
pub fn emit_indexed(m: &IndexMap<usize, String>) -> String;
pub fn emit_assoc(m: &IndexMap<String, String>) -> String;
```

The parsers are **strict**: they accept what bash emits, not everything bash
would accept. That is deliberate — the input always comes from bash, so
leniency would only hide a bug on our side. `ParseError` carries the input and
a byte offset:

```rust
pub struct ParseError { /* … */ }
impl ParseError {
    pub fn new(input: &str, at: usize, message: impl Into<String>) -> Self;
}
```

`IndexMap` rather than `HashMap` because bash's own iteration order is the
only order these have, and losing it would make output unstable for no gain.
Indexed arrays are `IndexMap<usize, _>` rather than `Vec` because bash arrays
are sparse: `a=([0]=x [5]=y)` is three elements short of a five-element `Vec`
and pretending otherwise loses the index.

### What a word can be

`${v@Q}` produces one of four forms per word, and `parse_q_words` handles all
of them plus their concatenation:

| form | example |
|---|---|
| single-quoted | `'hello world'` |
| ANSI-C quoted | `$'two\nlines'` — used whenever the value contains a newline |
| backslash-escaped | `\$` |
| bare | `plain` |

The ANSI-C case is why a message is always one line: bash never emits a raw
newline inside `@Q` output, so line-oriented framing is sound. See
[wire.md](wire.md#frames).

## Trees, and the two codecs

Bash arrays are flat. Anything nested has to be encoded textually, and there
are two reasonable ways to do it. Both are described by a `Schema` that
mirrors the value's depth:

```rust
pub enum BashVal { Str(String), Arr(Vec<BashVal>) }
pub enum Schema  { Scalar, Arr(Box<Schema>) }

impl Schema {
    pub fn n_d(n: usize) -> Self;   // n_d(2) == Arr(Arr(Scalar))
}

pub trait BashCodec {
    fn emit(&self, val: &BashVal, schema: &Schema) -> Result<Vec<String>, EmitError>;
    fn parse(&self, words: &[String], schema: &Schema) -> Result<BashVal, CodecParseError>;
    fn emit_literal(&self, val: &BashVal, schema: &Schema) -> Result<String, EmitError>;
    fn parse_literal(&self, input: &str, schema: &Schema) -> Result<BashVal, CodecParseError>;
}
```

**`QuotedNest`** makes each inner array one quoted word at the outer level:

```
[[a, b], [c]]   →   ("('a' 'b')" "('c')")
```

The receiver unquotes one layer per level. This is what the rig uses, because
bash reconstructs a level with `declare -a inner="$word"` — its own parser,
no `eval` — and Rust reconstructs it with `parse_literal`. Depth costs one
parse per level and nothing else, which is why a bashcap snapshot can carry
frames, state, vars and notes as structure rather than flattening them behind
sentinels.

**`LinkedArr`** prefixes each group with its width instead:

```
[[a, b], [c]]   →   (2 a b 1 c)
```

Denser, and it matches `glue-core/src/data/linked_arr.bash` on the ManageBash
side, which is the reason it exists. Nothing in the rig uses it.

## Where it is used

| caller | what for |
|---|---|
| `wire::Record::parse_message` | one message, `QuotedNest` at `n_d(1)` |
| `bashcap::entry` | each snapshot section, `n_d(1)` or `n_d(2)` |
| `codegen`, `run` | `emit_scalar` for every path and literal baked into the prelude |
| `resolve::cli` | `LinkedArr`, for the ManageBash resolver protocol |

## See also

- [wire.md](wire.md) — the message format built on `QuotedNest`
- `src/bash/value/codec/tests.rs` — round-trip proofs for both codecs
