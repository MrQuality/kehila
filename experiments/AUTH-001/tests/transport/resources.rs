use std::{io, time::Duration};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, StatusCode};
use kehila_local_tls_probe::{request, CONNECTION_DEADLINE, CONNECTION_LIMIT};
use rustls::pki_types::ServerName;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

use super::{Authority, Running};

async fn tls(running: &Running, ca: &Authority) -> tokio_rustls::client::TlsStream<TcpStream> {
    let socket = TcpStream::connect(running.address).await.unwrap();
    tokio_rustls::TlsConnector::from(ca.client())
        .connect(ServerName::from(running.address.ip()), socket)
        .await
        .unwrap()
}

async fn closed(socket: &mut (impl AsyncRead + Unpin), deadline: Duration) {
    let mut byte = [0];
    match timeout(deadline, socket.read(&mut byte))
        .await
        .expect("peer closure deadline")
    {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset
                    | io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::ConnectionAborted
            ) => {}
        other => panic!("expected a closed transport, got {other:?}"),
    }
}

#[tokio::test]
async fn excess_connections_are_rejected_and_shutdown_joins_stalled_peers() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let mut stalled = Vec::new();
    for _ in 0..CONNECTION_LIMIT {
        stalled.push(TcpStream::connect(running.address).await.unwrap());
        tokio::task::yield_now().await;
    }
    let mut excess = TcpStream::connect(running.address).await.unwrap();
    closed(&mut excess, Duration::from_secs(1)).await;
    let observed = timeout(
        CONNECTION_DEADLINE + Duration::from_secs(1),
        running.finish(),
    )
    .await
    .expect("shutdown must join bounded connection work");
    assert_eq!(observed.accepted, CONNECTION_LIMIT);
    assert!(observed.capacity_rejected >= 1);
    assert!(observed.timed_out >= 1);
    drop(stalled);
}

#[tokio::test]
async fn idle_tls_handshake_expires() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let mut socket = TcpStream::connect(running.address).await.unwrap();
    closed(&mut socket, Duration::from_secs(3)).await;
    assert!(running.finish().await.timed_out >= 1);
}

#[tokio::test]
async fn idle_http_headers_expire_after_valid_tls() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let mut socket = tls(&running, &ca).await;
    closed(&mut socket, Duration::from_secs(3)).await;
    assert!(running.finish().await.protocol_rejected >= 1);
}

#[tokio::test]
async fn incomplete_body_expires_under_total_connection_deadline() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let mut socket = tls(&running, &ca).await;
    socket.write_all(format!("POST /probe HTTP/1.1\r\nHost: {0}\r\nOrigin: https://{0}\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n", running.address).as_bytes()).await.unwrap();
    closed(&mut socket, CONNECTION_DEADLINE + Duration::from_secs(1)).await;
    assert!(running.finish().await.timed_out >= 1);
}

#[tokio::test]
async fn plaintext_cannot_receive_the_https_success_response() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let mut socket = TcpStream::connect(running.address).await.unwrap();
    socket
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .await
        .unwrap();
    let mut response = [0; 7];
    let received = timeout(Duration::from_secs(3), socket.read(&mut response))
        .await
        .unwrap();
    match received {
        Ok(0) => {}
        Ok(length) => {
            timeout(
                Duration::from_secs(1),
                socket.read_exact(&mut response[length..]),
            )
            .await
            .expect("bounded TLS alert completion")
            .expect("complete TLS alert");
            assert_eq!(response[0], 21, "TLS alert record type");
            assert_eq!(response[1], 3, "TLS record version");
            assert!(response[2] <= 3);
            assert_eq!(&response[3..6], &[0, 2, 2], "two-byte fatal alert");
            closed(&mut socket, Duration::from_secs(1)).await;
        }
        Err(error) => assert!(matches!(
            error.kind(),
            io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
        )),
    }
    assert!(running.finish().await.protocol_rejected >= 1);
}

#[tokio::test]
async fn oversized_body_is_rejected_after_valid_tls() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let host = running.address.to_string();
    let body = Request::post("/probe")
        .header("Host", &host)
        .header("Origin", format!("https://{host}"))
        .header("Content-Type", "application/json")
        .header("Content-Length", "3")
        .body(Full::new(Bytes::from_static(b"{}x")))
        .unwrap();
    assert_eq!(
        request(
            running.address,
            ca.client(),
            ServerName::from(running.address.ip()),
            body
        )
        .await
        .unwrap()
        .0,
        StatusCode::BAD_REQUEST
    );
    running.finish().await;
}
