# bash-strings

Parse and emit the right-hand side of a bash assignment: `@Q` quoted words,
array literals, `declare -p` bodies — strict on input (only what bash itself
writes), canonical on output. Plus `Cursor`/`parse_with`, the word lexer over
bash's quoting rules for grammars of your own.

The reference is the crate doc (`cargo doc --open`); design notes live in
[`KB/values.md`](KB/values.md).
