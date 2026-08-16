# Values — bash's own quoted forms

`src/bash/value/` — `quoting.rs`, `parser.rs`, `emit.rs`, `codec.rs`, `error.rs`

The Rust side of the format bash already has.

**This layer stands on its own.** It imports nothing else in the crate, and
three otherwise-disconnected callers use it: `bash/rig/` for the wire,
`bashcap/` for captured variables, and `resolve/` for `dependencies.list` and
the aspect grammar. Its surface is what parsing and formatting bash values
needs — not what any one caller happens to reach for.

Every module inside is private. The re-export list in `mod.rs` is the API, so
nothing is reachable only by reaching through, and no dependency of the
implementation appears in a signature.

## Three levels

```
the shapes bash prints      one call each, no codec, no schema
        │  built on
any depth, either encoding  BashVal + Schema + BashCodec
        │  built on
a grammar over other syntax Cursor + parse_with
```

Each is a public entry point. A helper with a sane default never closes off the
general form underneath it.

## Level 1 — the shapes bash prints

| form | shape | type | produced by bash as |
|---|---|---|---|
| scalar | `'foo'` | `String` | `"${x@Q}"` |
| q_words | `'a' 'b c'` | `Vec<String>` | `"${arr[*]@Q}"` |
| array | `('a' 'b c')` | `Vec<String>` | `"(${arr[*]@Q})"` |
| rows | `("'a' 'b'" "'c'")` | `Vec<Vec<String>>` | one level of nesting |
| indexed | `([0]='a' [5]='b')` | `IndexMap<usize, String>` | `declare -p`, `@A` |
| assoc | `(['k']='v')` | `IndexMap<String, String>` | `declare -p`, `@A` |

```rust
pub fn parse_scalar(text: &str)  -> Result<String, ParseError>;
pub fn parse_q_words(text: &str) -> Result<Vec<String>, ParseError>;
pub fn parse_array(text: &str)   -> Result<Vec<String>, ParseError>;
pub fn parse_rows(text: &str)    -> Result<Vec<Vec<String>>, ParseError>;
pub fn parse_indexed(text: &str) -> Result<IndexMap<usize, String>, ParseError>;
pub fn parse_assoc(text: &str)   -> Result<IndexMap<String, String>, ParseError>;

pub fn emit_scalar(text: &str)         -> String;
pub fn emit_q_words(words: &[String])  -> String;
pub fn emit_array(words: &[String])    -> String;
pub fn emit_rows(rows: &[Vec<String>]) -> String;
pub fn emit_indexed(m: &IndexMap<usize, String>) -> String;
pub fn emit_assoc(m: &IndexMap<String, String>)  -> String;
```

**An array literal is the shape everything else travels in.** A wire frame is
one, an answer is one, and each section of a bashcap message is one. It is
spelled once on each side — `emit_array` puts the parentheses on, `inside`
takes them off — and `parse_array` takes no codec, because at one dimension
`QuotedNest` and `LinkedArr` write the same text. That equality is asserted,
not assumed.

Parsers accept bash's canonical output and nothing else; emitters produce
canonical single-quoted form. `$'…'` ANSI-C strings are accepted on input,
which is how a value containing a newline or a tab survives: bash emits that
form for them, so a delimiter inside a value never resembles the frame around
it.

Insertion order is preserved (`IndexMap`), because a snapshot read back should
list variables in the order bash reported them.

One error type covers all of it: `ParseError` carries the message, the byte
offset it stopped at, and the text around that offset, widened to a character
boundary. A codec failure is one of these too — a layout that is not a literal,
or a depth that wanted one word and found several.

### Where the forms stop

Bash's output is the whole of what is accepted, and two places in it are wider
than the type behind them. Both refuse rather than wrap:

- **An octal escape is a byte.** `$'\377'` is the widest bash prints — that is
  how a byte it cannot show crosses the wire, as ASCII, keeping the frame
  valid UTF-8. Three octal digits reach 511, so `$'\400'` and above are not
  bash's output and are an error. An escape takes at most three digits, so a
  fourth is text.
- **A subscript is a machine integer.** `[0]`, `[5]`, up to `usize::MAX`. One
  too wide to be one was never printed by bash.

