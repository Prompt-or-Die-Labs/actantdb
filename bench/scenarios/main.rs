//! End-to-end ActantDB benchmark scenarios.
//!
//! Drives a *running* `actantdb-server` over HTTP and reports
//! p50/p95/p99 latencies + throughput for realistic workloads. Designed
//! to run against the released v0.0.7 release binary on an on-disk DB,
//! complementing the in-memory criterion micro-benchmarks under
//! `bench/benches/`.
//!
//! Usage:
//!   actantdb-server --bind 127.0.0.1:4555 --db /tmp/bench.db &
//!   cargo run -p actant-bench --release --bin scenarios -- \
//!       --base-url http://127.0.0.1:4555 \
//!       --workspace ws_bench --actor act_bench \
//!       --scenario all

use std::time::{Duration, Instant};

use clap::Parser;
use futures::future::join_all;
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(name = "scenarios", about = "ActantDB end-to-end scenarios")]
struct Cli {
    /// Base URL of a running actantdb-server.
    #[arg(long, default_value = "http://127.0.0.1:4555")]
    base_url: String,

    /// Workspace id (must already exist; seed with `sqlite3 INSERT INTO workspace`).
    #[arg(long, default_value = "ws_bench")]
    workspace: String,

    /// Actor id (must already exist).
    #[arg(long, default_value = "act_bench")]
    actor: String,

    /// Which scenario to run: single | concurrent | replay | all.
    #[arg(long, default_value = "all")]
    scenario: String,

    /// Single-agent burst message count.
    #[arg(long, default_value_t = 100)]
    single_msgs: usize,

    /// Concurrent: number of sessions.
    #[arg(long, default_value_t = 10)]
    conc_sessions: usize,

    /// Concurrent: messages per session.
    #[arg(long, default_value_t = 50)]
    conc_msgs: usize,

    /// Replay scenario: number of events to record before replay.
    #[arg(long, default_value_t = 200)]
    replay_events: usize,

    /// Output raw per-request latencies (ns) to this file.
    #[arg(long)]
    raw_out: Option<String>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

    // Confirm server is up.
    let ready: Value = client
        .get(format!("{}/v1/healthz/ready", cli.base_url))
        .send()
        .await?
        .json()
        .await?;
    if ready.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Err(format!("server not ready: {ready}").into());
    }
    println!("server ready at {}", cli.base_url);
    println!("workspace={} actor={}", cli.workspace, cli.actor);
    println!();

    match cli.scenario.as_str() {
        "single" => {
            single_agent_burst(&client, &cli, cli.single_msgs).await?;
        }
        "concurrent" => {
            concurrent_sessions(&client, &cli, cli.conc_sessions, cli.conc_msgs).await?;
        }
        "replay" => {
            replay_from_event(&client, &cli, cli.replay_events).await?;
        }
        "all" => {
            single_agent_burst(&client, &cli, cli.single_msgs).await?;
            println!();
            concurrent_sessions(&client, &cli, cli.conc_sessions, cli.conc_msgs).await?;
            println!();
            replay_from_event(&client, &cli, cli.replay_events).await?;
        }
        other => return Err(format!("unknown scenario: {other}").into()),
    }

    if let Some(path) = cli.raw_out.as_ref() {
        println!("(raw latencies would be at {path} if requested per-run)");
    }
    Ok(())
}

/// POST a command, returning (latency_ns, parsed result).
async fn dispatch_timed(
    client: &Client,
    base_url: &str,
    workspace: &str,
    actor: &str,
    command_type: &str,
    input: Value,
) -> Result<(u128, Value), Box<dyn std::error::Error>> {
    let body = json!({
        "workspace_id": workspace,
        "actor_id": actor,
        "command_type": command_type,
        "input": input,
    });
    let url = format!("{base_url}/v1/command");
    let start = Instant::now();
    let resp = client.post(&url).json(&body).send().await?;
    let status = resp.status();
    let v: Value = resp.json().await?;
    let elapsed = start.elapsed().as_nanos();
    if !status.is_success() {
        return Err(format!("command {command_type} failed: {status} {v}").into());
    }
    Ok((elapsed, v))
}

async fn create_session(client: &Client, cli: &Cli) -> Result<String, Box<dyn std::error::Error>> {
    let (_, v) = dispatch_timed(
        client,
        &cli.base_url,
        &cli.workspace,
        &cli.actor,
        "create_session",
        json!({}),
    )
    .await?;
    Ok(v["result"]["session_id"].as_str().unwrap().to_string())
}

// --- scenario 1: single-agent burst ----------------------------------------

async fn single_agent_burst(
    client: &Client,
    cli: &Cli,
    n_msgs: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("== scenario: single-agent burst ({n_msgs} sequential msgs) ==");

    let (session_lat_ns, sess_resp) = dispatch_timed(
        client,
        &cli.base_url,
        &cli.workspace,
        &cli.actor,
        "create_session",
        json!({}),
    )
    .await?;
    let session_id = sess_resp["result"]["session_id"]
        .as_str()
        .unwrap()
        .to_string();
    println!(
        "  create_session: {:.3} ms",
        session_lat_ns as f64 / 1_000_000.0
    );

    let mut latencies = Vec::with_capacity(n_msgs);
    let wall_start = Instant::now();
    for i in 0..n_msgs {
        let (lat, _v) = dispatch_timed(
            client,
            &cli.base_url,
            &cli.workspace,
            &cli.actor,
            "append_user_message",
            json!({"session_id": session_id, "text": format!("msg {i}")}),
        )
        .await?;
        latencies.push(lat);
    }
    let wall = wall_start.elapsed();
    report_quantiles("append_user_message", &mut latencies, wall);
    Ok(())
}

