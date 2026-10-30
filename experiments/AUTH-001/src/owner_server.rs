//! Bounded disposable Linux owner listener; no supported installation contract.
mod clock;
#[cfg(test)]
pub(crate) mod tests;
mod ui;

use crate::{
    application::jobs,
    database::Database,
    http_auth::{adapter, Profile},
    secret::Secret,
    startup::{self, Arguments, Assets},
    Observations, Outcome, Rejection, CONNECTION_DEADLINE, CONNECTION_LIMIT, HANDSHAKE_DEADLINE,
};
use clock::Window;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use std::{
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpSocket, TcpStream},
    sync::oneshot,
    task::JoinSet,
    time::{timeout, timeout_at, Instant as TokioInstant},
};
use tokio_rustls::{server::TlsStream, TlsAcceptor};

type Retained = Arc<Mutex<Option<adapter::Exchange>>>;

struct Guarded {
    tls: TlsStream<TcpStream>,
    exchange: Retained,
}
impl Guarded {
    fn check(&self) -> io::Result<()> {
        self.exchange
            .lock()
            .map_err(|_| crate::io_error("reply ownership failed"))?
            .as_ref()
            .map(adapter::Exchange::check_outbound)
            .transpose()
            .map(|_| ())
            .map_err(|_| crate::io_error("reply eligibility ended"))
    }

    async fn close(&mut self) -> Result<(), Rejection> {
        // Send TLS closure without TCP FIN: a forwarding transport may stop
        // relaying the peer's close notification after observing a half-close.
        // Retain the reply gate and original deadline through both directions.
        self.tls.get_mut().1.send_close_notify();
        self.flush().await.map_err(|_| Rejection::TlsShutdown)?;
        let mut unexpected = [0];
        if !matches!(self.read(&mut unexpected).await, Ok(0)) {
            return Err(Rejection::PeerClose);
        }
        self.shutdown().await.map_err(|_| Rejection::TlsShutdown)?;
        Ok(())
    }
}
impl AsyncRead for Guarded {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // TLS reads can flush pending records, so retain the same delivery gate.
        if let Err(error) = self.check() {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut self.tls).poll_read(cx, buf)
    }
}
impl AsyncWrite for Guarded {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Err(error) = self.check() {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut self.tls).poll_write(cx, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(error) = self.check() {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut self.tls).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(error) = self.check() {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut self.tls).poll_shutdown(cx)
    }
}

fn bind(arguments: &Arguments) -> io::Result<TcpListener> {
    use std::io::Read;
    // Missing/unknown sysctl state is failure. Exact bind then proves kernel
    // locality, not interface assignment: the launcher must verify assignment.
    let mut value = Vec::new();
    std::fs::File::open("/proc/sys/net/ipv4/ip_nonlocal_bind")?
        .take(16)
        .read_to_end(&mut value)?;
    if value != b"0\n" && value != b"0" {
        return Err(crate::io_error("local binding policy unavailable"));
    }
    let socket = TcpSocket::new_v4()?;
    socket.bind(arguments.listen.into())?;
    socket.listen(CONNECTION_LIMIT as u32)
}

async fn connection(
    socket: TcpStream,
    acceptor: TlsAcceptor,
    profile: Arc<Profile>,
    jobs: jobs::Handle,
    window: Window,
    deadline: Instant,
    authority: Arc<str>,
) -> Outcome {
    let work = async {
        if !window.open(startup::utc().ok(), Instant::now()) {
            jobs.stop();
            return Outcome::Rejected(Rejection::Tls);
        }
        let tls = match timeout(HANDSHAKE_DEADLINE, acceptor.accept(socket)).await {
            Ok(Ok(tls)) => tls,
            Ok(Err(_)) => return Outcome::Rejected(Rejection::Tls),
            Err(_) => return Outcome::TimedOut,
        };
        let exchange: Retained = Arc::new(Mutex::new(None));
        let retain = exchange.clone();
        let service = service_fn(move |request| {
            let jobs = jobs.clone();
            let profile = profile.clone();
            let retain = retain.clone();
            let authority = authority.clone();
            async move {
                if let Some(response) = ui::response(&request, &authority) {
                    return Ok::<_, io::Error>(response);
                }
                let (response, reply) = adapter::handle(request, &profile, &jobs, deadline).await;
                *retain
                    .lock()
                    .map_err(|_| crate::io_error("reply ownership failed"))? = Some(reply);
                Ok::<_, io::Error>(response)
            }
        });
        let mut http = hyper::server::conn::http1::Builder::new();
        http.keep_alive(false)
            .max_headers(32)
            .max_buf_size(8192)
            .timer(TokioTimer::new())
            .header_read_timeout(HANDSHAKE_DEADLINE);
        let parts = match http
            .serve_connection(
                TokioIo::new(Guarded {
                    tls,
                    exchange: exchange.clone(),
                }),
                service,
            )
            .without_shutdown()
            .await
        {
            Ok(parts) => parts,
            Err(_) => return Outcome::Rejected(Rejection::Http),
        };
        let mut io = parts.io.into_inner();
        if !parts.read_buf.is_empty() {
            return Outcome::Rejected(Rejection::PeerClose);
        }
        if let Err(rejection) = io.close().await {
            return Outcome::Rejected(rejection);
        }
        let reply = match exchange.lock() {
            Ok(mut retained) => retained.take(),
            Err(_) => return Outcome::Rejected(Rejection::Http),
        };
        if reply.is_some_and(|reply| reply.confirm().is_err()) {
            return Outcome::Rejected(Rejection::Http);
        }
        Outcome::Complete
    };
    match timeout_at(TokioInstant::from_std(deadline), work).await {
        Ok(outcome) => outcome,
        Err(_) => Outcome::TimedOut,
    }
}

