//! Spec 09 — SDK design verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn ts_sdk_exposes_every_alpha_command() {
    let sdk = read_repo("packages/actant-sdk/src/index.ts");
    for needle in [
        "createSession",
        "appendUserMessage",
        "requestToolCall",
        "approveToolCall",
        "recordToolResult",
    ] {
        assert!(sdk.contains(needle), "TS SDK missing method {needle}");
    }
    assert!(
        sdk.contains("async command("),
        "TS SDK missing generic command()"
    );
}

#[test]
fn python_sdk_exposes_every_alpha_command() {
    let sdk = read_repo("sdks/python/actantdb/client.py");
    for needle in [
        "create_session",
        "append_user_message",
        "request_tool_call",
        "approve_tool_call",
        "record_tool_result",
    ] {
        assert!(sdk.contains(needle), "Python SDK missing method {needle}");
    }
}

#[test]
fn ts_sdk_does_not_silently_retry() {
    // Spec 09: "No SDK silently retries `command` calls."
    // Extract the `async command(...)` method body by brace-matching.
    let sdk = read_repo("packages/actant-sdk/src/index.ts");
    let after = sdk.split("async command(").nth(1).unwrap_or("");
    let mut depth: i32 = 0;
    let mut body = String::new();
    let mut started = false;
    for c in after.chars() {
        if c == '{' {
            depth += 1;
            started = true;
        }
        if started {
            body.push(c);
            if c == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
        }
    }
    let fetcher_count = body.matches("this.fetcher(").count();
    assert_eq!(
        fetcher_count, 1,
        "command() body makes {fetcher_count} fetcher calls — must be exactly 1 (no retry)"
    );
    for needle in ["retry", "while (", ".catch("] {
        assert!(
            !body.contains(needle),
            "command() body contains forbidden retry-shape pattern: {needle}"
        );
    }
}

#[test]
fn metadata_fetch_is_the_only_truth() {
    // Spec 09: "The metadata fetch in §7 is the only source of truth — no
    // SDK hand-writes command shapes."
    // Approximation: the SDK uses a generic `command()` method; convenience
    // wrappers exist, but they all funnel through `command()`.
    let sdk = read_repo("packages/actant-sdk/src/index.ts");
    assert!(sdk.contains("/v1/command"), "SDK must call /v1/command");
    assert!(
        sdk.contains("this.command("),
        "convenience methods must funnel through command()"
    );
}
