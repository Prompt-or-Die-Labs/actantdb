//! actant-prompts — prompt template + version registry.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// A prompt template version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    /// Version number (monotonic).
    pub version: u32,
    /// Body (mustache-style placeholders are interpolated by `render`).
    pub body: String,
}

/// A prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Display name.
    pub name: String,
    /// Versions, ordered ascending.
    pub versions: Vec<Version>,
}

impl Template {
    /// Latest version.
    pub fn latest(&self) -> Option<&Version> {
        self.versions.last()
    }
    /// Render a version with `{{var}}` substitutions.
    pub fn render(&self, version: u32, vars: &serde_json::Value) -> Option<String> {
        let v = self.versions.iter().find(|v| v.version == version)?;
        Some(interpolate(&v.body, vars))
    }
}

fn interpolate(body: &str, vars: &serde_json::Value) -> String {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            chars.next();
            let mut key = String::new();
            while let Some(c2) = chars.next() {
                if c2 == '}' && chars.peek() == Some(&'}') {
                    chars.next();
                    break;
                }
                key.push(c2);
            }
            let key = key.trim();
            let val = vars.get(key).and_then(|v| v.as_str()).unwrap_or("");
            out.push_str(val);
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation() {
        let t = Template {
            name: "greet".into(),
            versions: vec![Version {
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
