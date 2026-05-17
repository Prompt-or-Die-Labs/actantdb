//! actant-trace — OpenTelemetry trace context propagation.
//!
//! Phase 1: helpers to mint trace + span ids and stamp them onto Chronicle
//! payloads. Real OTLP exporter lands with `tracing-opentelemetry` later.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use rand::RngCore;

/// W3C-style trace-id (16 random bytes, hex).
pub fn new_trace_id() -> String {
    let mut buf = [0u8; 16];
    rand::rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// W3C-style span-id (8 random bytes, hex).
pub fn new_span_id() -> String {
    let mut buf = [0u8; 8];
    rand::rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_id_is_32_hex() {
        let t = new_trace_id();
        assert_eq!(t.len(), 32);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn span_id_is_16_hex() {
        let s = new_span_id();
        assert_eq!(s.len(), 16);
    }
}
