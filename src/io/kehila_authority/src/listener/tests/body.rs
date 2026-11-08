//! Socket-level body admission into the native adapter, not protected startup.
use super::*;
use hyper::body::{Body, Frame, Incoming};
use std::future::Future;
use tokio::sync::oneshot;

struct Observed {
    incoming: Incoming,
    waiting: Option<oneshot::Sender<()>>,
}
impl Body for Observed {
    type Data = bytes::Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let result = Pin::new(&mut self.incoming).poll_frame(cx);
        if result.is_pending() {
            if let Some(waiting) = self.waiting.take() {
                let _ = waiting.send(());
            }
        }
        result
    }
}

/// The complete control and incomplete case use the same service and envelope.
/// `during` runs only after the admitted adapter actually waits on socket data.
pub(crate) async fn exchange<F: Future<Output = ()>>(
    jobs: jobs::Handle,
    request: hyper::Request<Full<bytes::Bytes>>,
    during: Option<F>,
) -> (hyper::StatusCode, bytes::Bytes) {
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let acceptor = TlsAcceptor::from(
        crate::server_config(
            cert.der().clone(),
            PrivatePkcs8KeyDer::from(signing_key.serialize_der()).into(),
        )
        .unwrap(),
    );
    let client = crate::client_config(Some(cert.der().clone())).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let profile = Arc::new(Profile::new(address).unwrap());
    let deadline = Instant::now() + CONNECTION_DEADLINE;
    let (waiting, pending) = oneshot::channel();
    let (returned, mut completed) = oneshot::channel();
    let signals = Arc::new(Mutex::new(Some((waiting, returned))));
    let server = async {
        let (socket, _) = listener.accept().await.unwrap();
        let tls = acceptor.accept(socket).await.unwrap();
        let retained: Retained = Arc::new(Mutex::new(None));
        let retain = retained.clone();
        let service = service_fn(move |request: hyper::Request<Incoming>| {
            let jobs = jobs.clone();
            let profile = profile.clone();
            let retain = retain.clone();
            let (waiting, returned) = signals.lock().unwrap().take().unwrap();
            async move {
                let request = request.map(|incoming| Observed {
                    incoming,
                    waiting: Some(waiting),
                });
                let (response, reply) = adapter::handle(request, &profile, &jobs, deadline).await;
                // This observes adapter completion, before any outbound/close work.
                returned.send(response.status()).unwrap();
                *retain.lock().unwrap() = Some(reply);
                Ok::<_, io::Error>(response)
            }
        });
        let parts = hyper::server::conn::http1::Builder::new()
            .keep_alive(false)
            .serve_connection(
                TokioIo::new(Guarded {
                    tls,
                    exchange: retained.clone(),
                }),
                service,
            )
            .without_shutdown()
            .await
            .unwrap();
        let mut io = parts.io.into_inner();
        assert!(parts.read_buf.is_empty());
        io.flush().await.unwrap();
        retained
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .confirm()
            .unwrap();
        assert!(io.close().await.is_ok(), "owned TLS closure failed");
    };
    let client = async {
        let socket = TcpStream::connect(address).await.unwrap();
        let mut tls = TlsConnector::from(client)
            .connect(ServerName::IpAddress(address.ip().into()), socket)
            .await
            .unwrap();
        let (mut parts, body) = request.into_parts();
        let body = body.collect().await.unwrap().to_bytes();
        assert!(!body.is_empty());
        parts
            .headers
            .insert("host", address.to_string().parse().unwrap());
        parts
            .headers
            .insert("origin", format!("https://{address}").parse().unwrap());
        parts.headers.insert("connection", "close".parse().unwrap());
        let mut wire = format!("{} {} HTTP/1.1\r\n", parts.method, parts.uri).into_bytes();
        for (name, value) in &parts.headers {
            wire.extend_from_slice(format!("{name}: ").as_bytes());
            wire.extend_from_slice(value.as_bytes());
            wire.extend_from_slice(b"\r\n");
        }
        wire.extend_from_slice(b"\r\n");
        let length = body.len() - usize::from(during.is_some());
        wire.extend_from_slice(&body[..length]);
        tls.write_all(&wire).await.unwrap();
        if let Some(during) = during {
            pending.await.unwrap();
            assert!(matches!(
                completed.try_recv(),
                Err(oneshot::error::TryRecvError::Empty)
            ));
            during.await;
            assert!(
                matches!(
                    completed.try_recv(),
                    Err(oneshot::error::TryRecvError::Empty)
                ),
                "adapter completed before concurrent native read"
            );
        }
        let status = completed.await.unwrap();
        let mut response = Vec::new();
        tls.read_to_end(&mut response).await.unwrap();
        tls.shutdown().await.unwrap();
        assert!(response.starts_with(format!("HTTP/1.1 {} ", status.as_u16()).as_bytes()));
        let end = response
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .unwrap()
            + 4;
        (status, bytes::Bytes::copy_from_slice(&response[end..]))
    };
    // Both futures are owned here; outer failure drops both sockets and work.
    let ((), result) = tokio::time::timeout_at(TokioInstant::from_std(deadline), async {
        tokio::join!(server, client)
    })
    .await
    .unwrap();
    result
}