pub(crate) async fn run(
    arguments: Vec<String>,
    mut stop: oneshot::Receiver<()>,
) -> io::Result<Observations> {
    let arguments =
        Arguments::parse(arguments).map_err(|_| crate::io_error("startup arguments rejected"))?;
    let Assets {
        endpoint,
        installation,
        blocklist,
        tls,
        not_before,
        not_after,
    } = startup::prepare().map_err(|_| crate::io_error("startup assets rejected"))?;
    let window = Window::new(not_before, not_after, startup::utc().ok(), Instant::now())
        .ok_or_else(|| crate::io_error("startup clock rejected"))?;
    let profile = Arc::new(
        Profile::new(arguments.public.into())
            .map_err(|_| crate::io_error("public origin rejected"))?,
    );
    let authority: Arc<str> = arguments.public.to_string().into();
    let attempt = Secret::generate().map_err(|_| crate::io_error("startup entropy unavailable"))?;
    let attempt = std::str::from_utf8(attempt.encoded())
        .map_err(|_| crate::io_error("startup identifier rejected"))?
        .to_owned();
    let database = Database::start(
        &endpoint,
        installation,
        attempt,
        Instant::now() + CONNECTION_DEADLINE,
    )
    .await
    .map_err(|_| crate::io_error("startup database unavailable"))?;
    let live = database.liveness();
    let (jobs, owner) = jobs::Owner::new(database, blocklist)
        .await
        .map_err(|_| crate::io_error("startup coordinator unavailable"))?;
    let owner = owner.run();
    tokio::pin!(owner);
    let listener = match if window.open(startup::utc().ok(), Instant::now()) {
        bind(&arguments)
    } else {
        Err(crate::io_error("pre-listener clock rejected"))
    } {
        Ok(listener) => listener,
        Err(error) => {
            jobs.stop();
            let _ = owner.await;
            return Err(error);
        }
    };
    let acceptor = TlsAcceptor::from(tls);
    let mut tasks = JoinSet::new();
    let mut observed = Observations::default();
    let mut completed_owner = None;
    let mut failed = false;
    let mut clock = tokio::time::interval(Duration::from_secs(1));
    clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            result = &mut owner => { completed_owner = Some(result); break; }
            _ = &mut stop => break,
            _ = clock.tick() => {
                if !window.open(startup::utc().ok(), Instant::now()) { break; }
            }
            result = tasks.join_next(), if !tasks.is_empty() => {
                match result {
                    Some(Ok(outcome)) => crate::observe(outcome, &mut observed),
                    _ => { failed = true; break; }
                }
            }
            accepted = listener.accept() => {
                let (socket, _) = match accepted {
                    Ok(value) => value,
                    Err(_) => { failed = true; break; }
                };
                if tasks.len() >= CONNECTION_LIMIT {
                    observed.capacity_rejected += 1; drop(socket); continue;
                }
                observed.accepted += 1;
                let deadline = Instant::now() + CONNECTION_DEADLINE;
                tasks.spawn(connection(socket, acceptor.clone(), profile.clone(), jobs.clone(), window, deadline, authority.clone()));
            }
        }
    }
    drop(listener);
    let stop_deadline = live.shutdown_deadline();
    jobs.stop();
    let settle = async {
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(outcome) => crate::observe(outcome, &mut observed),
                Err(_) => failed = true,
            }
        }
    };
    let finish_owner = async {
        match completed_owner {
            Some(result) => result,
            None => owner.await,
        }
    };
    let (_, result) = tokio::join!(settle, finish_owner);
    if failed || result.is_err() || Instant::now() > stop_deadline {
        return Err(crate::io_error("owned shutdown failed"));
    }
    Ok(observed)
}
