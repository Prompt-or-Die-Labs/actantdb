//! Model registry contract types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One row in the model registry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantModelInfo {
    /// Provider.
    pub provider: String,
    /// Model name.
    pub name: String,
    /// Locality, such as `local` or `cloud`.
    pub locality: String,
    /// Privacy class, such as `public` or `private`.
    pub privacy_class: String,
    /// Cost per 1K input tokens in USD.
    pub cost_per_input_1k: f64,
    /// Cost per 1K output tokens in USD.
    pub cost_per_output_1k: f64,
    /// 50th-percentile latency in milliseconds.
    pub latency_p50_ms: u32,
}

/// In-memory model registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ActantModelRegistry {
    models: Vec<ActantModelInfo>,
}

impl ActantModelRegistry {
    /// Register a new model.
    pub fn register(&mut self, m: ActantModelInfo) {
        self.models.push(m);
    }

    /// Pick the cheapest cloud-allowed model that matches a privacy class.
    pub fn pick_cheapest_cloud(&self, privacy_class: &str) -> Option<&ActantModelInfo> {
        self.models
            .iter()
            .filter(|m| {
                m.locality == "cloud"
                    && m.privacy_class == privacy_class
                    && m.cost_per_output_1k.is_finite()
            })
            .min_by(|a, b| {
                a.cost_per_output_1k
                    .partial_cmp(&b.cost_per_output_1k)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.provider.cmp(&b.provider))
                    .then_with(|| a.name.cmp(&b.name))
            })
    }

    /// Pick the lowest-latency local model.
    pub fn pick_local(&self) -> Option<&ActantModelInfo> {
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
        let mut r = ActantModelRegistry::default();
        r.register(ActantModelInfo {
            provider: "openai".into(),
            name: "gpt-4.1".into(),
            locality: "cloud".into(),
            privacy_class: "public".into(),
            cost_per_input_1k: 0.01,
            cost_per_output_1k: 0.03,
            latency_p50_ms: 800,
        });
        r.register(ActantModelInfo {
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
