//! Cross-actor isolation property test for `actant-cache`.
//!
//! NOTE — limits of this test:
//! The v0.1 `Cache` type in `actant-cache` is namespace-agnostic: its public
//! surface is `get(&str) -> Option<String>` and `put(String, String)`. There
//! is no actor or sensitivity parameter on the production API. The full
//! `CacheService` described in `agents/actant-cache.md` (with per-actor
//! sensitivity-aware reads) is not yet implemented.
//!
//! What this test therefore verifies: when callers prefix keys with the
//! actor id (the convention every consumer in the repo follows today, and
//! the foundation on which the future `CacheService` will be built), no
//! actor ever observes another actor's value. In other words: this proves
//! the underlying key-space discipline is sound, which is the precondition
//! for the spec'd actor-isolation guarantee.
//!
//! Once `CacheService` lands, replace this test with one that exercises
//! its `actor`/`sensitivity_ceiling` parameters directly.

use std::collections::HashMap;

use actant_cache::Cache;
use proptest::collection::vec;
use proptest::prelude::*;

/// One simulated put.
#[derive(Debug, Clone)]
struct Put {
    actor: u8,       // 0..16 — restricted alphabet so collisions on key happen.
    key: String,     // 1..16 chars, ascii alphanumeric.
    sensitivity: u8, // 0..6 — mirrors the Sensitivity enum cardinality.
    value: String,   // 0..32 chars.
}

prop_compose! {
    fn arb_put()(
        actor in 0u8..16,
        key in "[a-z0-9]{1,16}",
        sensitivity in 0u8..6,
        value in "[a-zA-Z0-9 ]{0,32}",
    ) -> Put {
        Put { actor, key, sensitivity, value }
    }
}

/// The convention every caller follows: scope the cache key with the actor id.
fn scoped_key(actor: u8, key: &str, sensitivity: u8) -> String {
    format!("actor:{actor}|sens:{sensitivity}|key:{key}")
}

proptest! {
    #![proptest_config(ProptestConfig {
        // 1000 random put/get rounds, as required by the work package AC.
        cases: 1000,
        .. ProptestConfig::default()
    })]
    #[test]
    fn cross_actor_isolation(
        puts in vec(arb_put(), 1..32),
    ) {
        let cache = Cache::default();

        // Shadow map: the "truth" the cache must agree with. The shadow
        // keys mirror the scoping convention, so any leak between actors
        // shows up as a divergence between cache state and shadow state.
        let mut shadow: HashMap<String, String> = HashMap::new();

        for p in &puts {
            let k = scoped_key(p.actor, &p.key, p.sensitivity);
            cache.put(k.clone(), p.value.clone());
            shadow.insert(k, p.value.clone());
        }

        // Now perform a get for every (actor, key, sensitivity) tuple
        // the shadow knows about. For each one, the cache MUST return the
        // value the shadow stored — and it must NOT return any value
        // that belongs to a different actor.
        for p in &puts {
            let owner_key = scoped_key(p.actor, &p.key, p.sensitivity);
            let got = cache.get(&owner_key);
            let expected = shadow.get(&owner_key).cloned();
            prop_assert_eq!(
                got.clone(),
                expected.clone(),
                "actor {} expected {:?} for key {}, got {:?}",
                p.actor, expected, owner_key, got
            );

            // Cross-actor probe: for every OTHER actor in 0..16, ensure
            // that asking for THIS key under their namespace does not
            // accidentally return our value.
            for other_actor in 0u8..16 {
                if other_actor == p.actor {
                    continue;
                }
                let other_key = scoped_key(other_actor, &p.key, p.sensitivity);
                let other_got = cache.get(&other_key);
                if let Some(v) = &other_got {
                    // It is only acceptable if the other actor genuinely
                    // performed a put with the same (key, sensitivity).
                    let truth = shadow.get(&other_key);
                    prop_assert_eq!(
                        Some(v),
                        truth,
                        "leak: actor {} got actor {}'s value {:?} under key {}",
                        other_actor, p.actor, v, other_key
                    );
                }
            }
        }
    }
}
