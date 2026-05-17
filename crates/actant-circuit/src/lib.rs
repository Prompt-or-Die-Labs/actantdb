//! actant-circuit — circuit breaker (closed/open/half-open).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Circuit state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Normal — calls pass.
    Closed,
    /// Tripped — calls fail fast.
    Open,
    /// Probing — limited calls pass.
    HalfOpen,
}

/// Circuit breaker policy.
#[derive(Debug, Clone)]
pub struct Policy {
    /// Failures in a row that trip the circuit.
    pub failure_threshold: u32,
    /// How long the circuit stays open before half-open probe.
    pub open_duration: Duration,
}

/// In-memory circuit breaker.
#[derive(Debug)]
pub struct Breaker {
    policy: Policy,
    state: State,
    failure_count: u32,
    opened_at: Option<Instant>,
}

impl Breaker {
    /// New closed breaker.
    pub fn new(policy: Policy) -> Self {
        Self {
            policy,
            state: State::Closed,
            failure_count: 0,
            opened_at: None,
        }
    }

    /// Current state with auto open→half-open transition.
    pub fn current(&mut self) -> State {
        if self.state == State::Open {
            if let Some(t) = self.opened_at {
                if t.elapsed() >= self.policy.open_duration {
                    self.state = State::HalfOpen;
                }
            }
        }
        self.state
    }

    /// True if calls should be allowed.
    pub fn allow(&mut self) -> bool {
        !matches!(self.current(), State::Open)
    }

    /// Record a success.
    pub fn on_success(&mut self) {
        self.failure_count = 0;
        self.state = State::Closed;
        self.opened_at = None;
    }

    /// Record a failure.
    pub fn on_failure(&mut self) {
        self.failure_count = self.failure_count.saturating_add(1);
        if self.failure_count >= self.policy.failure_threshold {
            self.state = State::Open;
            self.opened_at = Some(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trips_then_recovers() {
        let mut b = Breaker::new(Policy {
            failure_threshold: 2,
            open_duration: Duration::from_millis(20),
        });
        assert!(b.allow());
        b.on_failure();
        b.on_failure();
        assert_eq!(b.current(), State::Open);
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(b.current(), State::HalfOpen);
        b.on_success();
        assert_eq!(b.current(), State::Closed);
    }
}
