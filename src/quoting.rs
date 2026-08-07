//! How bash spells one word, and what it takes to build a parser on that.
//!
//! A word is any of the quoting forms, and adjacent ones concatenate:
//!
//! | | |
//! |---|---|
//! | `'…'` | single — no escapes at all |
//! | `"…"` | double — `\$ \" \\ \`` and a line continuation |
//! | `$'…'` | ANSI-C — the full escape set, including `\nnn` and `\uXXXX` |
//! | `$"…"` | locale, read as double |
//! | bare | up to a stop character, `\` escaping the next one |
//!
//! `a"b"c'd'$'e'` is one word, `abcde`. Where a word ends is the caller's:
//! [`word`] takes the stop characters, so a grammar over a different syntax
//! passes its own and builds on [`run`] and [`ws0`] for the rest.

use winnow::Parser;
use winnow::error::{ContextError, ErrMode};
use winnow::token::take_until;

use super::error::{cut, ParseError};

/// Run a parser over the whole of `input`. Anything it leaves behind is an
/// error, so a grammar cannot quietly match a prefix.
pub(crate) fn run<T>(
    mut p: impl FnMut(&mut &str) -> winnow::ModalResult<T>,
    input: &str,
) -> Result<T, ParseError> {
    let mut cursor = input;
    match p(&mut cursor) {
        Ok(v) if cursor.is_empty() => Ok(v),
        Ok(_) => Err(ParseError::new(input, input.len() - cursor.len(), "trailing input")),
        Err(e) => Err(ParseError::from_winnow(input, input.len() - cursor.len(), e)),
    }
}

pub(crate) fn ws0(input: &mut &str) {
    while let Some(c) = input.chars().next() {
        if c == ' ' || c == '\t' || c == '\n' { *input = &input[c.len_utf8()..]; } else { break; }
    }
}

/// One word: a first segment, then every adjacent one that still belongs to
/// it. The first must be there; a following one that will not read ends the
/// word rather than failing it.
pub(crate) fn word(stops: &[char], input: &mut &str) -> winnow::ModalResult<String> {
    let mut out = first_segment(stops, input)?;
    loop {
        let snap = *input;
        match next_segment(stops, input) {
            Ok(Some(s)) => out.push_str(&s),
            Ok(None) | Err(_) => { *input = snap; break; }
        }
    }
    Ok(out)
}

fn first_segment(stops: &[char], input: &mut &str) -> winnow::ModalResult<String> {
    if input.starts_with("$'") { return single_ansi_c(input); }
    if input.starts_with('\'') { return single_quoted(input); }
    if input.starts_with('"')  { return double_quoted(input); }
    if input.starts_with("$\"") { *input = &input[1..]; return double_quoted(input); }
    bare(stops, input)
}

fn next_segment(stops: &[char], input: &mut &str) -> winnow::ModalResult<Option<String>> {
    if input.starts_with("$'") { return single_ansi_c(input).map(Some); }
    if input.starts_with('\'') { return single_quoted(input).map(Some); }
    if input.starts_with('"')  { return double_quoted(input).map(Some); }
    if input.starts_with("$\"") { *input = &input[1..]; return double_quoted(input).map(Some); }
    match input.chars().next() {
        Some(c) if !stops.contains(&c) && c != '\\' => bare(stops, input).map(Some),
        Some('\\') => bare(stops, input).map(Some),
        _ => Ok(None),
    }
}

fn single_quoted(input: &mut &str) -> winnow::ModalResult<String> {
    "'".parse_next(input)?;
    let body = take_until(0.., "'").parse_next(input)?;
    "'".parse_next(input)?;
    Ok(body.to_string())
}

fn double_quoted(input: &mut &str) -> winnow::ModalResult<String> {
    "\"".parse_next(input)?;
    let mut out = String::new();
    loop {
        // Reading the character is what says there is one, so the body below
        // never has to ask again.
        let Some(c) = input.chars().next() else { return Err(cut()) };

        if c == '"' { *input = &input[1..]; return Ok(out); }
        if c == '\\' {
            let rest = &input[1..];
            match rest.chars().next() {
                Some(c @ ('$' | '"' | '\\' | '`')) => { out.push(c); *input = &rest[c.len_utf8()..]; }
                Some('\n') => { *input = &rest[1..]; }
                Some(c) => { out.push('\\'); out.push(c); *input = &rest[c.len_utf8()..]; }
                None => return Err(cut()),
            }
            continue;
        }
        out.push(c);
        *input = &input[c.len_utf8()..];
    }
}