The parsers are the crate's only reader of text it did not write, so the
boundary is a `ParseError` at each of them and never a panic.

## Level 2 — trees and codecs

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

    fn rows(&self, input: &str) -> Result<Vec<Vec<String>>, ParseError>;
}
```

A bash array is flat, so nesting is encoded textually and the depth is carried
by a `Schema` alongside the data. This is how a value with structure is
transported through a shape that has none.

`emit` takes no `Schema`: the value's own depth decides the encoding, so there
is nothing to disagree with and nothing to fail. `parse` needs one, because
the text alone does not say how many layers of quoting to peel — and reading
the same text at `n_d(2)` where it was written at `n_d(3)` yields the outer
level with its inner literals intact, which is a peel rather than an error.

`rows` is `parse_literal` at `n_d(2)` with the tree walked back down to
strings. One dimension has no such method: `parse_array` covers it for both
codecs. `parse_rows` is `QuotedNest::rows` under a name, and `LinkedArr.rows`
is how a caller says otherwise.

**`QuotedNest`** makes each inner array one quoted word at the outer level:

```
[[a, b], [c]]   →   ("('a' 'b')" "('c')")
```

Bash reconstructs a level with `declare -a inner="$word"`, its own parser, and
Rust with `parse_array`. Depth costs one parse per level. This is what the rig
uses.

**`LinkedArr`** prefixes each group with its width:

```
[[a, b], [c]]   →   (2 a b 1 c)
```

Denser, and it matches `glue-core/src/data/linked_arr.bash` on the ManageBash
side. `resolve/cli/resolver/resolver_util.rs` emits it.

## Level 3 — a grammar over other syntax

`quoting.rs` knows how bash spells **one word** and nothing above that. Where a
word *ends* belongs to the caller, so a grammar over unrelated syntax passes
its own stop characters and gets every quoting form for free.

```rust
pub struct Cursor<'a>;

impl<'a> Cursor<'a> {
    pub fn word(&mut self, stops: &[char]) -> Result<String, ParseError>;
    pub fn ws0(&mut self);
    pub fn eat(&mut self, text: &str) -> bool;
    pub fn lit(&mut self, text: &str) -> Result<(), ParseError>;
    pub fn take_while(&mut self, accept: impl Fn(char) -> bool) -> &'a str;
    pub fn peek(&self) -> Option<char>;
    pub fn starts_with(&self, text: &str) -> bool;
    pub fn at_end(&self) -> bool;
    pub fn at(&self) -> usize;
    pub fn rest(&self) -> &'a str;
    pub fn fail(&self, message: impl Into<String>) -> ParseError;
}

pub fn parse_with<T>(
    input: &str,
    grammar: impl FnOnce(&mut Cursor<'_>) -> Result<T, ParseError>,
) -> Result<T, ParseError>;
```

`parse_with` is also what enforces that a grammar consumed the whole input, so
none can quietly match a prefix. A refusal carries the offset it reached,
constructed where the parse stopped.

`resolve/parsing/aspect.rs` is the worked instance: `Name(key=value,
key2='two words')` is not a bash value, but each parameter is a bash word, so
it declares two stop sets and writes twenty lines.

A word is any of the quoting forms, and adjacent ones concatenate — `a"b"c'd'`
is one word, `abcd`:

| | |
|---|---|
| `'…'` | single — no escapes at all |
| `"…"` | double — `\$ \" \\ \`` and a line continuation |
| `$'…'` | ANSI-C — the full escape set, including `\nnn` and `\uXXXX` |
| `$"…"` | locale, read as double |
| bare | up to a stop character, `\` escaping the next one |

## No parser dependency in the surface

The layer has no third-party parsing dependency. What one provided was literal
matching, a character-class take, and an error model whose backtrack/cut
distinction nothing here discriminated on — every choice is a `starts_with`,
and the one site that could tell the two apart treated them alike. `Cursor` is
that, spelled directly.

## See also

- `bash-interop/KB/wire.md#messages` — the one message format, built on array literals
- `bashcap/KB/bashcap.md#the-decoder` — the deepest use, at `n_d(2)`
- `bash-interop/KB/scoping.md` — where the bash half of all this binds its names
