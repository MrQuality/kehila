use std::net::{Ipv4Addr, SocketAddr};

use tokio::{
    net::TcpListener,
    sync::{Mutex, MutexGuard},
};

static NETWORK: Mutex<()> = Mutex::const_new(());

/// Serial ownership permits a caller-selected port without weakening the TLS tests.
pub struct Network {
    pub listener: TcpListener,
    pub serial: MutexGuard<'static, ()>,
}

impl Network {
    pub async fn bind() -> Self {
        let serial = NETWORK.lock().await;
        let port = match std::env::var("KEHILA_TLS_TEST_PORT") {
            Ok(value) => {
                let port: u16 = value.parse().expect("KEHILA_TLS_TEST_PORT must be a port");
                assert_ne!(port, 0, "omit KEHILA_TLS_TEST_PORT for an ephemeral port");
                port
            }
            Err(std::env::VarError::NotPresent) => 0,
            Err(error) => panic!("invalid KEHILA_TLS_TEST_PORT: {error}"),
        };
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
            .await
            .expect("test listener must bind its requested address");
        Self { listener, serial }
    }
}
