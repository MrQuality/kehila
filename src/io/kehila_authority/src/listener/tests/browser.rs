//! Multi-connection test fixture; protected startup/publication are excluded.
use super::*;

pub(crate) struct BrowserListener {
    pub(crate) ready: oneshot::Receiver<()>,
    pub(crate) stop: oneshot::Sender<()>,
    pub(crate) running: tokio::task::JoinHandle<ListenerResult>,
}

pub(crate) struct ListenerResult {
    pub(crate) served: io::Result<()>,
    pub(crate) forced_connections: bool,
}

pub(crate) fn browser_listener(
    jobs: jobs::Handle,
    address: std::net::SocketAddrV4,
    certificate: Vec<u8>,
    key: Vec<u8>,
) -> BrowserListener {
    let (ready, observed) = oneshot::channel();
    let (stop, mut stopped) = oneshot::channel();
    let running = tokio::spawn(async move {
        let mut forced_connections = false;
        let served = async {
        let tls = crate::server_config(
            certificate.into(),
            PrivatePkcs8KeyDer::from(key).into(),
        ).map_err(|_| crate::io_error("browser fixture TLS rejected"))?;
        let public = std::net::SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, 16291);
        if address.port() != public.port() {
            return Err(crate::io_error("browser fixture origin mismatch"));
        }
        let profile = Arc::new(Profile::new(public.into())
            .map_err(|_| crate::io_error("browser fixture origin rejected"))?);
        let authority: Arc<str> = public.to_string().into();
        let utc = startup::utc()
            .map_err(|_| crate::io_error("browser fixture clock unavailable"))?;
        let end = rustls::pki_types::UnixTime::since_unix_epoch(Duration::from_secs(
            utc.as_secs() + 900,
        ));
        let window = Window::new(utc, end, Some(utc), Instant::now())
            .ok_or_else(|| crate::io_error("browser fixture clock unavailable"))?;
        let listener = TcpListener::bind(address).await?;
        let acceptor = TlsAcceptor::from(tls);
        let mut tasks = JoinSet::new();
        let _ = ready.send(());
        let served: io::Result<()> = async {
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stopped => return Ok(()),
                    result = tasks.join_next(), if !tasks.is_empty() => {
                        // A disconnected browser connection is an expected case.
                        // A panicked connection owner is a fixture failure.
                        result.unwrap().map_err(|_| crate::io_error("browser connection panicked"))?;
                    }
                    accepted = listener.accept() => {
                        let (socket, _) = accepted?;
                        if tasks.len() >= CONNECTION_LIMIT { drop(socket); continue; }
                        tasks.spawn(connection(socket, acceptor.clone(), profile.clone(),
                            jobs.clone(), window, Instant::now() + CONNECTION_DEADLINE,
                            authority.clone()));
                    }
                }
            }
        }.await;
        drop(listener);
        // The fixture's owner stops jobs as well and joins their owned work.
        let joined = timeout(CONNECTION_DEADLINE, async {
            let mut panicked = false;
            while let Some(result) = tasks.join_next().await {
                panicked |= result.is_err();
            }
            if panicked { Err(crate::io_error("browser connection panicked")) }
            else { Ok::<_, io::Error>(()) }
        }).await;
        if joined.is_err() {
            forced_connections = true;
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
            return Err(crate::io_error("browser connections required cancellation"));
        }
        joined.unwrap()?;
        served
        }.await;
        ListenerResult {
            served,
            forced_connections,
        }
    });
    BrowserListener {
        ready: observed,
        stop,
        running,
    }
}
