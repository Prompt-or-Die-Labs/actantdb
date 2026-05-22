//! Prompt template + version registry.

pub use actant_contracts::{ActantPromptTemplate, ActantPromptVersion};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation() {
        let t = ActantPromptTemplate {
            name: "greet".into(),
            versions: vec![ActantPromptVersion {
                version: 1,
                body: "Hello, {{name}}!".into(),
            }],
        };
        assert_eq!(
            t.render(1, &serde_json::json!({"name":"Wes"})).unwrap(),
            "Hello, Wes!"
        );
    }
}
