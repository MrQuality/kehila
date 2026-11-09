//! Retained setup for synthetic capacity tests. Every request still uses a new
//! verified TLS connection and the unchanged production connection handler.
use super::*;

#[derive(Clone)]
pub(crate) struct Client {
    address: std::net::SocketAddr,
    config: Arc<rustls::ClientConfig>,
}

#[must_use = "the fixture must explicitly stop and join its server"]
pub(crate) struct Server {
    stop: Option<oneshot::Sender<()>>,
    running: Option<tokio::task::JoinHandle<io::Result<ConnectionMetrics>>>,
    window: Window,
}

pub(crate) async fn start(jobs: jobs::Handle) -> io::Result<(Client, Server)> {
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()])
            .map_err(|_| crate::io_error("capacity certificate setup failed"))?;
    let acceptor = TlsAcceptor::from(
        crate::server_config(
            cert.der().clone(),
            PrivatePkcs8KeyDer::from(signing_key.serialize_der()).into(),
        )
        .map_err(|_| crate::io_error("capacity TLS setup failed"))?,
    );
    let config = crate::client_config(Some(cert.der().clone()))
        .map_err(|_| crate::io_error("capacity trust setup failed"))?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let profile = Arc::new(
        Profile::new(address).map_err(|_| crate::io_error("capacity origin setup failed"))?,
    );
    let utc = startup::utc().map_err(|_| crate::io_error("capacity clock setup failed"))?;
    // Fixed test validity, separate from request and external execution budgets.
    // It is never renewed during a cell. Expiry invalidates the fixture.
    let end = utc
        .as_secs()
        .checked_add(900)
        .ok_or_else(|| crate::io_error("capacity validity overflow"))?;
    let end = rustls::pki_types::UnixTime::since_unix_epoch(Duration::from_secs(end));
    let started = Instant::now();
    let window = Window::new(utc, end, Some(utc), started)
        .ok_or_else(|| crate::io_error("capacity validity setup failed"))?;
    let (stop, mut stopped) = oneshot::channel();
    let running = tokio::spawn(async move {
        let mut tasks = JoinSet::new();
        let mut observed = ConnectionMetrics::default();
        let mut failure = None;
        loop {
            tokio::select! {
                biased;
                _ = &mut stopped => break,
                result = tasks.join_next(), if !tasks.is_empty() => {
                    match result {
                        Some(Ok(outcome)) => crate::observe(outcome, &mut observed),
                        _ => { failure = Some(crate::io_error("capacity connection task failed")); break; }
                    }
                }
                accepted = listener.accept() => {
                    let (socket, _) = match accepted {
                        Ok(value) => value,
                        Err(error) => { failure = Some(error); break; }
                    };
                    if tasks.len() >= CONNECTION_LIMIT {
                        observed.capacity_rejected += 1;
                        drop(socket);
                        continue;
                    }
                    observed.accepted += 1;
                    tasks.spawn(connection(socket, acceptor.clone(), profile.clone(), jobs.clone(),
                        window, Instant::now() + CONNECTION_DEADLINE, address.to_string().into()));
                }
            }
        }
        drop(listener);
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(outcome) => crate::observe(outcome, &mut observed),
                Err(_) => {
                    if failure.is_none() {
                        failure = Some(crate::io_error("capacity connection drain failed"));
                    }
                }
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(observed),
        }
    });
    Ok((
        Client { address, config },
        Server {
            stop: Some(stop),
            running: Some(running),
            window,
        },
    ))
}

impl Client {
    pub(crate) fn untrusted_for_control(&self) -> io::Result<Self> {
        Ok(Self {
            address: self.address,
            config: crate::client_config(None)
                .map_err(|_| crate::io_error("capacity negative trust setup failed"))?,
        })
    }

    /// Includes the real connection driver's cleanup. Callers validate the
    /// returned body before ending their client latency observation.
    pub(crate) async fn request(
        &self,
        mut request: hyper::Request<Full<bytes::Bytes>>,
    ) -> io::Result<(hyper::StatusCode, hyper::HeaderMap, bytes::Bytes)> {
        let host = self.address.to_string();
        request.headers_mut().insert(
            "host",
            host.parse()
                .map_err(|_| crate::io_error("capacity request authority invalid"))?,
        );
        request.headers_mut().insert(
            "connection",
            hyper::header::HeaderValue::from_static("close"),
        );
        if request
            .headers()
            .get("x-kehila-client")
            .is_some_and(|value| value == "browser")
        {
            request.headers_mut().insert(
                "origin",
                format!("https://{host}")
                    .parse()
                    .map_err(|_| crate::io_error("capacity request origin invalid"))?,
            );
        }
        crate::test_exchange::request_with_headers(
            self.address,
            self.config.clone(),
            ServerName::IpAddress(self.address.ip().into()),
            request,
        )
        .await
    }
}

impl Server {
    pub(crate) async fn finish(mut self) -> io::Result<ConnectionMetrics> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        // Keep ownership in self across await: cancelling finish invokes the
        // abort fallback, rather than detaching a locally moved JoinHandle.
        let result = self
            .running
            .as_mut()
            .ok_or_else(|| crate::io_error("capacity server already joined"))?
            .await;
        self.running.take();
        let observed = result.map_err(|_| crate::io_error("capacity server owner failed"))??;
        if !self.window.open(startup::utc().ok(), Instant::now()) {
            return Err(crate::io_error("capacity fixture validity window expired"));
        }
        Ok(observed)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(running) = &self.running {
            // Unqualified fallback only. Drop never establishes observed joins.
            running.abort();
        }
    }
}