// --- scenario 2: concurrent sessions ---------------------------------------

async fn concurrent_sessions(
    client: &Client,
    cli: &Cli,
    n_sessions: usize,
    msgs_per_session: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("== scenario: concurrent ({n_sessions} sessions x {msgs_per_session} msgs) ==");

    // Pre-create sessions sequentially so the bench measures only the
    // concurrent message phase.
    let mut sessions = Vec::with_capacity(n_sessions);
    for _ in 0..n_sessions {
        sessions.push(create_session(client, cli).await?);
    }

    let wall_start = Instant::now();
    let mut handles = Vec::with_capacity(n_sessions);
    for sid in sessions {
        let client = client.clone();
        let base_url = cli.base_url.clone();
        let ws = cli.workspace.clone();
        let actor = cli.actor.clone();
        let n = msgs_per_session;
        handles.push(tokio::spawn(async move {
            let mut lats: Vec<u128> = Vec::with_capacity(n);
            for i in 0..n {
                let (lat, _v) = dispatch_timed(
                    &client,
                    &base_url,
                    &ws,
                    &actor,
                    "append_user_message",
                    json!({"session_id": sid, "text": format!("c{i}")}),
                )
                .await
                .expect("dispatch");
                lats.push(lat);
            }
            lats
        }));
    }

    let mut latencies: Vec<u128> = Vec::with_capacity(n_sessions * msgs_per_session);
    for r in join_all(handles).await {
        latencies.extend(r.expect("task"));
    }
    let wall = wall_start.elapsed();
    report_quantiles("append_user_message (concurrent)", &mut latencies, wall);
    Ok(())
}

// --- scenario 3: replay-from-event -----------------------------------------

async fn replay_from_event(
    client: &Client,
    cli: &Cli,
    n_events: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("== scenario: replay-from-event ({n_events}-event session) ==");

    let session_id = create_session(client, cli).await?;
    // Record n_events user messages.
    let mut last_event_id: Option<String> = None;
    let record_start = Instant::now();
    for i in 0..n_events {
        let (_, v) = dispatch_timed(
            client,
            &cli.base_url,
            &cli.workspace,
            &cli.actor,
            "append_user_message",
            json!({"session_id": session_id, "text": format!("r{i}")}),
        )
        .await?;
        last_event_id = v["event_id"].as_str().map(|s| s.to_string());
    }
    let record_wall = record_start.elapsed();
    println!(
        "  record {n_events} events: wall={:.2} ms ({:.0} ev/s)",
        record_wall.as_secs_f64() * 1000.0,
        n_events as f64 / record_wall.as_secs_f64()
    );

    let event_id = last_event_id.ok_or("no event id")?;
    // Create checkpoint.
    let cp_start = Instant::now();
    let cp_resp: Value = client
        .post(format!("{}/v1/replay/checkpoint", cli.base_url))
        .json(&json!({"workspace_id": cli.workspace, "event_id": event_id}))
        .send()
        .await?
        .json()
        .await?;
    let cp_wall = cp_start.elapsed();
    let checkpoint_id = cp_resp["checkpoint_id"]
        .as_str()
        .ok_or_else(|| format!("no checkpoint_id in response: {cp_resp}"))?;
    println!("  checkpoint: {:.2} ms", cp_wall.as_secs_f64() * 1000.0);

    // Run replay in 'recorded' mode + diff (server returns the diff).
    let run_start = Instant::now();
    let run_resp = client
        .post(format!("{}/v1/replay/run", cli.base_url))
        .json(&json!({
            "actor_id": cli.actor,
            "checkpoint_id": checkpoint_id,
            "mode": "recorded"
        }))
        .send()
        .await?;
    let status = run_resp.status();
    let run_body: Value = run_resp.json().await?;
    let run_wall = run_start.elapsed();
    if !status.is_success() {
        return Err(format!("replay run failed: {status} {run_body}").into());
    }
    let entry_count = run_body["entries"].as_array().map(|a| a.len()).unwrap_or(0);
    println!(
        "  replay run+diff: {:.2} ms ({} diff entries)",
        run_wall.as_secs_f64() * 1000.0,
        entry_count
    );
    println!(
        "  build+diff total: {:.2} ms",
        (cp_wall + run_wall).as_secs_f64() * 1000.0
    );
    Ok(())
}

// --- shared reporter -------------------------------------------------------

fn report_quantiles(label: &str, lats: &mut [u128], wall: Duration) {
    lats.sort_unstable();
    let n = lats.len();
    let q = |p: f64| {
        let i = ((n as f64) * p).floor() as usize;
        let i = i.min(n - 1);
        lats[i] as f64 / 1000.0
    };
    let mean = lats.iter().sum::<u128>() as f64 / (n as f64) / 1000.0;
    let throughput = n as f64 / wall.as_secs_f64();
    println!(
        "  {label}: n={n} wall={:.2} ms throughput={:.0} req/s",
        wall.as_secs_f64() * 1000.0,
        throughput
    );
    println!(
        "    min={:.1} µs  mean={:.1} µs  p50={:.1} µs  p95={:.1} µs  p99={:.1} µs  max={:.1} µs",
        lats[0] as f64 / 1000.0,
        mean,
        q(0.50),
        q(0.95),
        q(0.99),
        lats[n - 1] as f64 / 1000.0,
    );
}
