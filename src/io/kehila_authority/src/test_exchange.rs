//! Test-only ordinary TLS client and owned exchange regressions.
mod diagnostics;
use crate::{io_error, CONNECTION_DEADLINE};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, ServerName},
    ClientConfig, ServerConfig,
};
use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{net::TcpStream, time::timeout};
use tokio_rustls::TlsConnector;
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
pub(crate) async fn request_with_headers(
    address: SocketAddr,
    config: Arc<ClientConfig>,
    name: ServerName<'static>,
    request: Request<Full<Bytes>>,
) -> io::Result<(StatusCode, hyper::HeaderMap, Bytes)> {
    let headers = Arc::new(std::sync::Mutex::new(None));
    let (status, body) = request_inner(
        address,
        config,
        name,
        request,
        crate::project::wire::MAX_WIRE_BYTES,
        Some(headers.clone()),
    )
    .await?;
    let captured = headers
        .lock()
        .map_err(|_| io_error("test headers unavailable"))?
        .take()
        .ok_or_else(|| io_error("test headers missing"))?;
    Ok((status, captured, body))
}

async fn request_inner(
    address: SocketAddr,
    config: Arc<ClientConfig>,
    name: ServerName<'static>,
    request: Request<Full<Bytes>>,
    limit: usize,
    headers: Option<Arc<std::sync::Mutex<Option<hyper::HeaderMap>>>>,
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
                if let Some(headers) = headers {
                    *headers
                        .lock()
                        .map_err(|_| diagnostics::ExchangeFailure::Request)? =
                        Some(response.headers().clone());
                }
                let body = Limited::new(response.into_body(), limit)
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
