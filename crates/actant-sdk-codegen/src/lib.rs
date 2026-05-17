//! actant-sdk-codegen — emit thin client SDKs (TS, Python, Swift) from the
//! contract types. Phase 1 ships a TS client stub.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Languages supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// TypeScript / Node.
    Ts,
    /// Python.
    Py,
    /// Swift.
    Swift,
}

/// Emit a thin client wrapper that posts JSON to `/v1/command`.
pub fn emit_client(lang: Lang) -> String {
    match lang {
        Lang::Ts => include_str!("../templates/client.ts").into(),
        Lang::Py => include_str!("../templates/client.py").into(),
        Lang::Swift => include_str!("../templates/client.swift").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_each_lang() {
        for l in [Lang::Ts, Lang::Py, Lang::Swift] {
            assert!(!emit_client(l).is_empty());
        }
    }
}
