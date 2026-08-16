# bash-strings — working in this crate

The bash value<->string codec, whole: strict parsers for what bash itself
writes, canonical single-quoted emitters, `BashVal`/`Schema` for nested
values, `Cursor`/`parse_with` for grammars over bash quoting. Depends on
`indexmap` and nothing else; `bash-interop` and `mb_resolver` build on it.

`cargo test` and `cargo clippy --all-targets -- -D warnings` (silent) are the
gate; tests are inline with the codec they cover. Design notes: `docs/values.md`.
Style follows the parent workspace's CLAUDE.md: comments carry technical fact,
never narrative; non-defensive; prefer deleting.
