//! SDK codegen — emit thin client SDKs (TS, Python, Swift) from the
//! contract types. Phase 1 ships a TS client stub.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Languages supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActantLang {
    /// TypeScript / Node.
    Ts,
    /// Python.
    Py,
    /// Swift.
    Swift,
}

/// Emit a thin client wrapper that posts JSON to `/v1/command`.
pub fn emit_client(lang: ActantLang) -> String {
    match lang {
        ActantLang::Ts => include_str!("../templates/client.ts").into(),
        ActantLang::Py => include_str!("../templates/client.py").into(),
        ActantLang::Swift => include_str!("../templates/client.swift").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_each_lang() {
        for l in [ActantLang::Ts, ActantLang::Py, ActantLang::Swift] {
            assert!(!emit_client(l).is_empty());
        }
    }
}
