//! Dead-connection detection for the WebSocket streams.
//!
//! These run against a real local WebSocket server rather than a mock, because
//! the behaviour under test is precisely what happens at the socket level when
//! a peer stops responding.
//!
//! Note the distinction the two fake servers draw. A peer that still answers
//! pings is *alive* even if it has nothing to say — a quiet market is not a
//! broken connection — so only the peer that answers nothing at all should trip
//! the idle timeout.

use futures_util::StreamExt;
use polymarket_us::{
    MarketStreamClient, MarketSubscription, ReconnectConfig, StreamConnectConfig,
    StreamControlEvent, StreamMessageKind,
};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;

fn subscription() -> MarketSubscription {
    MarketSubscription::market_data(["test-market"])
}

/// Accept one WebSocket connection and then stop servicing it entirely — no
/// data, and no pong, because the stream is never polled. This is the silently
/// dead socket that used to hang the stream forever.
async fn spawn_dead_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                if let Ok(ws) = tokio_tungstenite::accept_async(socket).await {
                    // Hold the connection open but never poll it, so the
                    // client's keepalive pings go unanswered.
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                    drop(ws);
                }
            });
        }
    });

    format!("ws://{addr}")
}

/// Accept one WebSocket connection and keep reading from it — which answers
/// pings automatically — but never send an application frame.
async fn spawn_quiet_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                if let Ok(mut ws) = tokio_tungstenite::accept_async(socket).await {
                    while ws.next().await.is_some() {}
                }
            });
        }
    });

    format!("ws://{addr}")
}

#[tokio::test]
async fn idle_timeout_tears_down_a_dead_connection() {
    let url = spawn_dead_server().await;
    let client = MarketStreamClient::with_base_url(url, None);

    let config = StreamConnectConfig::default()
        .with_reconnect(ReconnectConfig::disabled())
        .with_idle_timeout(Some(Duration::from_millis(300)));

    let mut stream = client
        .connect_with_config(vec![subscription()], config)
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
    let url = spawn_dead_server().await;
    let client = MarketStreamClient::with_base_url(url, None);

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
        .connect_with_config(vec![subscription()], config)
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
async fn disabled_idle_timeout_leaves_a_dead_connection_open() {
    let url = spawn_dead_server().await;
    let client = MarketStreamClient::with_base_url(url, None);

    let config = StreamConnectConfig::default()
        .with_reconnect(ReconnectConfig::disabled())
        .with_idle_timeout(None);

    let mut stream = client
        .connect_with_config(vec![subscription()], config)
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

#[tokio::test]
async fn keepalive_keeps_a_quiet_connection_alive() {
    let url = spawn_quiet_server().await;
    let client = MarketStreamClient::with_base_url(url, None);

    // The idle window is far shorter than anything this server will ever send,
    // so the connection survives only if the pings are drawing pongs.
    let config = StreamConnectConfig::default()
        .with_reconnect(ReconnectConfig::disabled())
        .with_idle_timeout(Some(Duration::from_millis(400)))
        .with_keepalive_interval(Some(Duration::from_millis(100)));

    let mut stream = client
        .connect_with_config(vec![subscription()], config)
        .await
        .expect("connect");

    let result = tokio::time::timeout(Duration::from_millis(1200), async {
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
        "a market with nothing to report should not look like a dead socket, but got: {result:?}"
    );
}

#[tokio::test]
async fn a_quiet_connection_dies_without_keepalive() {
    let url = spawn_quiet_server().await;
    let client = MarketStreamClient::with_base_url(url, None);

    // Same server, keepalive off: now there is no traffic at all to feed the
    // idle check, so it fires. This is what pins the previous test's result on
    // the keepalive rather than on the server happening to say something.
    let config = StreamConnectConfig::default()
        .with_reconnect(ReconnectConfig::disabled())
        .with_idle_timeout(Some(Duration::from_millis(300)))
        .with_keepalive_interval(None);

    let mut stream = client
        .connect_with_config(vec![subscription()], config)
        .await
        .expect("connect");

    let mut saw_idle_error = false;
    while let Some(message) = stream.next().await {
        if let StreamMessageKind::Control(StreamControlEvent::Error(err)) = &message.kind {
            assert!(err.contains("idle"), "expected idle-timeout error: {err}");
            saw_idle_error = true;
        }
    }

    assert!(saw_idle_error, "stream closed without an idle timeout");
}
