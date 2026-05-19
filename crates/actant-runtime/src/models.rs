//! Model registry + routing.

use serde::{Deserialize, Serialize};

/// One row in the model registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Provider (openai, anthropic, mlx, ...).
    pub provider: String,
    /// Model name.
    pub name: String,
    /// Locality: "local" or "cloud".
    pub locality: String,
    /// Privacy class: "public" | "private" | ...
    pub privacy_class: String,
    /// Cost per 1K input tokens (USD).
    pub cost_per_input_1k: f64,
    /// Cost per 1K output tokens (USD).
    pub cost_per_output_1k: f64,
    /// 50th-percentile latency (ms).
    pub latency_p50_ms: u32,
}

/// In-memory model registry.
#[derive(Debug, Default)]
pub struct Registry {
    /// Indexed by `provider:name`.
    models: Vec<ModelInfo>,
}

impl Registry {
    /// Register a new model.
    pub fn register(&mut self, m: ModelInfo) {
        self.models.push(m);
    }

    /// Pick the cheapest cloud-allowed model that matches a privacy class.
    pub fn pick_cheapest_cloud(&self, privacy_class: &str) -> Option<&ModelInfo> {
        self.models
            .iter()
            .filter(|m| m.locality == "cloud" && m.privacy_class == privacy_class)
            .min_by(|a, b| {
                a.cost_per_output_1k
                    .partial_cmp(&b.cost_per_output_1k)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Pick the lowest-latency local model.
    pub fn pick_local(&self) -> Option<&ModelInfo> {
        self.models
            .iter()
            .filter(|m| m.locality == "local")
            .min_by_key(|m| m.latency_p50_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_cheapest_cloud() {
        let mut r = Registry::default();
        r.register(ModelInfo {
            provider: "openai".into(),
            name: "gpt-4.1".into(),
            locality: "cloud".into(),
            privacy_class: "public".into(),
            cost_per_input_1k: 0.01,
            cost_per_output_1k: 0.03,
            latency_p50_ms: 800,
        });
        r.register(ModelInfo {
            provider: "anthropic".into(),
            name: "haiku".into(),
            locality: "cloud".into(),
            privacy_class: "public".into(),
            cost_per_input_1k: 0.001,
            cost_per_output_1k: 0.005,
            latency_p50_ms: 400,
        });
        let m = r.pick_cheapest_cloud("public").unwrap();
        assert_eq!(m.name, "haiku");
    }
}
