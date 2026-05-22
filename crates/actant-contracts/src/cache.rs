//! Content-keyed semantic cache contract type.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use sha2::{Digest, Sha256};

const DEFAULT_MAX_ENTRIES: usize = 1024;

/// In-memory content cache.
#[derive(Debug)]
pub struct ActantCache {
    inner: Mutex<CacheInner>,
    max_entries: usize,
}

#[derive(Debug, Default)]
struct CacheInner {
    values: HashMap<String, String>,
    lru: VecDeque<String>,
}

impl CacheInner {
    fn touch(&mut self, key: &str) {
        self.lru.retain(|existing| existing != key);
        self.lru.push_back(key.to_owned());
    }

    fn evict_until_room(&mut self, max_entries: usize) {
        while self.values.len() >= max_entries {
            match self.lru.pop_front() {
                Some(key) => {
                    self.values.remove(&key);
                }
                None => {
                    self.values.clear();
                    return;
                }
            }
        }
    }
}

impl Default for ActantCache {
    fn default() -> Self {
        Self::with_max_entries(DEFAULT_MAX_ENTRIES)
    }
}

impl ActantCache {
    /// Create a bounded cache.
    pub fn with_max_entries(max_entries: usize) -> Self {
        assert!(
            max_entries > 0,
            "cache max_entries must be greater than zero"
        );
        Self {
            inner: Mutex::new(CacheInner::default()),
            max_entries,
        }
    }

    /// Compute a stable key from any JSON value.
    pub fn key_for(value: &serde_json::Value) -> String {
        sha256_hex(canonical_json(value).as_bytes())
    }

    /// Get a value.
    pub fn get(&self, key: &str) -> Option<String> {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let value = inner.values.get(key).cloned()?;
        inner.touch(key);
        Some(value)
    }

    /// Put a value.
    pub fn put(&self, key: String, value: String) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !inner.values.contains_key(&key) {
            inner.evict_until_room(self.max_entries);
        }
        inner.values.insert(key.clone(), value);
        inner.touch(&key);
    }
}

fn canonical_json(value: &serde_json::Value) -> String {
    canonicalize(value).to_string()
}

fn canonicalize(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: std::collections::BTreeMap<String, serde_json::Value> =
                std::collections::BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), canonicalize(v));
            }
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonicalize).collect())
        }
        other => other.clone(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_stable_across_key_order() {
        let k1 = ActantCache::key_for(&serde_json::json!({"a":1,"b":2}));
        let k2 = ActantCache::key_for(&serde_json::json!({"b":2,"a":1}));
        assert_eq!(k1, k2);
    }

    #[test]
    fn put_and_get() {
        let c = ActantCache::default();
        let k = "k1".to_string();
        c.put(k.clone(), "v1".into());
        assert_eq!(c.get(&k), Some("v1".into()));
    }

    #[test]
    fn evicts_least_recently_used_entry() {
        let c = ActantCache::with_max_entries(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".into()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
        assert_eq!(c.get("a"), Some("1".into()));
        assert_eq!(c.get("c"), Some("3".into()));
    }
}
