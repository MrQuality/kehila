//! Controlled in-memory Hyper I/O failure, not a Windows/TLS reproduction.
use super::*;
use http_body_util::{BodyExt, Full};
use hyper::{service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite, DuplexStream, ReadBuf};

struct ResetOnShutdown(DuplexStream);

impl AsyncRead for ResetOnShutdown {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
    }
}

impl AsyncWrite for ResetOnShutdown {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Err(io::ErrorKind::ConnectionReset.into()))
    }
}

#[tokio::test]
async fn actual_hyper_shutdown_error_keeps_completed_exchange_and_io_category() {
    tokio::time::timeout(Duration::from_secs(2), async {
        let (client, server) = tokio::io::duplex(4096);
        let server = async {
            hyper::server::conn::http1::Builder::new()
                .keep_alive(false)
                .serve_connection(
                    TokioIo::new(server),
                    service_fn(|_| async {
                        Ok::<_, Infallible>(Response::new(Full::new(Bytes::from_static(b"ok"))))
                    }),
                )
                .await
                .unwrap();
        };
        let (mut sender, driver) =
            hyper::client::conn::http1::handshake(TokioIo::new(ResetOnShutdown(client)))
                .await
                .unwrap();
        let exchange = async move {
            let response = sender
                .send_request(
                    Request::get("/")
                        .header("Host", "fixture")
                        .body(Full::new(Bytes::new()))
                        .unwrap(),
                )
                .await
                .map_err(|_| ExchangeFailure::Request)?;
            let status = response.status();
            let body = response
                .into_body()
                .collect()
                .await
                .map_err(|_| ExchangeFailure::Body)?
                .to_bytes();
            drop(sender);
            Ok((status, body))
        };
        let (exchange, driver, ()) = tokio::join!(exchange, driver, server);
        assert!(driver.as_ref().unwrap_err().is_shutdown());
        let error = finish(exchange, driver).unwrap_err();
        let text = error.to_string();
        assert!(
            text.contains("Complete { status: 200, body_bytes: 2 }"),
            "{text}"
        );
        assert!(text.contains("ConnectionReset"), "{text}");
        assert!(text.contains("during_shutdown: true"), "{text}");
    })
    .await
    .expect("controlled Hyper I/O test deadline");
}
