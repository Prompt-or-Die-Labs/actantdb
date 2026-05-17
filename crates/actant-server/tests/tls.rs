//! TLS termination round-trip. Generates a self-signed cert with `rcgen`,
//! writes it to temp files, boots actantdb-server bound to a TLS socket,
//! then makes an HTTPS request and asserts the response.

use std::net::SocketAddr;

use actant_server::bootstrap;

#[tokio::test]
async fn tls_round_trip_serves_healthz() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    // 1. Self-signed cert for localhost.
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .expect("generate self-signed cert");
    let pem_cert = cert.cert.pem();
    let pem_key = cert.key_pair.serialize_pem();

    let dir = std::env::temp_dir().join(format!("actantdb-tls-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).unwrap();
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, pem_cert).unwrap();
    std::fs::write(&key_path, pem_key).unwrap();

    // 2. Bootstrap router on an ephemeral port via axum_server::bind_rustls.
    let (router, _state) = bootstrap(None).await.expect("bootstrap");
    let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .expect("rustls config");
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let handle = axum_server::Handle::new();
    let server_handle = handle.clone();
    let server = tokio::spawn(async move {
        axum_server::bind_rustls(addr, config)
            .handle(server_handle)
            .serve(router.into_make_service())
            .await
            .unwrap();
    });

    // Wait for the listener to bind so we know which port we got.
    let listening_addr = loop {
        if let Some(a) = handle.listening().await {
            break a;
        }
    };

    // 3. HTTPS client that trusts our test cert.
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("client");
    let url = format!(
        "https://127.0.0.1:{}/v1/healthz/ready",
        listening_addr.port()
    );
    let resp = client.get(&url).send().await.expect("https request");
    assert_eq!(resp.status(), 200, "status: {:?}", resp.status());
    let body = resp.text().await.unwrap();
    assert!(body.contains("\"phase\""), "body: {body}");

    handle.graceful_shutdown(Some(std::time::Duration::from_millis(100)));
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;

    let _ = std::fs::remove_dir_all(&dir);
}
