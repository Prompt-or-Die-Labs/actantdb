//! Token-bucket rate limiter.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Rate-limit policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Maximum tokens in the bucket.
    pub limit: u32,
    /// Tokens refilled per second.
    pub refill_per_second: f64,
}

/// Token bucket state.
#[derive(Debug)]
pub struct Bucket {
    policy: Policy,
    tokens: f64,
    last: Instant,
}

impl Bucket {
    /// New full bucket.
    pub fn new(policy: Policy) -> Self {
        let tokens = policy.limit as f64;
        Self {
            policy,
            tokens,
            last: Instant::now(),
        }
    }

    /// Try to consume `n` tokens.
    pub fn try_consume(&mut self, n: u32) -> Result<(), Duration> {
        self.refill();
        if (self.tokens as u32) >= n {
            self.tokens -= n as f64;
            Ok(())
        } else {
            let deficit = n as f64 - self.tokens;
            let secs = deficit / self.policy.refill_per_second.max(0.001);
            Err(Duration::from_secs_f64(secs))
        }
    }

    fn refill(&mut self) {
        let elapsed = self.last.elapsed().as_secs_f64();
        self.tokens =
            (self.tokens + elapsed * self.policy.refill_per_second).min(self.policy.limit as f64);
        self.last = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_burst_until_drained() {
        let mut b = Bucket::new(Policy {
            limit: 3,
            refill_per_second: 1.0,
        });
        assert!(b.try_consume(1).is_ok());
        assert!(b.try_consume(1).is_ok());
        assert!(b.try_consume(1).is_ok());
        assert!(b.try_consume(1).is_err());
    }
}
