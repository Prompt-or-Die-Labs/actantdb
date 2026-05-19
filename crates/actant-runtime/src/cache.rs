//! Content-keyed semantic cache.

use std::collections::HashMap;
use std::sync::Mutex;

use sha2::{Digest, Sha256};

/// In-memory cache. Phase 1: simple keyed store.
#[derive(Debug, Default)]
pub struct Cache {
    inner: Mutex<HashMap<String, String>>,
}

impl Cache {
    /// Compute a stable key from any serializable structure.
    pub fn key_for(value: &serde_json::Value) -> String {
        let canon = canonical(value).to_string();
        let mut h = Sha256::new();
        h.update(canon.as_bytes());
        hex::encode(h.finalize())
    }

    /// Get a value.
    pub fn get(&self, key: &str) -> Option<String> {
        self.inner.lock().unwrap().get(key).cloned()
    }

    /// Put a value.
    pub fn put(&self, key: String, value: String) {
        self.inner.lock().unwrap().insert(key, value);
    }
}

fn canonical(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut sorted: std::collections::BTreeMap<_, _> =
                map.iter().map(|(k, v)| (k.clone(), canonical(v))).collect();
            serde_json::Value::Object(sorted.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .as_object()
                .cloned()
                .map(serde_json::Value::Object)
                .unwrap_or_else(|| {
                    let mut o = serde_json::Map::new();
                    for (k, v) in sorted.iter_mut() {
                        o.insert(k.clone(), std::mem::replace(v, serde_json::Value::Null));
                    }
                    serde_json::Value::Object(o)
                })
        }
        serde_json::Value::Array(a) => serde_json::Value::Array(a.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_stable_across_key_order() {
        let k1 = Cache::key_for(&serde_json::json!({"a":1,"b":2}));
        let k2 = Cache::key_for(&serde_json::json!({"b":2,"a":1}));
        assert_eq!(k1, k2);
    }

    #[test]
    fn put_and_get() {
        let c = Cache::default();
        let k = "k1".to_string();
        c.put(k.clone(), "v1".into());
        assert_eq!(c.get(&k), Some("v1".into()));
    }
}
