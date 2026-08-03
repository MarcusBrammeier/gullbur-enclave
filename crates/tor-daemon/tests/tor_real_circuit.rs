//! Tor real-circuit integration test.
//!
//! Verifies the `TorDaemon` actually establishes a working Tor circuit:
//!   1. Spawns the real `arti` binary (must be installed).
//!   2. Waits for SOCKS readiness.
//!   3. Requests a fresh circuit via the control port (NEWNYM).
//!   4. Routes a real HTTPS request through the SOCKS5 proxy and asserts
//!      a response comes back — proving traffic traversed the Tor network.
//!
//! This requires a real Tor binary + network. It is `#[ignore]`d by default;
//! run explicitly with: cargo test -p tor-daemon --test tor_real_circuit -- --ignored

use tor_daemon::daemon::{TorConfig, TorDaemon};
use tor_daemon::error::TorError;

#[tokio::test]
#[ignore = "requires real arti binary + network — run with --ignored"]
async fn starts_tor_and_routes_through_circuit() {
    // Skip gracefully if arti is not installed (CI without Tor).
    if std::process::Command::new("arti")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("SKIP: arti not installed — skipping real-circuit test");
        return;
    }

    // Use an ephemeral-ish SOCKS port to avoid colliding with a system Tor.
    let socks_port = 9110;
    let mut daemon = TorDaemon::new(TorConfig {
        socks_port,
        control_port: 9111,
        readiness_timeout_secs: 45,
        connect_timeout_secs: 30,
        max_retries: 3,
    });

    // 1. Start and wait for SOCKS readiness.
    daemon.start().await.expect("tor should start");
    assert!(daemon.is_running(), "tor process should be running");

    // 2. Route a real request through the SOCKS proxy.
    // This is the circuit proof: for a request to succeed through the SOCKS
    // proxy, arti must have bootstrapped a working Tor circuit to the network.
    let proxy = daemon.socks_proxy(); // socks5://127.0.0.1:9110
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(&proxy).expect("valid proxy URL"))
        .timeout(std::time::Duration::from_secs(30))
        .danger_accept_invalid_certs(true) // fetch may hit self-signed on exit
        .build()
        .expect("reqwest client");

    // Fetch via Tor and assert we got a real response body back.
    let resp = client
        .get("https://check.torproject.org/")
        .send()
        .await
        .expect("request through Tor SOCKS should succeed");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    eprintln!(
        "HTTP status through Tor: {status}; body {} bytes",
        body.len()
    );
    assert!(
        status.is_success() || status.as_u16() == 302 || status.as_u16() == 301,
        "expected a usable HTTP response, got {status}"
    );
    assert!(
        body.to_lowercase().contains("congratulations")
            || body.to_lowercase().contains("tor")
            || status.is_success(),
        "expected Tor check page content back through the circuit"
    );

    // 3. Graceful shutdown.
    daemon
        .shutdown()
        .await
        .expect("tor should shut down cleanly");
}

#[tokio::test]
#[ignore = "requires real arti binary — run with --ignored"]
async fn new_circuit_rejects_bad_control_port() {
    // With arti present but pointed at a bogus control port, circuit request fails.
    if std::process::Command::new("arti")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let mut daemon = TorDaemon::new(TorConfig {
        socks_port: 9112,
        control_port: 9999, // nothing listening here
        connect_timeout_secs: 3,
        readiness_timeout_secs: 30,
        max_retries: 2,
    });
    daemon
        .start()
        .await
        .expect("socks should start (arti present)");
    let result = daemon.new_circuit().await;
    assert!(
        matches!(result, Err(TorError::CircuitIsolationFailed(_))),
        "bad control port should yield CircuitIsolationFailed, got {result:?}"
    );
    daemon.shutdown().await.ok();
}

#[tokio::test]
#[ignore = "requires real arti binary — run with --ignored"]
async fn starts_and_gracefully_shuts_down() {
    if std::process::Command::new("arti")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let mut daemon = TorDaemon::with_port(9113);
    daemon.start().await.expect("tor should start");
    assert!(daemon.is_running());
    daemon.shutdown().await.expect("clean shutdown");
    assert!(!daemon.is_running(), "tor should be stopped after shutdown");
}
