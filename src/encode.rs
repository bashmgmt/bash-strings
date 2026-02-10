use super::types::BashValue;

/// Encode a Rust string as a bash literal.
/// Uses `'...'` for simple strings, `$'...'` with ANSI-C escaping otherwise.
/// This is the inverse of `parse_one_word` and the single canonical way to
/// produce bash-safe string literals from Rust.
pub fn encode_scalar(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    if !needs_ansi_c(s) {
        return format!("'{s}'");
    }
    ansi_c_encode(s)
}

fn needs_ansi_c(s: &str) -> bool {
    s.bytes().any(|b| b == b'\'' || b == b'\\' || b < 0x20 || b == 0x7f)
}

/// Encode as `$'...'` with full ANSI-C escape handling.
/// Inverse of `ansi_c_quoting::parse_ansi_c_body`.
fn ansi_c_encode(s: &str) -> String {
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
            '\x1B' => out.push_str("\\e"),
            '\x0C' => out.push_str("\\f"),
            '\x0B' => out.push_str("\\v"),
            c if (c as u32) < 0x20 || c == '\x7f' => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

impl BashValue {
    /// Encode as bash assignment syntax.
    /// - `String("hello")` → `'hello'`
    /// - `IndexedArray(["a", "b"])` → `('a' 'b')`
    /// - `AssocArray({"k": "v"})` → `(['k']='v')`
    pub fn to_bash(&self) -> String {
        match self {
            BashValue::String(s) => encode_scalar(s),
            BashValue::IndexedArray(v) => {
                let inner: String = v.iter().enumerate()
                    .map(|(i, s)| if i > 0 { format!(" {}", encode_scalar(s)) } else { encode_scalar(s) })
                    .collect();
                format!("({inner})")
            }
            BashValue::AssocArray(m) => {
                let inner: String = m.iter().enumerate()
                    .map(|(i, (k, v))| {
                        let pair = format!("[{}]={}", encode_scalar(k), encode_scalar(v));
                        if i > 0 { format!(" {pair}") } else { pair }
                    })
                    .collect();
                format!("({inner})")
            }
        }
    }
}
