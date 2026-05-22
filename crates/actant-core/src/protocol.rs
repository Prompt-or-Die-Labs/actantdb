//! MCP / A2A / AP2 protocol surfaces.
//!
//! Phase 1 ships data shapes only — wire clients are a Phase 4 deliverable.
//! See `/specs/16-protocols.md`.

pub use actant_contracts::{ActantA2aCard, ActantAp2Mandate, ActantMcpServer};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mandate_blocks_overage() {
        let m = ActantAp2Mandate {
            holder: "agent_x".into(),
            purpose: "subscription".into(),
            spend_limit_usd: 50.0,
        };
        assert!(m.permits(49.0));
        assert!(!m.permits(51.0));
        assert!(!m.permits(-1.0));
        assert!(!m.permits(f64::NAN));
    }
}
