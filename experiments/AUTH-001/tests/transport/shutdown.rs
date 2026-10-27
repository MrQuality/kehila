use super::{Authority, Running};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use kehila_local_tls_probe::{CONNECTION_DEADLINE, CONNECTION_LIMIT};
use rustls::pki_types::ServerName;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::TcpStream,
    time::{timeout, Sleep},
};

// Delay TCP half-close after rustls has flushed its closure alert. No injected error.
struct DelayedShutdown {
    socket: TcpStream,
    delay: Option<Pin<Box<Sleep>>>,
}

impl AsyncRead for DelayedShutdown {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().socket).poll_read(cx, buf)
    }
}

impl AsyncWrite for DelayedShutdown {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().socket).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().socket).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let delay = this
            .delay
            .get_or_insert_with(|| Box::pin(tokio::time::sleep(Duration::from_millis(10))));
        if std::future::Future::poll(delay.as_mut(), cx).is_pending() {
            return Poll::Pending;
        }
        Pin::new(&mut this.socket).poll_shutdown(cx)
    }
}

async fn exchange(
    mut sender: hyper::client::conn::http1::SendRequest<Full<Bytes>>,
    host: String,
) -> Result<(StatusCode, Bytes), &'static str> {
    let request = Request::post("/probe")
        .header("Host", &host)
        .header("Origin", format!("https://{host}"))
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from_static(b"{}")))
        .unwrap();
    let response = sender
        .send_request(request)
        .await
        .map_err(|_| "request failed")?;
    let status = response.status();
    let body = http_body_util::Limited::new(response.into_body(), 1024)
        .collect()
        .await
        .map_err(|_| "response body failed")?
        .to_bytes();
    drop(sender);
    Ok((status, body))
}

#[tokio::test]
async fn delayed_tcp_shutdown_keeps_complete_response_and_clean_driver() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let work = async {
        let socket = TcpStream::connect(running.address).await.unwrap();
        let tls = tokio_rustls::TlsConnector::from(ca.client())
            .connect(
                ServerName::from(running.address.ip()),
                DelayedShutdown {
                    socket,
                    delay: None,
                },
            )
            .await
            .unwrap();
        let (sender, driver) = hyper::client::conn::http1::handshake(TokioIo::new(tls))
            .await
            .unwrap();
        tokio::join!(exchange(sender, running.address.to_string()), driver)
    };
    let (response, driver) = timeout(CONNECTION_DEADLINE, work)
        .await
        .expect("client deadline");
    let observed = running.finish_checked().await;
    let response_summary = response
        .as_ref()
        .map(|(status, body)| (status.as_u16(), body.len()));
    assert!(
        driver.is_ok(),
        "exchange={response_summary:?}; driver_shutdown={:?}; server={observed:?}",
        driver.as_ref().err().map(hyper::Error::is_shutdown)
    );
    assert_eq!(
        response.unwrap(),
        (
            StatusCode::OK,
            Bytes::from_static(b"{\"transport\":\"ready\"}")
        )
    );
    let observed = observed.unwrap();
    assert_eq!(
        (
            observed.completed,
            observed.protocol_rejected,
            observed.timed_out
        ),
        (1, 0, 0)
    );
}

async fn response_without_client_close(
    running: &Running,
    ca: &Authority,
) -> tokio_rustls::client::TlsStream<TcpStream> {
    timeout(CONNECTION_DEADLINE, async {
        let socket = TcpStream::connect(running.address).await.unwrap();
        let tls = tokio_rustls::TlsConnector::from(ca.client())
            .connect(ServerName::from(running.address.ip()), socket)
            .await
            .unwrap();
        let (sender, driver) = hyper::client::conn::http1::handshake(TokioIo::new(tls))
            .await
            .unwrap();
        let (response, parts) = tokio::join!(
            exchange(sender, running.address.to_string()),
            driver.without_shutdown()
        );
        assert_eq!(response.unwrap().0, StatusCode::OK);
        let parts = parts.unwrap();
        assert!(
            parts.read_buf.is_empty(),
            "no unconsumed HTTP bytes in closure fixture"
        );
        parts.io.into_inner()
    })
    .await
    .expect("response fixture deadline")
}

#[tokio::test]
async fn peer_without_close_notify_keeps_owned_task_until_total_deadline() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let peer = response_without_client_close(&running, &ca).await;
    let observed = timeout(
        CONNECTION_DEADLINE + Duration::from_secs(1),
        running.finish(),
    )
    .await
    .expect("silent-peer shutdown must settle owned task");
    assert_eq!(
        (observed.completed, observed.timed_out, observed.accepted),
        (0, 1, 1)
    );
    drop(peer);
}

#[tokio::test]
async fn application_data_after_response_is_rejected_during_peer_close() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let mut peer = response_without_client_close(&running, &ca).await;
    // A separate post-response TLS write cannot sit in Hyper's old read buffer.
    peer.write_all(b"x").await.unwrap();
    peer.flush().await.unwrap();
    let observed = running.finish().await;
    assert_eq!(
        (
            observed.completed,
            observed.protocol_rejected,
            observed.timed_out
        ),
        (0, 1, 0)
    );
    assert_eq!(observed.peer_close_rejected, 1);
}

#[tokio::test]
async fn tcp_eof_without_tls_close_notify_is_not_clean_completion() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let peer = response_without_client_close(&running, &ca).await;
    drop(peer);
    let observed = running.finish().await;
    assert_eq!(
        (
            observed.completed,
            observed.peer_close_rejected,
            observed.protocol_rejected
        ),
        (0, 1, 1)
    );
}

#[tokio::test]
async fn waiting_for_peer_closure_retains_connection_slots_and_owned_shutdown() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let peers = timeout(Duration::from_secs(2), async {
        let mut peers = Vec::new();
        for _ in 0..CONNECTION_LIMIT {
            peers.push(response_without_client_close(&running, &ca).await);
        }
        peers
    })
    .await
    .expect("all silent peers must be established before expiry");
    let mut excess = TcpStream::connect(running.address).await.unwrap();
    let mut byte = [0];
    match timeout(Duration::from_secs(1), excess.read(&mut byte))
        .await
        .expect("capacity rejection deadline")
    {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
            ) => {}
        _ => panic!("excess connection was not rejected"),
    }
    let observed = timeout(
        CONNECTION_DEADLINE + Duration::from_secs(1),
        running.finish(),
    )
    .await
    .expect("all owned peer-close tasks must settle");
    assert_eq!(observed.accepted, CONNECTION_LIMIT);
    assert_eq!(
        (observed.completed, observed.timed_out),
        (0, CONNECTION_LIMIT)
    );
    assert!(observed.capacity_rejected >= 1);
    drop(peers);
}
