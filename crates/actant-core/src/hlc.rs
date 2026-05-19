//! Hybrid Logical Clock (HLC) for replication-friendly event ordering.
//!
//! Implements the HLC algorithm from Kulkarni et al.,
//! "Logical Physical Clocks and Consistent Snapshots in Globally Distributed
//! Databases" (Buffalo TR 2014-04). The clock fuses a wall-clock millisecond
//! reading with a monotonic logical counter so that:
//!
//! * Two events from the same process are strictly ordered.
//! * Two events from different processes that see each other's HLC are
//!   ordered consistently after observation.
//! * Modest clock skew between machines is absorbed by the logical
//!   counter instead of corrupting ordering.
//!
//! Used to derive content-stable event ids
//! (`sha256(canonical_payload || hlc.physical_ms || hlc.logical || actor_id)`)
//! and as the tiebreaker for per-projection LWW conflict resolution
//! (see `actant-replay::conflict`).
//!
//! Storage encoding: a single `AtomicU64` holding
//! `(physical_ms << 16) | (logical & 0xFFFF)`. 48 bits of physical
//! milliseconds is enough for ~8900 years past the Unix epoch; 16 bits
//! of logical counter allows ~65K ticks per millisecond per process
//! before saturation (the clock saturates rather than panics).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// One HLC reading. Comparable as `(physical_ms, logical)` — physical
/// wins, logical breaks ties.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hlc {
    /// Wall-clock milliseconds since the Unix epoch as observed when the
    /// reading was produced or last bumped.
    pub physical_ms: u64,
    /// Monotonic counter; bumped to break ties or absorb skew.
    pub logical: u32,
}

impl Hlc {
    /// HLC at the zero point. Useful as a sentinel before any events are
    /// observed.
    pub const ZERO: Hlc = Hlc {
        physical_ms: 0,
        logical: 0,
    };

    /// Construct from explicit components.
    pub const fn new(physical_ms: u64, logical: u32) -> Self {
        Self {
            physical_ms,
            logical,
        }
    }

    /// Pack into the storage `AtomicU64` representation.
    #[inline]
    fn pack(self) -> u64 {
        ((self.physical_ms & PHYSICAL_MASK) << LOGICAL_BITS) | (self.logical as u64 & LOGICAL_MASK)
    }

    /// Unpack from the storage `AtomicU64` representation.
    #[inline]
    fn unpack(raw: u64) -> Self {
        Self {
            physical_ms: (raw >> LOGICAL_BITS) & PHYSICAL_MASK,
            logical: (raw & LOGICAL_MASK) as u32,
        }
    }
}

const LOGICAL_BITS: u64 = 16;
const LOGICAL_MASK: u64 = (1u64 << LOGICAL_BITS) - 1;
const PHYSICAL_MASK: u64 = (1u64 << (64 - LOGICAL_BITS)) - 1;
const LOGICAL_MAX: u32 = LOGICAL_MASK as u32;

/// Lock-free HLC clock backed by a single `AtomicU64`. Cheap to clone the
/// owning `Arc<HlcClock>` and share across threads.
#[derive(Debug)]
pub struct HlcClock {
    state: AtomicU64,
    physical_now_ms: fn() -> u64,
}

impl HlcClock {
    /// Construct an HLC clock seeded with `initial`. New ticks will never
    /// produce values strictly less than `initial`.
    pub fn new(initial: Hlc) -> Self {
        Self {
            state: AtomicU64::new(initial.pack()),
            physical_now_ms: system_now_ms,
        }
    }

    /// Construct with an injected physical-time source. Tests use this to
    /// drive the clock deterministically.
    pub fn with_clock_source(initial: Hlc, source: fn() -> u64) -> Self {
        Self {
            state: AtomicU64::new(initial.pack()),
            physical_now_ms: source,
        }
    }

    /// Current HLC reading without advancing it.
    pub fn peek(&self) -> Hlc {
        Hlc::unpack(self.state.load(Ordering::Acquire))
    }

