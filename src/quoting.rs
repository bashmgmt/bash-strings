//! How bash spells one word, and the cursor a grammar builds on.
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
//! `a"b"c'd'$'e'` is one word, `abcde`. Where a word *ends* belongs to the
//! caller: [`Cursor::word`] takes the stop characters, so a grammar over an
//! entirely different syntax passes its own and gets bash quoting for free.
//!
//! ```
//! use bash_strings::{parse_with, ParseError};
//!
//! // `key: value, key: value` — not a bash value, but the words are bash's.
//! const STOPS: &[char] = &[':', ',', ' '];
//!
//! let pairs = parse_with("a: 'one two', b: $'x\\ty'", |c| {
//!     let mut out = Vec::new();
//!     loop {
//!         c.ws0();
//!         let key = c.word(STOPS)?;
//!         c.lit(":")?;
//!         c.ws0();
//!         out.push((key, c.word(STOPS)?));
//!         c.ws0();
//!         if !c.eat(",") {
//!             return Ok(out);
//!         }
//!     }
//! })?;
//!
//! assert_eq!(pairs, [("a".to_string(), "one two".to_string()),
//!                    ("b".to_string(), "x\ty".to_string())]);
//! # Ok::<(), ParseError>(())
//! ```

use super::error::ParseError;

/// A position in some text, and the bash word grammar over it.
///
/// Built by [`parse_with`], which is also what checks that a grammar consumed
/// the whole input.
pub struct Cursor<'a> {
    input: &'a str,
    rest: &'a str,
}

