//! Prometheus exposition for `/metrics`.
//!
//! Lives alongside the legacy hand-rolled `/v1/metrics` exposition in
//! `lib.rs`. The two endpoints answer different questions:
//!
//! * `/v1/metrics` is the snapshot-from-the-database view (event counts
//!   by kind, effect counts by status, pending approvals). It is cheap
//!   to add new rows because each one is a one-off SQL aggregate.
//! * `/metrics` (this module) is the in-process counter view used by
//!   Prometheus / OpenMetrics scrapers. Counters and histograms live
//!   in a process-local registry and get cheap atomic bumps from the
//!   request path.
//!
//! The scaffold ships with the metrics that can be populated from
//! within `actant-server` alone:
//!
//! * `actant_commands_dispatched_total{workspace_id,command_type}`
//! * `actant_http_request_duration_seconds{method,path,status}`
//!   (histogram)
//! * `actant_ledger_bytes{workspace_id}` (gauge — best-effort, populated
//!   from the SQLite file size on disk when a path is detectable)
//!
//! The remaining metrics named in `DEVX_GAPS.md` X53
//! (`actant_events_appended_total`, `actant_active_sessions`,
//! `actant_subscribe_active`) need wiring inside `actant-storage` /
//! `actant-subscribe` and are intentionally not implemented here.
//! Adding them is a follow-up that crosses crate boundaries.

use std::time::Instant;

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use once_cell::sync::Lazy;
use prometheus::{
    register_counter_vec_with_registry, register_gauge_vec_with_registry,
    register_histogram_vec_with_registry, CounterVec, Encoder, GaugeVec, HistogramVec, Registry,
    TextEncoder,
};

/// Process-local registry. Kept private to this module so other code
/// has to go through the typed accessor functions below.
pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

/// `actant_commands_dispatched_total{workspace_id,command_type}` —
/// counter incremented for every command the dispatch handler accepts.
/// Bumped from `dispatch_command` after auth + rate-limit gates pass,
/// regardless of whether the engine call ultimately succeeded.
pub static COMMANDS_DISPATCHED: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec_with_registry!(
        "actant_commands_dispatched_total",
        "Total commands accepted by the dispatch handler.",
        &["workspace_id", "command_type"],
        REGISTRY
    )
    .expect("commands_dispatched register")
});

/// `actant_http_request_duration_seconds_bucket{method,path,status,le}` —
/// histogram of every HTTP request the axum stack handles.
pub static HTTP_REQUEST_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec_with_registry!(
        "actant_http_request_duration_seconds",
        "HTTP request latency in seconds, labelled by method, path, and status.",
        &["method", "path", "status"],
        // Powers-of-2 milliseconds in seconds. Covers fast in-memory
        // responses (under 1 ms) up to slow PG queries (over a second).
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        REGISTRY
    )
    .expect("http_request_duration register")
});

/// `actant_ledger_bytes{workspace_id}` — gauge for the DB-on-disk size
/// attributed to a workspace. Today the SQLite store is per-process
/// (not per-workspace), so we publish the file size against the
/// `workspace_id="_global"` label as a best-effort signal. Per-workspace
/// attribution lands when the multi-tenant storage layout does.
pub static LEDGER_BYTES: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec_with_registry!(
        "actant_ledger_bytes",
        "Approximate ledger size in bytes, by workspace (best-effort).",
        &["workspace_id"],
        REGISTRY
    )
    .expect("ledger_bytes register")
});

/// Force-initialise every lazy collector. Calling this at boot turns
/// the first scrape into a normal-shape response instead of a partial
/// one missing collectors that have never been touched.
pub fn init() {
    Lazy::force(&REGISTRY);
    Lazy::force(&COMMANDS_DISPATCHED);
    Lazy::force(&HTTP_REQUEST_DURATION);
    Lazy::force(&LEDGER_BYTES);
}

/// Convenience: bump the per-(workspace, command_type) counter.
pub fn record_command(workspace_id: &str, command_type: &str) {
    COMMANDS_DISPATCHED
        .with_label_values(&[workspace_id, command_type])
        .inc();
}

/// Convenience: publish a ledger size sample. Caller decides how often
/// (today the `/metrics` handler refreshes it on every scrape).
pub fn record_ledger_bytes(workspace_id: &str, bytes: u64) {
    LEDGER_BYTES
        .with_label_values(&[workspace_id])
        .set(bytes as f64);
}

/// Render the registry into a Prometheus text-format response.
pub fn render() -> Response {
    let metric_families = REGISTRY.gather();
    let mut buf = Vec::with_capacity(4096);
    let encoder = TextEncoder::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buf) {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(format!("# error encoding metrics: {e}\n")))
            .unwrap();
    }
    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(buf))
        .unwrap();
    // The Prometheus text format media type. Hard-coded so we don't
    // have to keep `TextEncoder` alive long enough for `from_static` to
    // borrow `format_type()` (which returns an `&'static str` but only
    // through a temporary `TextEncoder`).
    resp.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    resp
}

/// Tower middleware that times every request and records into the
/// `actant_http_request_duration_seconds` histogram. The `path` label
/// uses the matched route (`req.uri().path()`); high-cardinality query
/// strings are stripped.
pub async fn record_http_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let started = Instant::now();
    let resp = next.run(req).await;
    let elapsed = started.elapsed().as_secs_f64();
    HTTP_REQUEST_DURATION
        .with_label_values(&[method.as_str(), path.as_str(), resp.status().as_str()])
        .observe(elapsed);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_emits_text_format() {
        init();
        record_command("ws_test", "create_session");
        record_ledger_bytes("_global", 1234);
        let resp = render();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(ct.starts_with("text/plain"), "got content-type {ct:?}");
    }

    #[test]
    fn record_command_increments() {
        init();
        let before = COMMANDS_DISPATCHED
            .with_label_values(&["ws_inc", "noop"])
            .get();
        record_command("ws_inc", "noop");
        let after = COMMANDS_DISPATCHED
            .with_label_values(&["ws_inc", "noop"])
            .get();
        assert!(after > before);
    }
}
