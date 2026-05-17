//! actant-worker-manager — registry of every shipped worker, wired through
//! the same `WorkerRunner`. Lets ops run one binary that hosts whatever set
//! of effect types a deployment needs.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// Which workers a manager binary should host.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManagerConfig {
    /// Enable the shell worker.
    pub shell: bool,
    /// Enable the file worker.
    pub file: bool,
    /// Enable the model worker (mock provider).
    pub model: bool,
    /// Enable the email worker (recording mailer).
    pub email: bool,
    /// Enable the slack worker (HTTP poster).
    pub slack: bool,
    /// Enable the browser worker (emulator driver).
    pub browser: bool,
    /// Enable the MCP worker (envelope stub).
    pub mcp: bool,
}

impl ManagerConfig {
    /// Enable all workers.
    pub fn all() -> Self {
        Self {
            shell: true,
            file: true,
            model: true,
            email: true,
            slack: true,
            browser: true,
            mcp: true,
        }
    }
    /// Returns true if at least one worker is configured.
    pub fn any_enabled(&self) -> bool {
        self.shell
            || self.file
            || self.model
            || self.email
            || self.slack
            || self.browser
            || self.mcp
    }
    /// List the effect types this config will subscribe to.
    pub fn capabilities(&self) -> Vec<&'static str> {
        let mut caps = Vec::new();
        if self.shell {
            caps.push("shell.run");
        }
        if self.file {
            caps.push("file.read");
            caps.push("file.write");
        }
        if self.model {
            caps.push("model.call");
        }
        if self.email {
            caps.push("email.send");
        }
        if self.slack {
            caps.push("slack.post");
        }
        if self.browser {
            caps.push("browser.navigate");
        }
        if self.mcp {
            caps.push("mcp.call");
        }
        caps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_enables_everything() {
        let c = ManagerConfig::all();
        let caps = c.capabilities();
        assert!(caps.contains(&"shell.run"));
        assert!(caps.contains(&"file.write"));
        assert!(caps.contains(&"email.send"));
        assert!(caps.contains(&"slack.post"));
        assert!(caps.contains(&"browser.navigate"));
        assert!(caps.contains(&"model.call"));
        assert!(caps.contains(&"mcp.call"));
        assert!(c.any_enabled());
    }

    #[test]
    fn default_has_nothing() {
        let c = ManagerConfig::default();
        assert!(!c.any_enabled());
        assert!(c.capabilities().is_empty());
    }
}
