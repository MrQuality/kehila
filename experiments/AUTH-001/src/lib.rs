//! Disposable diagnostic transport and experimental credential/coordinator code.
//! No supported installation, production database or trust-store access.
#[cfg(target_os = "linux")]
#[expect(
    dead_code,
    reason = "Owned job dispatcher follows in the next focused commit"
)]
mod application;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "No caller until complete credential admission is qualified"
    )
)]
mod bundle;
#[cfg(target_os = "linux")]
#[expect(
    dead_code,
    reason = "Native database ownership awaits fixture qualification and HTTP wiring"
)]
mod database;
mod diagnostics;
mod lifecycle;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Credential core is not connected to HTTP before qualification"
    )
)]
mod password;
#[cfg_attr(
    all(not(test), not(target_os = "linux")),
    expect(
        dead_code,
        reason = "Pre-session state awaits fenced login integration"
    )
)]
mod pre_session;
#[cfg_attr(
    all(not(test), not(target_os = "linux")),
    expect(dead_code, reason = "Secret primitives await session integration")
)]
mod secret;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Registry candidates await fenced runtime integration"
    )
)]
mod session;

use std::{
    convert::Infallible,
    io,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{body::Incoming, header, service::service_fn, Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, ServerName},
    ClientConfig, ServerConfig,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinSet,
    time::timeout,
};
use tokio_rustls::{TlsAcceptor, TlsConnector};

pub const CONNECTION_LIMIT: usize = 16;
pub const CONNECTION_DEADLINE: Duration = Duration::from_secs(5);
const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(2);
const RESPONSE: &[u8] = b"{\"transport\":\"ready\"}";
const PAGE: &[u8] = b"<!doctype html><title>Kehila A0</title><p>HTTPS transport probe only. No login or project data.</p>";

fn io_error(message: &'static str) -> io::Error {
    io::Error::other(message)
}

fn require_localhost(address: SocketAddr) -> io::Result<()> {
    if address.ip() != Ipv4Addr::LOCALHOST {
        return Err(io_error("literal IPv4 loopback required"));
    }
    Ok(())
}

pub fn server_config(
    cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
) -> Result<Arc<ServerConfig>, rustls::Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

pub fn client_config(
    root: Option<CertificateDer<'static>>,
) -> Result<Arc<ClientConfig>, rustls::Error> {
    let mut roots = rustls::RootCertStore::empty();
    if let Some(root) = root {
        roots.add(root)?;
    }
    let mut config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_root_certificates(roots)
            .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

fn response(status: StatusCode, body: &'static [u8]) -> Response<Full<Bytes>> {
    let mut result = Response::new(Full::new(Bytes::from_static(body)));
    *result.status_mut() = status;
    for (name, value) in [
        (header::CONNECTION, "close"),
        (header::CACHE_CONTROL, "no-store"),
        (header::CONTENT_TYPE, "text/html; charset=utf-8"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; frame-ancestors 'none'; form-action 'none'",
        ),
    ] {
        result
            .headers_mut()
            .insert(name, header::HeaderValue::from_static(value));
    }
    result
}

fn one_header<B>(request: &Request<B>, name: header::HeaderName) -> Option<&str> {
    let mut values = request.headers().get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value)
}

fn admission<B>(request: &Request<B>, address: SocketAddr) -> Result<bool, StatusCode> {
    let host = address.to_string();
    if one_header(request, header::HOST) != Some(host.as_str())
        || request.uri().authority().is_some()
        || request.uri().query().is_some()
        || [
            "forwarded",
            "x-forwarded-host",
            "x-forwarded-proto",
            "x-forwarded-for",
            "authorization",
            "cookie",
            "transfer-encoding",
        ]
        .iter()
        .any(|name| request.headers().contains_key(*name))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let origin = format!("https://{host}");
    if request.headers().contains_key(header::ORIGIN)
        && one_header(request, header::ORIGIN) != Some(origin.as_str())
    {
        return Err(StatusCode::FORBIDDEN);
    }
    if request.method() == Method::GET && request.uri().path() == "/" {
        if request.headers().contains_key(header::CONTENT_LENGTH)
            && one_header(request, header::CONTENT_LENGTH) != Some("0")
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        return Ok(false);
    }
    if request.method() != Method::POST || request.uri().path() != "/probe" {
        return Err(StatusCode::NOT_FOUND);
    }
    if one_header(request, header::ORIGIN) != Some(origin.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }
    if one_header(request, header::CONTENT_TYPE) != Some("application/json")
        || one_header(request, header::CONTENT_LENGTH) != Some("2")
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(true)
}

