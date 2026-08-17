//! Bash-value emitters — single canonical form (single-quoted, with
//! `$'…'` fallback for non-printable bytes). One emission primitive
//! ([`emit_scalar`]) underlies every typed emitter, and one
//! [`literal`] puts the parentheses on.

use indexmap::IndexMap;

pub fn emit_scalar(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    if needs_ansi_c(s) {
        return ansi_c(s);
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

pub fn emit_q_words(words: &[String]) -> String {
    words
        .iter()
        .map(|word| emit_scalar(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One bash array literal: `["a", "b c"]` → `('a' 'b c')`. The shape a
/// message, an answer and every captured column travel as, and the inverse of
/// [`parse_array`](super::parse_array).
pub fn emit_array(words: &[String]) -> String {
    literal(words.iter().map(|word| emit_scalar(word)))
}

pub fn emit_indexed(m: &IndexMap<usize, String>) -> String {
    literal(
        m.iter()
            .map(|(key, value)| format!("[{key}]={}", emit_scalar(value))),
    )
}

pub fn emit_assoc(m: &IndexMap<String, String>) -> String {
    literal(m.iter().map(|(key, value)| {
        format!(
            "[{}]={}",
            emit_scalar(key),
            emit_scalar(value)
        )
    }))
}

fn literal(pairs: impl IntoIterator<Item = String>) -> String {
    format!(
        "({})",
        pairs.into_iter().collect::<Vec<_>>().join(" ")
    )
}

fn needs_ansi_c(s: &str) -> bool {
    s.bytes().any(|b| b < 0x20 || b == 0x7f)
}

fn ansi_c(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    out.push_str("$'");
    for c in s.chars() {
        match c {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x07' => out.push_str("\\a"),
            '\x08' => out.push_str("\\b"),
            '\x1B' => out.push_str("\\E"),
            '\x0C' => out.push_str("\\f"),
            '\x0B' => out.push_str("\\v"),
            c if (c as u32) < 0x20 || c == '\x7f' => {
                out.push_str(&format!("\\{:03o}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::super::parser::{parse_assoc, parse_indexed, parse_q_words, parse_scalar};
    use super::*;

    fn ix<I: IntoIterator<Item = (usize, &'static str)>>(it: I) -> IndexMap<usize, String> {
        it.into_iter().map(|(k, v)| (k, v.to_string())).collect()
    }
    fn ax<I: IntoIterator<Item = (&'static str, &'static str)>>(it: I) -> IndexMap<String, String> {
        it.into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn scalar_forms() {
        assert_eq!(emit_scalar(""), "''");
        assert_eq!(emit_scalar("hello"), "'hello'");
        assert_eq!(emit_scalar("it's"), "'it'\\''s'");
        assert_eq!(emit_scalar("a\nb"), "$'a\\nb'");
        assert_eq!(emit_scalar("a\tb"), "$'a\\tb'");
    }

    #[test]
    fn q_words_emit() {
        assert_eq!(emit_q_words(&[]), "");
        assert_eq!(
            emit_q_words(&["a".into(), "b c".into()]),
            "'a' 'b c'"
        );
    }

    #[test]
    fn indexed_emit() {
        assert_eq!(emit_indexed(&ix([])), "()");
        assert_eq!(
            emit_indexed(&ix([(0, "a"), (5, "b c")])),
            "([0]='a' [5]='b c')"
        );
    }

    #[test]
    fn assoc_emit() {
        assert_eq!(emit_assoc(&ax([])), "()");
        assert_eq!(
            emit_assoc(&ax([("k", "v"), ("k 2", "v 2")])),
            "(['k']='v' ['k 2']='v 2')"
        );
    }

    #[test]
    fn roundtrip_scalar() {
        for s in ["", "simple", "it's", "a\nb", "a\tb", "c\x01d", "café"] {
            assert_eq!(
                parse_scalar(&emit_scalar(s)).unwrap(),
                s
            );
        }
    }

    #[test]
    fn roundtrip_q_words() {
        let v = vec![
            "a".to_string(),
            "b c".to_string(),
            "d\ne".to_string(),
            "".to_string(),
        ];
        assert_eq!(
            parse_q_words(&emit_q_words(&v)).unwrap(),
            v
        );
    }

    #[test]
    fn roundtrip_indexed() {
        let m = ix([(0, "a"), (2, "d\ne"), (5, "b c")]);
        assert_eq!(
            parse_indexed(&emit_indexed(&m)).unwrap(),
            m
        );
    }

    #[test]
    fn roundtrip_assoc() {
        let m = ax([("foo", "1"), ("k 2", "v\n2")]);
        assert_eq!(parse_assoc(&emit_assoc(&m)).unwrap(), m);
    }
}
