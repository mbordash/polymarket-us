//! Dead-connection detection for the WebSocket stream.
//!
//! These run against a real local WebSocket server rather than a mock, because
//! the behaviour under test is precisely what happens at the socket level when a
//! server accepts a connection and then goes silent.

use futures_util::StreamExt;
use polymarket_us::{
    PolymarketUsStreamClient, ReconnectConfig, StreamConnectConfig, StreamControlEvent,
    StreamMessageKind, StreamSubscription,
};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;

/// Accept one WebSocket connection, then hold it open without ever sending a
/// frame — the silent-server case that used to hang the stream forever.
async fn spawn_silent_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                if let Ok(ws) = tokio_tungstenite::accept_async(socket).await {
                    // Drain client frames (the subscription) but never reply.
                    let (_write, mut read) = ws.split();
                    while read.next().await.is_some() {}
                }
            });
        }
    });

    format!("ws://{addr}")
}

#[tokio::test]
async fn idle_timeout_tears_down_a_silent_connection() {
    let url = spawn_silent_server().await;
    let client = PolymarketUsStreamClient::new(url, None);

    let config = StreamConnectConfig::default()
        .with_reconnect(ReconnectConfig::disabled())
        .with_idle_timeout(Some(Duration::from_millis(300)));

    let mut stream = client
        .connect_with_config(vec![StreamSubscription::heartbeat()], config)
        .await
        .expect("connect");

    let started = Instant::now();
    let mut saw_idle_error = false;

    while let Some(message) = stream.next().await {
        if let StreamMessageKind::Control(StreamControlEvent::Error(err)) = &message.kind {
            assert!(
                err.contains("idle"),
                "expected an idle-timeout error, got: {err}"
            );
            saw_idle_error = true;
        }
    }

    assert!(
        saw_idle_error,
        "stream closed without reporting the idle timeout"
    );
    // Fired on the timeout, not on some unrelated immediate failure.
    assert!(
        started.elapsed() >= Duration::from_millis(250),
        "idle timeout fired too early: {:?}",
        started.elapsed()
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "idle timeout did not fire promptly: {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn idle_timeout_triggers_reconnect_when_enabled() {
    let url = spawn_silent_server().await;
    let client = PolymarketUsStreamClient::new(url, None);

    let config = StreamConnectConfig::default()
        .with_reconnect(ReconnectConfig {
            enabled: true,
            max_attempts: Some(1),
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_millis(50),
            multiplier: 1.0,
        })
        .with_idle_timeout(Some(Duration::from_millis(200)));

    let mut stream = client
        .connect_with_config(vec![StreamSubscription::heartbeat()], config)
        .await
        .expect("connect");

    let mut saw_reconnecting = false;
    while let Some(message) = stream.next().await {
        if let StreamMessageKind::Control(StreamControlEvent::Reconnecting { .. }) = &message.kind {
            saw_reconnecting = true;
        }
    }

    assert!(
        saw_reconnecting,
        "a stalled connection should have driven a reconnect attempt"
    );
}

#[tokio::test]
async fn disabled_idle_timeout_leaves_a_silent_connection_open() {
    let url = spawn_silent_server().await;
    let client = PolymarketUsStreamClient::new(url, None);

    let config = StreamConnectConfig::default()
        .with_reconnect(ReconnectConfig::disabled())
        .with_idle_timeout(None);

    let mut stream = client
        .connect_with_config(vec![StreamSubscription::heartbeat()], config)
        .await
        .expect("connect");

    // With detection off, nothing should arrive and the stream should stay open.
    let result = tokio::time::timeout(Duration::from_millis(600), async {
        while let Some(message) = stream.next().await {
            if let StreamMessageKind::Control(StreamControlEvent::Error(err)) = &message.kind {
                return Some(err.clone());
            }
        }
        None
    })
    .await;

    assert!(
        result.is_err(),
        "expected the stream to stay open, but it produced: {result:?}"
    );
}