async fn route(
    request: Request<Incoming>,
    address: SocketAddr,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let admitted = admission(&request, address);
    // Bound even rejection-body cleanup; the outer connection deadline owns cancellation.
    let body = Limited::new(request.into_body(), 2)
        .collect()
        .await
        .map(|body| body.to_bytes());
    let result = match (admitted, body) {
        (Ok(false), Ok(body)) if body.is_empty() => response(StatusCode::OK, PAGE),
        (Ok(true), Ok(body)) if body.as_ref() == b"{}" => {
            let mut result = response(StatusCode::OK, RESPONSE);
            result.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/json"),
            );
            result
        }
        (Err(status), _) => response(status, b"request rejected"),
        _ => response(StatusCode::BAD_REQUEST, b"request rejected"),
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_literal_ipv4_localhost_is_admitted() {
        assert!(require_localhost("127.0.0.1:1".parse().unwrap()).is_ok());
        for address in ["127.0.0.2:1", "[::1]:1", "192.0.2.1:1"] {
            assert!(require_localhost(address.parse().unwrap()).is_err());
        }
    }

    fn probe() -> Request<()> {
        Request::post("/probe")
            .header("Host", "127.0.0.1:16281")
            .header("Origin", "https://127.0.0.1:16281")
            .header("Content-Type", "application/json")
            .header("Content-Length", "2")
            .body(())
            .unwrap()
    }

    #[test]
    fn admission_requires_exact_authority_and_original_origin() {
        let address = "127.0.0.1:16281".parse().unwrap();
        assert_eq!(admission(&probe(), address), Ok(true));
        for origin in [
            None,
            Some("null"),
            Some("https://127.0.0.1:16282"),
            Some("https://foreign.invalid"),
        ] {
            let mut request = probe();
            request.headers_mut().remove(header::ORIGIN);
            if let Some(origin) = origin {
                request
                    .headers_mut()
                    .insert(header::ORIGIN, origin.parse().unwrap());
            }
            assert_eq!(admission(&request, address), Err(StatusCode::FORBIDDEN));
        }
        let mut duplicate = probe();
        duplicate
            .headers_mut()
            .append(header::ORIGIN, "https://127.0.0.1:16281".parse().unwrap());
        assert_eq!(admission(&duplicate, address), Err(StatusCode::FORBIDDEN));
    }

    #[test]
    fn admission_rejects_proxy_assertions_credentials_and_unbounded_body_framing() {
        let address = "127.0.0.1:16281".parse().unwrap();
        for (name, value) in [
            ("Host", "foreign.invalid"),
            ("Forwarded", "for=127.0.0.1"),
            ("X-Forwarded-Host", "127.0.0.1:16281"),
            ("Authorization", "Bearer fixture"),
            ("Cookie", "fixture=x"),
            ("Transfer-Encoding", "chunked"),
            ("Content-Length", "3"),
        ] {
            let mut request = probe();
            request.headers_mut().insert(
                header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
            assert_eq!(admission(&request, address), Err(StatusCode::BAD_REQUEST));
        }
        let mut duplicate = probe();
        duplicate
            .headers_mut()
            .append(header::HOST, "127.0.0.1:16281".parse().unwrap());
        assert_eq!(admission(&duplicate, address), Err(StatusCode::BAD_REQUEST));
    }
}

#[derive(Default, Debug)]
pub struct Observations {
    pub accepted: usize,
    pub capacity_rejected: usize,
    pub protocol_rejected: usize,
    pub timed_out: usize,
    pub completed: usize,
    pub tls_rejected: usize,
    pub http_rejected: usize,
    pub tls_shutdown_rejected: usize,
    pub peer_close_rejected: usize,
}

enum Rejection {
    Tls,
    Http,
    TlsShutdown,
    PeerClose,
}

enum Outcome {
    Complete,
    Rejected(Rejection),
    TimedOut,
}

fn observe(result: Outcome, observations: &mut Observations) {
    match result {
        Outcome::Complete => observations.completed += 1,
        Outcome::Rejected(stage) => {
            observations.protocol_rejected += 1;
            match stage {
                Rejection::Tls => observations.tls_rejected += 1,
                Rejection::Http => observations.http_rejected += 1,
                Rejection::TlsShutdown => observations.tls_shutdown_rejected += 1,
                Rejection::PeerClose => observations.peer_close_rejected += 1,
            }
        }
        Outcome::TimedOut => observations.timed_out += 1,
    }
}

#[test]
fn rejection_stages_preserve_aggregate_and_timeout_classification() {
    let mut observed = Observations::default();
    for stage in [
        Rejection::Tls,
        Rejection::Http,
        Rejection::TlsShutdown,
        Rejection::PeerClose,
    ] {
        observe(Outcome::Rejected(stage), &mut observed);
    }
    observe(Outcome::Complete, &mut observed);
    observe(Outcome::TimedOut, &mut observed);
    assert_eq!(observed.protocol_rejected, 4);
    assert_eq!(observed.peer_close_rejected, 1);
    assert_eq!(
        (
            observed.tls_rejected,
            observed.http_rejected,
            observed.tls_shutdown_rejected
        ),
        (1, 1, 1)
    );
    assert_eq!((observed.completed, observed.timed_out), (1, 1));
}

async fn close_tls(mut tls: tokio_rustls::server::TlsStream<TcpStream>) -> Outcome {
    if tls.shutdown().await.is_err() {
        return Outcome::Rejected(Rejection::TlsShutdown);
    }
    // Keep the read side owned until peer closure, under the outer deadline.
    // One unexpected application byte is rejection, never an unbounded drain.
    let mut byte = [0];
    match tls.read(&mut byte).await {
        Ok(0) => Outcome::Complete,
        _ => Outcome::Rejected(Rejection::PeerClose),
    }
}

/// Runs at most three minutes, then settles every accepted connection before returning.
pub async fn serve(
    listener: TcpListener,
    config: Arc<ServerConfig>,
    mut stop: oneshot::Receiver<()>,
) -> io::Result<Observations> {
    let address = listener.local_addr()?;
    require_localhost(address)?;
    let acceptor = TlsAcceptor::from(config);
    let mut tasks = JoinSet::new();
    let mut observations = Observations::default();
    let mut failure = None;
    let lifetime = tokio::time::sleep(Duration::from_secs(180));
    tokio::pin!(lifetime);
    loop {
        tokio::select! {
            _ = &mut stop => break,
            _ = &mut lifetime => break,
            joined = tasks.join_next(), if !tasks.is_empty() => {
                match joined {
                    Some(Ok(outcome)) => observe(outcome, &mut observations),
                    _ => { failure = Some(io_error("connection task failed")); break; },
                }
            },
            accepted = listener.accept() => {
                let (socket, _) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => { failure = Some(error); break; },
                };
                // Completed tasks also own a slot until joined, bounding their retained results.
                if tasks.len() >= CONNECTION_LIMIT {
                    observations.capacity_rejected += 1;
                    drop(socket);
                    continue;
                }
                observations.accepted += 1;
                let acceptor = acceptor.clone();
                tasks.spawn(async move {
                    let work = async {
                        let tls = match timeout(HANDSHAKE_DEADLINE, acceptor.accept(socket)).await {
                            Ok(Ok(tls)) => tls,
                            Ok(Err(_)) => return Outcome::Rejected(Rejection::Tls),
                            Err(_) => return Outcome::TimedOut,
                        };
                        let mut http = hyper::server::conn::http1::Builder::new();
                        http.keep_alive(false).max_headers(32).max_buf_size(8192)
                            .timer(TokioTimer::new()).header_read_timeout(HANDSHAKE_DEADLINE);
                        match http.serve_connection(TokioIo::new(tls), service_fn(move |request| route(request, address))).without_shutdown().await {
                            Ok(parts) => close_tls(parts.io.into_inner()).await,
                            Err(_) => Outcome::Rejected(Rejection::Http),
                        }
                    };
                    match timeout(CONNECTION_DEADLINE, work).await {
                        Ok(result) => result,
                        Err(_) => Outcome::TimedOut,
                    }
                });
            }
        }
    }
    drop(listener);
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok(outcome) => observe(outcome, &mut observations),
            Err(_) => {
                if failure.is_none() {
                    failure = Some(io_error("connection task failed"));
                }
            }
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(observations),
    }
}

