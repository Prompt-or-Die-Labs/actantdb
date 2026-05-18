//! Variable substitution for bundled templates.
//!
//! Substitutes `{{key}}` occurrences in a text body against a `HashMap`. The
//! engine is intentionally minimal: no conditionals, no loops, no escaping.
//! Unknown placeholders are left untouched (we never silently swallow them) so
//! a template author sees them in the rendered output if a variable is missing.

use std::collections::HashMap;

/// Substitute every `{{key}}` placeholder in `text` using `vars`.
///
/// Rules:
///
/// - Keys are matched literally (case-sensitive) and may contain ASCII letters,
///   digits, underscores, and dots.
/// - Whitespace inside the braces is tolerated: `{{ project_name }}` and
///   `{{project_name}}` resolve identically.
/// - A placeholder with no matching key in `vars` is left in place unchanged.
/// - Braces with non-key content (e.g. `{{ 1+2 }}`) are also left in place.
pub fn substitute(text: &str, vars: &HashMap<String, String>) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            // Find closing `}}`.
            if let Some(end) = find_close(bytes, i + 2) {
                let raw = &text[i + 2..end];
                let key = raw.trim();
                if is_valid_key(key) {
                    if let Some(value) = vars.get(key) {
                        out.push_str(value);
                        i = end + 2;
                        continue;
                    }
                }
                // Unknown / non-key contents — keep the original text including braces.
                out.push_str(&text[i..end + 2]);
                i = end + 2;
                continue;
            }
        }
        // Push one UTF-8 character so we don't split inside a multi-byte sequence.
        let ch_len = utf8_char_len(bytes[i]);
        out.push_str(&text[i..i + ch_len]);
        i += ch_len;
    }
    out
}

fn find_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut j = start;
    while j + 1 < bytes.len() {
        if bytes[j] == b'}' && bytes[j + 1] == b'}' {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn is_valid_key(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}

fn utf8_char_len(first_byte: u8) -> usize {
    if first_byte < 0x80 {
        1
    } else if first_byte < 0xC0 {
        // Continuation byte — shouldn't happen at a char boundary but be safe.
        1
    } else if first_byte < 0xE0 {
        2
    } else if first_byte < 0xF0 {
        3
    } else {
        4
    }
}