fn single_ansi_c(input: &mut &str) -> winnow::ModalResult<String> {
    "$'".parse_next(input)?;
    let mut out = String::new();
    loop {
        let Some(c) = input.chars().next() else { return Err(cut()) };

        if c == '\'' { *input = &input[1..]; return Ok(out); }
        if c == '\\' {
            let rest = &input[1..];
            let escaped = rest.chars().next().ok_or_else(cut)?;

            *input = &rest[escaped.len_utf8()..];
            decode_ansi_c_escape(escaped, input, &mut out)?;
            continue;
        }
        out.push(c);
        *input = &input[c.len_utf8()..];
    }
}

fn decode_ansi_c_escape(c: char, input: &mut &str, out: &mut String) -> winnow::ModalResult<()> {
    match c {
        'a' => out.push('\x07'),
        'b' => out.push('\x08'),
        'e' | 'E' => out.push('\x1B'),
        'f' => out.push('\x0C'),
        'n' => out.push('\n'),
        'r' => out.push('\r'),
        't' => out.push('\t'),
        'v' => out.push('\x0B'),
        '\\' => out.push('\\'),
        '\'' => out.push('\''),
        '"' => out.push('"'),
        '?' => out.push('?'),
        'c' => {
            let cc = input.chars().next().ok_or_else(cut)?;
            *input = &input[cc.len_utf8()..];
            out.push(((cc as u32) & 0x1F) as u8 as char);
        }
        'x' => push_radix(input, out, 2, 16)?,
        'u' => push_unicode(input, out, 4)?,
        'U' => push_unicode(input, out, 8)?,
        d if d.is_ascii_digit() && d != '8' && d != '9' => {
            let mut oct = String::from(d);
            for _ in 0..2 {
                match input.chars().next() {
                    Some(dd) if dd.is_ascii_digit() && dd != '8' && dd != '9' => {
                        oct.push(dd);
                        *input = &input[dd.len_utf8()..];
                    }
                    _ => break,
                }
            }
            // Three octal digits reach 511. Bash prints no escape above
            // `\377`, so a wider one is not its output.
            let byte = u8::from_str_radix(&oct, 8).map_err(|_| cut())?;

            out.push(byte as char);
        }
        _ => { out.push('\\'); out.push(c); }
    }
    Ok(())
}

fn push_radix(input: &mut &str, out: &mut String, max: usize, radix: u32) -> winnow::ModalResult<()> {
    let h = take_hex(input, max);
    if h.is_empty() { return Err(cut()); }
    let v = u8::from_str_radix(&h, radix).map_err(|_| cut())?;
    out.push(v as char);
    Ok(())
}

fn push_unicode(input: &mut &str, out: &mut String, max: usize) -> winnow::ModalResult<()> {
    let h = take_hex(input, max);
    if h.is_empty() { return Err(cut()); }
    let v = u32::from_str_radix(&h, 16).map_err(|_| cut())?;
    out.push(char::from_u32(v).ok_or_else(cut)?);
    Ok(())
}

fn take_hex(input: &mut &str, max: usize) -> String {
    let mut out = String::new();
    for _ in 0..max {
        match input.chars().next() {
            Some(c) if c.is_ascii_hexdigit() => { out.push(c); *input = &input[c.len_utf8()..]; }
            _ => break,
        }
    }
    out
}

/// Unquoted text up to a stop character. A backslash takes the next character
/// whatever it is, and a backslash-newline is a line continuation. Empty is a
/// backtrack, not a failure: it is how `word` learns a segment ended.
fn bare(stops: &[char], input: &mut &str) -> winnow::ModalResult<String> {
    let mut out = String::new();
    while let Some(c) = input.chars().next() {
        if c == '\'' || c == '"' || c == '$' { break; }
        if c == '\\' {
            let rest = &input[1..];
            match rest.chars().next() {
                Some('\n') => { *input = &rest[1..]; continue; }
                Some(e) => { out.push(e); *input = &rest[e.len_utf8()..]; continue; }
                None => break,
            }
        }
        if stops.contains(&c) { break; }
        out.push(c);
        *input = &input[c.len_utf8()..];
    }
    if out.is_empty() { return Err(ErrMode::Backtrack(ContextError::new())); }
    Ok(out)
}
