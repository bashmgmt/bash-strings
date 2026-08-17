# bash-strings

Bash has a serialisation format of its own. It is what `printf %q`, `${var@Q}`
and `declare -p` write, and what `declare -a arr="$word"` reads back. This
crate is the Rust side of that format.

```rust
let arr  = parse_array(r#"([0]="a b" [1]=$'x\ty')"#)?;   // ["a b", "x\ty"]
let word = emit_scalar("a b");                            // 'a b'
```

The parsers accept the forms bash writes and refuse everything else, so input
that came from somewhere other than a shell fails at the boundary with the
offset where it stopped. The emitters are canonical: one value produces one
word, and bash reads that word back unchanged.

Nested values go through `BashVal` and a `Schema`. The schema carries the
depth, which the text by itself does not — `(2 a b 1 c)` is two groups or
seven words depending on what was meant. Two codecs cover the two readings.
`QuotedNest` quotes one layer per level, so bash rebuilds a level at a time
with its own parser. `LinkedArr` prefixes each group with its width, and a
bash-side reader walks it with `shift` alone.

For syntax that is not a bash value but contains bash words, `Cursor` and
`parse_with` expose the word lexer with a stop set you choose. A grammar of
your own then gets every quoting form — single, double, `$'…'`, and adjacent
forms that concatenate into one word — without implementing any of them.

The only dependency is `indexmap`.

Reference: [`docs/values.md`](docs/values.md), or `cargo doc --open`.

Licensed under the MIT licence.
