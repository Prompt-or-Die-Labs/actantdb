use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli_errors;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TelemetryChoice {
    Enabled,
    Disabled,
}

#[derive(Debug, Serialize, Deserialize)]
struct TelemetryConfig {
    telemetry: TelemetryChoice,
    decided_at: String,
    version: u8,
}

pub(crate) fn prompt_if_needed() -> anyhow::Result<()> {
    let path = config_path();
    if path.exists() {
        return Ok(());
    }
    if let Some(choice) = env_choice()? {
        persist_choice(&path, choice)?;
        return Ok(());
    }
    if std::env::var_os("CI").is_some() || !std::io::stdin().is_terminal() {
        return Ok(());
    }

    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut output = std::io::stderr();
    prompt_with(&path, &mut input, &mut output)
}

fn prompt_with<R: BufRead, W: Write>(
    path: &Path,
    input: &mut R,
    output: &mut W,
) -> anyhow::Result<()> {
    if path.exists() {
        return Ok(());
    }

    writeln!(
        output,
        "ActantDB would like to collect anonymous CLI usage and error telemetry so we can fix what breaks."
    )?;
    writeln!(
        output,
        "No ledger contents, prompts, payloads, database paths, or secrets are sent."
    )?;
    loop {
        write!(output, "Share anonymous usage? [y/N] ")?;
        output.flush()?;

        let mut answer = String::new();
        input.read_line(&mut answer)?;
        match parse_answer(&answer) {
            Some(choice) => {
                persist_choice(path, choice)?;
                return Ok(());
            }
            None => writeln!(output, "Please answer y or n.")?,
        }
    }
}

fn parse_answer(raw: &str) -> Option<TelemetryChoice> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => Some(TelemetryChoice::Enabled),
        "" | "n" | "no" | "skip" => Some(TelemetryChoice::Disabled),
        _ => None,
    }
}

fn env_choice() -> anyhow::Result<Option<TelemetryChoice>> {
    let Some(raw) = std::env::var_os("ACTANTDB_TELEMETRY") else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy().trim().to_ascii_lowercase();
    match raw.as_str() {
        "1" | "true" | "yes" | "on" => Ok(Some(TelemetryChoice::Enabled)),
        "0" | "false" | "no" | "off" | "skip" => Ok(Some(TelemetryChoice::Disabled)),
        "" | "ask" => Ok(None),
        other => Err(cli_errors::invalid_input(format!(
            "ACTANTDB_TELEMETRY must be yes, no, on, off, or ask; got `{other}`"
        ))
        .into()),
    }
}

fn persist_choice(path: &Path, choice: TelemetryChoice) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| cli_errors::storage("create config dir", e))?;
    }
    let body = TelemetryConfig {
        telemetry: choice,
        decided_at: crate::chrono_rfc3339(),
        version: 1,
    };
    let data = serde_json::to_vec_pretty(&body)
        .map_err(|e| cli_errors::internal("encode telemetry config", e))?;
    std::fs::write(path, data).map_err(|e| cli_errors::storage("write telemetry config", e))?;
    Ok(())
}

fn config_path() -> PathBuf {
    let mut path = crate::dirs_local();
    path.push(".actantdb");
    path.push("config.json");
    path
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn yes_persists_enabled_choice() {
        let path = temp_path();
        let mut input = Cursor::new(b"y\n");
        let mut output = Vec::new();

        prompt_with(&path, &mut input, &mut output).unwrap();

        let data = std::fs::read_to_string(&path).unwrap();
        assert!(data.contains(r#""telemetry": "enabled""#), "{data}");
        cleanup(&path);
    }

    #[test]
    fn blank_answer_persists_disabled_choice() {
        let path = temp_path();
        let mut input = Cursor::new(b"\n");
        let mut output = Vec::new();

        prompt_with(&path, &mut input, &mut output).unwrap();

        let data = std::fs::read_to_string(&path).unwrap();
        assert!(data.contains(r#""telemetry": "disabled""#), "{data}");
        cleanup(&path);
    }

    #[test]
    fn existing_choice_skips_prompt() {
        let path = temp_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "{}").unwrap();
        let mut input = Cursor::new(b"y\n");
        let mut output = Vec::new();

        prompt_with(&path, &mut input, &mut output).unwrap();

        assert!(output.is_empty());
        cleanup(&path);
    }

    fn temp_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("actantdb-telemetry-test-{}", ulid::Ulid::new()));
        path.push("config.json");
        path
    }

    fn cleanup(path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}