/// Run a grammar over the whole of `input`. Anything it leaves behind is an
/// error, so a grammar cannot quietly match a prefix.
pub fn parse_with<T>(
    input: &str,
    grammar: impl FnOnce(&mut Cursor<'_>) -> Result<T, ParseError>,
) -> Result<T, ParseError> {
    let mut cursor = Cursor { input, rest: input };
    let parsed = grammar(&mut cursor)?;

    match cursor.at_end() {
        true => Ok(parsed),
        false => Err(cursor.fail("trailing input")),
    }
}

impl<'a> Cursor<'a> {
    /// The byte offset reached so far.
    pub fn at(&self) -> usize {
        self.input.len() - self.rest.len()
    }

    /// What has not been read yet.
    pub fn rest(&self) -> &'a str {
        self.rest
    }

    pub fn at_end(&self) -> bool {
        self.rest.is_empty()
    }

    pub fn peek(&self) -> Option<char> {
        self.rest.chars().next()
    }

    pub fn starts_with(&self, text: &str) -> bool {
        self.rest.starts_with(text)
    }

    /// A refusal at the current position, carrying the text around it.
    pub fn fail(&self, message: impl Into<String>) -> ParseError {
        ParseError::new(self.input, self.at(), message)
    }

    /// Consume `text` if it is next, and say whether it was.
    pub fn eat(&mut self, text: &str) -> bool {
        match self.rest.strip_prefix(text) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    /// Consume `text`, which must be next.
    pub fn lit(&mut self, text: &str) -> Result<(), ParseError> {
        match self.eat(text) {
            true => Ok(()),
            false => Err(self.fail(format!("expected {text:?}"))),
        }
    }

    /// Every leading character the predicate accepts, possibly none.
    pub fn take_while(&mut self, accept: impl Fn(char) -> bool) -> &'a str {
        let end = self
            .rest
            .find(|c: char| !accept(c))
            .unwrap_or(self.rest.len());
        let (taken, rest) = self.rest.split_at(end);

        self.rest = rest;
        taken
    }

    /// Spaces, tabs and newlines, possibly none.
    pub fn ws0(&mut self) {
        self.take_while(|c| c == ' ' || c == '\t' || c == '\n');
    }

    /// One word: a first segment, then every adjacent one that still belongs
    /// to it. The first must be there; a following one that will not read ends
    /// the word rather than failing it.
    pub fn word(&mut self, stops: &[char]) -> Result<String, ParseError> {
        let mut out = match self.quoted()? {
            Some(text) => text,
            None => self.bare(stops)?,
        };

        loop {
            let snapshot = self.rest;
            match self.segment(stops) {
                Ok(Some(text)) => out.push_str(&text),
                Ok(None) | Err(_) => {
                    self.rest = snapshot;
                    break;
                }
            }
        }
        Ok(out)
    }

    fn advance(&mut self, c: char) {
        self.rest = &self.rest[c.len_utf8()..];
    }

    /// A segment after the first: a quoting form, more bare text, or the end
    /// of the word.
    fn segment(&mut self, stops: &[char]) -> Result<Option<String>, ParseError> {
        if let Some(text) = self.quoted()? {
            return Ok(Some(text));
        }

        match self.peek() {
            Some('\\') => self.bare(stops).map(Some),
            Some(c) if !stops.contains(&c) => self.bare(stops).map(Some),
            _ => Ok(None),
        }
    }

    /// One of the quoting forms, or `None` where the cursor is not at one.
    fn quoted(&mut self) -> Result<Option<String>, ParseError> {
        if self.eat("$'") {
            return self.ansi_c().map(Some);
        }
        if self.starts_with("'") {
            return self.single().map(Some);
        }
        if self.starts_with("\"") {
            return self.double().map(Some);
        }
        if self.starts_with("$\"") {
            self.advance('$');
            return self.double().map(Some);
        }
        Ok(None)
    }

    fn single(&mut self) -> Result<String, ParseError> {
        self.lit("'")?;

        let body = self.take_while(|c| c != '\'').to_string();
        if !self.eat("'") {
            return Err(self.fail("unterminated ' quote"));
        }
        Ok(body)
    }

    fn double(&mut self) -> Result<String, ParseError> {
        self.lit("\"")?;

        let mut out = String::new();
        loop {
            // Reading the character is what says there is one, so the body
            // below never has to ask again.
            let Some(c) = self.peek() else {
                return Err(self.fail("unterminated \" quote"));
            };

            if c == '"' {
                self.advance(c);
                return Ok(out);
            }
            if c == '\\' {
                self.advance(c);
                match self.peek() {
                    Some(escaped @ ('$' | '"' | '\\' | '`')) => {
                        out.push(escaped);
                        self.advance(escaped);
                    }
                    Some('\n') => self.advance('\n'),
                    Some(escaped) => {
                        out.push('\\');
                        out.push(escaped);
                        self.advance(escaped);
                    }
                    None => return Err(self.fail("a backslash at the end of the input")),
                }
                continue;
            }
            out.push(c);
            self.advance(c);
        }
    }

    fn ansi_c(&mut self) -> Result<String, ParseError> {
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.fail("unterminated $' quote"));
            };

            if c == '\'' {
                self.advance(c);
                return Ok(out);
            }
            if c == '\\' {
                self.advance(c);
                let Some(escaped) = self.peek() else {
                    return Err(self.fail("a backslash at the end of the input"));
                };

                self.advance(escaped);
                self.escape(escaped, &mut out)?;
                continue;
            }
            out.push(c);
            self.advance(c);
        }
    }

    fn escape(&mut self, c: char, out: &mut String) -> Result<(), ParseError> {
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
                let control = self
                    .peek()
                    .ok_or_else(|| self.fail(r"\c with nothing after it"))?;

                self.advance(control);
                out.push(((control as u32) & 0x1F) as u8 as char);
            }
            'x' => {
                let digits = self.hex(2);
                let byte = u8::from_str_radix(&digits, 16).map_err(|_| self.fail(r"\x wants one or two hex digits"))?;

                out.push(byte as char);
            }
            'u' => self.unicode(4, out)?,
            'U' => self.unicode(8, out)?,
            first if is_octal(first) => {
                let mut octal = String::from(first);
                for _ in 0..2 {
                    match self.peek() {
                        Some(digit) if is_octal(digit) => {
                            octal.push(digit);
                            self.advance(digit);
                        }
                        _ => break,
                    }
                }

                // Three octal digits reach 511. Bash prints no escape above
                // `\377`, so a wider one is not its output.
                let byte = u8::from_str_radix(&octal, 8).map_err(|_| {
                    self.fail(format!(
                        r"octal escape \{octal} is above \377"
                    ))
                })?;

                out.push(byte as char);
            }
            other => {
                out.push('\\');
                out.push(other);
            }
        }
        Ok(())
    }

    fn unicode(&mut self, width: usize, out: &mut String) -> Result<(), ParseError> {
        let digits = self.hex(width);
        let point = u32::from_str_radix(&digits, 16).map_err(|_| self.fail("a unicode escape wants hex digits"))?;

        out.push(char::from_u32(point).ok_or_else(|| self.fail("not a Unicode scalar value"))?);
        Ok(())
    }

    fn hex(&mut self, width: usize) -> String {
        let mut out = String::new();
        for _ in 0..width {
            match self.peek() {
                Some(c) if c.is_ascii_hexdigit() => {
                    out.push(c);
                    self.advance(c);
                }
                _ => break,
            }
        }
        out
    }

    /// Unquoted text up to a stop character. A backslash takes the next
    /// character whatever it is, and a backslash-newline is a line
    /// continuation. Empty is how [`word`](Cursor::word) learns a segment
    /// ended, so it is an error the caller catches rather than a value.
    fn bare(&mut self, stops: &[char]) -> Result<String, ParseError> {
        let mut out = String::new();

        while let Some(c) = self.peek() {
            if c == '\'' || c == '"' || c == '$' {
                break;
            }
            if c == '\\' {
                let after = &self.rest[1..];
                match after.chars().next() {
                    Some('\n') => self.rest = &after[1..],
                    Some(escaped) => {
                        out.push(escaped);
                        self.rest = &after[escaped.len_utf8()..];
                    }
                    // The backslash stays unread, so the word ends here.
                    None => break,
                }
                continue;
            }
            if stops.contains(&c) {
                break;
            }
            out.push(c);
            self.advance(c);
        }

        match out.is_empty() {
            true => Err(self.fail("expected a word")),
            false => Ok(out),
        }
    }
}

