//! Live smoke test for [`CdpDriver`] against a real Chrome/Chromium.
//!
//! Only built when the `cdp` feature is enabled. Even then the test is
//! `#[ignore]` because workspace CI runners do not ship Chrome - run
//! explicitly with:
//!
//! ```text
//! cargo test -p actant-worker-browser --features cdp -- --ignored cdp_smoke
//! ```
//!
//! The test launches a headless browser, navigates to a tiny inline data
//! URL, takes a screenshot, and asserts the returned JSON carries a
//! non-zero PNG byte count.

#![cfg(feature = "cdp")]

use actant_worker_browser::cdp::CdpDriver;
use actant_worker_browser::{Action, Driver};

fn chrome_on_path() -> bool {
    // chromiumoxide auto-detects google-chrome / chromium / chrome on PATH.
    // We mirror that detection so the test can self-skip with a useful
    // message rather than blowing up inside the launch handshake.
    for name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ] {
        if which(name).is_some() {
            return true;
        }
    }
    // macOS bundle.
    std::path::Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome").exists()
        || std::path::Path::new("/Applications/Chromium.app/Contents/MacOS/Chromium").exists()
}

fn which(prog: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(prog);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[tokio::test]
#[ignore = "requires Chrome/Chromium on PATH; opt in with --ignored"]
async fn navigate_and_screenshot_real_chrome() {
    if !chrome_on_path() {
        eprintln!("skipping cdp_smoke: no Chrome/Chromium on PATH");
        return;
    }
    let driver = CdpDriver::launch_headless()
        .await
        .expect("launch headless chrome");
    let nav = driver
        .run(Action::Navigate(
            "data:text/html,<h1>hello</h1>".to_string(),
        ))
        .await
        .expect("navigate");
    assert!(nav.get("title").is_some(), "nav result missing title");

    let shot = driver
        .run(Action::Screenshot)
        .await
        .expect("screenshot");
    let bytes = shot
        .get("bytes")
        .and_then(|v| v.as_u64())
        .expect("bytes field present");
    assert!(bytes > 0, "screenshot returned zero bytes");

    driver.close().await.expect("clean shutdown");
}
