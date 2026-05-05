/// Encode a Rust string as a bash literal: a single bash word (one token).
/// Uses `'...'` for simple strings, `$'...'` with ANSI-C escaping otherwise.
/// Inverse of `parse_one_word`.
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