    /// Tick once for a locally-generated event. Returns the HLC stamp to
    /// embed in the event.
    ///
    /// Algorithm (per HLC paper, §3):
    /// ```text
    /// pt  = current wall-clock ms
    /// old = previous (l, c)
    /// l'  = max(old.l, pt)
    /// if l' == old.l: c' = old.c + 1
    /// else:           c' = 0
    /// ```
    pub fn local_tick(&self) -> Hlc {
        let physical_now = (self.physical_now_ms)() & PHYSICAL_MASK;
        loop {
            let raw = self.state.load(Ordering::Acquire);
            let prev = Hlc::unpack(raw);
            let next = if physical_now > prev.physical_ms {
                Hlc {
                    physical_ms: physical_now,
                    logical: 0,
                }
            } else {
                Hlc {
                    physical_ms: prev.physical_ms,
                    logical: prev.logical.saturating_add(1).min(LOGICAL_MAX),
                }
            };
            if self
                .state
                .compare_exchange(raw, next.pack(), Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return next;
            }
        }
    }

    /// Tick observing a remote HLC. Must be called before applying an
    /// ingested event so subsequent local writes never go backwards.
    /// Returns the new local HLC reading.
    ///
    /// Algorithm (per HLC paper, §4):
    /// ```text
    /// pt   = current wall-clock ms
    /// old  = previous (l, c)
    /// l'   = max(old.l, remote.l, pt)
    /// match (l' == old.l, l' == remote.l):
    ///     (true,  true)  => c' = max(old.c, remote.c) + 1
    ///     (true,  false) => c' = old.c + 1
    ///     (false, true)  => c' = remote.c + 1
    ///     (false, false) => c' = 0
    /// ```
    pub fn observe(&self, remote: Hlc) -> Hlc {
        let physical_now = (self.physical_now_ms)() & PHYSICAL_MASK;
        loop {
            let raw = self.state.load(Ordering::Acquire);
            let prev = Hlc::unpack(raw);
            let l_new = prev.physical_ms.max(remote.physical_ms).max(physical_now);
            let c_new = match (l_new == prev.physical_ms, l_new == remote.physical_ms) {
                (true, true) => prev
                    .logical
                    .max(remote.logical)
                    .saturating_add(1)
                    .min(LOGICAL_MAX),
                (true, false) => prev.logical.saturating_add(1).min(LOGICAL_MAX),
                (false, true) => remote.logical.saturating_add(1).min(LOGICAL_MAX),
                (false, false) => 0,
            };
            let next = Hlc {
                physical_ms: l_new,
                logical: c_new,
            };
            if self
                .state
                .compare_exchange(raw, next.pack(), Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return next;
            }
        }
    }
}

impl Default for HlcClock {
    fn default() -> Self {
        Self::new(Hlc::ZERO)
    }
}

fn system_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_round_trip() {
        let h = Hlc::new(1_700_000_000_000, 42);
        let r = Hlc::unpack(h.pack());
        assert_eq!(h, r);
    }

    #[test]
    fn ordering_is_lex_physical_then_logical() {
        let a = Hlc::new(100, 5);
        let b = Hlc::new(100, 6);
        let c = Hlc::new(101, 0);
        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
    }

    #[test]
    fn local_tick_monotonic_under_stuck_clock() {
        // Physical time appears stuck at 100 — logical must advance.
        fn stuck() -> u64 {
            100
        }
        let clock = HlcClock::with_clock_source(Hlc::new(100, 0), stuck);
        let mut prev = clock.peek();
        for _ in 0..50 {
            let next = clock.local_tick();
            assert!(
                next > prev,
                "tick must strictly advance: {prev:?} -> {next:?}"
            );
            prev = next;
        }
    }

    #[test]
    fn local_tick_uses_physical_when_ahead() {
        fn advancing() -> u64 {
            500
        }
        let clock = HlcClock::with_clock_source(Hlc::new(100, 9), advancing);
        let next = clock.local_tick();
        assert_eq!(next.physical_ms, 500);
        assert_eq!(next.logical, 0, "logical resets when physical jumps");
    }

    #[test]
    fn observe_remote_advances_clock() {
        fn frozen() -> u64 {
            100
        }
        let clock = HlcClock::with_clock_source(Hlc::new(100, 0), frozen);
        // Remote is far ahead.
        let observed = clock.observe(Hlc::new(1_000, 7));
        assert!(observed.physical_ms >= 1_000);
        // Subsequent local tick stays >= the observed time.
        let next = clock.local_tick();
        assert!(next > observed);
    }

    #[test]
    fn observe_when_local_ahead_uses_local() {
        fn frozen() -> u64 {
            100
        }
        let clock = HlcClock::with_clock_source(Hlc::new(2_000, 3), frozen);
        let observed = clock.observe(Hlc::new(1_000, 99));
        assert_eq!(observed.physical_ms, 2_000);
        // Local wins; logical increments.
        assert_eq!(observed.logical, 4);
    }
}
