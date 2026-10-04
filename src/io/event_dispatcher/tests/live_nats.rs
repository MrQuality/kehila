//! Always runs against live infrastructure; unavailable infrastructure is a failure.
use event_dispatcher::JetStreamConnection;
use std::time::Duration;

#[test]
fn live_nats_handshake_and_second_round_trip() {
    let address = std::env::var("KEHILA_NATS_ADDRESS").unwrap_or_else(|_| "127.0.0.1:4222".into());
    let mut connection = JetStreamConnection::connect(
        address
            .parse()
            .expect("KEHILA_NATS_ADDRESS must be an IP:port"),
        Duration::from_secs(3),
    )
    .expect("live NATS required: python scripts/setup_env.py --start");
    connection
        .ping()
        .expect("second physical broker round trip");
}