fn is_octal(c: char) -> bool {
    c.is_ascii_digit() && c != '8' && c != '9'
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lexer stands on its own: a grammar with no bash value in it still
    /// gets bash's word rules, its own stop characters, and one error type.
    #[test]
    fn a_grammar_over_other_syntax_builds_on_the_word_rules() {
        const STOPS: &[char] = &['=', ';', ' '];

        let settings = parse_with(
            r#"a='one two';b=$'x\ty';c=bare\ word"#,
            |c| {
                let mut out = Vec::new();
                loop {
                    c.ws0();
                    let key = c.word(STOPS)?;
                    c.lit("=")?;
                    out.push((key, c.word(STOPS)?));
                    if !c.eat(";") {
                        return Ok(out);
                    }
                }
            },
        )
        .unwrap();

        assert_eq!(
            settings,
            [
                ("a".to_string(), "one two".to_string()),
                ("b".to_string(), "x\ty".to_string()),
                ("c".to_string(), "bare word".to_string()),
            ]
        );
    }

    #[test]
    fn a_grammar_that_stops_early_is_an_error() {
        let failed = parse_with("abc def", |c| c.word(&[' '])).expect_err("trailing input");

        assert_eq!(failed.at, 3);
        assert!(
            failed.message.contains("trailing"),
            "{failed}"
        );
    }

    /// An offset is where the parse actually stopped, not where it started.
    #[test]
    fn a_refusal_names_the_position_it_reached() {
        let failed = parse_with("'a' 'b", |c| {
            c.word(&[' '])?;
            c.lit(" ")?;
            c.word(&[' '])
        })
        .expect_err("unterminated");

        assert_eq!(
            failed.at, 6,
            "the end of the input, inside the open quote"
        );
        assert!(
            failed.message.contains("unterminated"),
            "{failed}"
        );
    }

    /// An octal escape takes at most three digits, so a fourth is text.
    #[test]
    fn an_octal_escape_leaves_what_it_does_not_take() {
        assert_eq!(
            parse_with(r"$'\1234'", |c| c.word(&[])).unwrap(),
            "\u{53}4"
        );
    }
}