/// One bounded TLS/HTTP request. No redirect, DNS, platform trust modification or permissive verifier.
pub async fn request(
    address: SocketAddr,
    config: Arc<ClientConfig>,
    name: ServerName<'static>,
    request: Request<Full<Bytes>>,
) -> io::Result<(StatusCode, Bytes)> {
    require_localhost(address)?;
    timeout(CONNECTION_DEADLINE, async {
        let socket = TcpStream::connect(address).await?;
        let tls = TlsConnector::from(config).connect(name, socket).await?;
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(tls))
            .await
            .map_err(|_| io_error("HTTP handshake failed"))?;
        let work = async move {
            let result = async {
                let response = sender
                    .send_request(request)
                    .await
                    .map_err(|_| diagnostics::ExchangeFailure::Request)?;
                let status = response.status();
                let body = Limited::new(response.into_body(), 1024)
                    .collect()
                    .await
                    .map_err(|_| diagnostics::ExchangeFailure::Body)?
                    .to_bytes();
                Ok((status, body))
            }
            .await;
            drop(sender);
            result
        };
        // Cancellation drops both owned futures; no detached connection task survives.
        let (result, connected) = tokio::join!(work, connection);
        diagnostics::finish(result, connected)
    })
    .await
    .map_err(|_| io_error("request deadline exceeded"))?
}
