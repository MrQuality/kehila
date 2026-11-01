use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, StatusCode};
use kehila_local_tls_probe::{client_config, request, serve, server_config, Observations};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, ExtendedKeyUsagePurpose, IsCa, Issuer,
    KeyPair, KeyUsagePurpose,
};
use rustls::{
    pki_types::{PrivatePkcs8KeyDer, ServerName},
    ClientConfig,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

#[path = "transport/resources.rs"]
mod resources;
#[path = "transport/shutdown.rs"]
mod shutdown;
mod support;
use support::Network;

struct Authority {
    root: Certificate,
    issuer: Issuer<'static, KeyPair>,
}
impl Authority {
    fn new() -> Self {
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        let key = KeyPair::generate().unwrap();
        let root = params.self_signed(&key).unwrap();
        Self {
            root,
            issuer: Issuer::new(params, key),
        }
    }
    fn leaf(&self, name: &str, expired: bool) -> (Certificate, KeyPair) {
        let mut params = CertificateParams::new(vec![name.to_owned()]).unwrap();
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        if expired {
            params.not_before = rcgen::date_time_ymd(2019, 1, 1);
            params.not_after = rcgen::date_time_ymd(2020, 1, 1);
        }
        let key = KeyPair::generate().unwrap();
        (params.signed_by(&key, &self.issuer).unwrap(), key)
    }
    fn client(&self) -> Arc<ClientConfig> {
        client_config(Some(self.root.der().clone())).unwrap()
    }
}

#[test]
fn certificate_fixture_completes_real_rustls_handshake_without_network() {
    let ca = Authority::new();
    let (leaf, key) = ca.leaf("127.0.0.1", false);
    let server = server_config(
        leaf.der().clone(),
        PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
    )
    .unwrap();
    let mut server = rustls::ServerConnection::new(server).unwrap();
    let mut client = rustls::ClientConnection::new(
        ca.client(),
        ServerName::from("127.0.0.1".parse::<std::net::IpAddr>().unwrap()),
    )
    .unwrap();
    for _ in 0..8 {
        let mut bytes = Vec::new();
        client.write_tls(&mut bytes).unwrap();
        server.read_tls(&mut std::io::Cursor::new(bytes)).unwrap();
        server.process_new_packets().unwrap();
        let mut bytes = Vec::new();
        server.write_tls(&mut bytes).unwrap();
        client.read_tls(&mut std::io::Cursor::new(bytes)).unwrap();
        client.process_new_packets().unwrap();
        if !client.is_handshaking() && !server.is_handshaking() {
            return;
        }
    }
    panic!("fixture handshake did not settle");
}

#[tokio::test]
async fn tcp_baseline_reaches_same_loopback_endpoint() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let Network {
        listener,
        serial: _serial,
    } = Network::bind().await;
    let address = listener.local_addr().unwrap();
    let work = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut data = [0; 4];
        socket.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"ping");
        socket.write_all(b"pong").await.unwrap();
    };
    let client = async {
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket.write_all(b"ping").await.unwrap();
        let mut data = [0; 4];
        socket.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"pong");
    };
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        tokio::join!(work, client);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn tls_baseline_without_http_or_server_loop() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let ca = Authority::new();
    let (leaf, key) = ca.leaf("127.0.0.1", false);
    let config = server_config(
        leaf.der().clone(),
        PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
    )
    .unwrap();
    let Network {
        listener,
        serial: _serial,
    } = Network::bind().await;
    let address = listener.local_addr().unwrap();
    let work = async {
        let (socket, _) = listener.accept().await.unwrap();
        let mut tls = tokio_rustls::TlsAcceptor::from(config)
            .accept(socket)
            .await
            .expect("server TLS baseline");
        let mut data = [0; 4];
        tls.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"ping");
        tls.write_all(b"pong").await.unwrap();
        tls.shutdown().await.unwrap();
    };
    let client = async {
        let socket = tokio::net::TcpStream::connect(address).await.unwrap();
        let mut tls = tokio_rustls::TlsConnector::from(ca.client())
            .connect(ServerName::from(address.ip()), socket)
            .await
            .expect("client TLS baseline");
        tls.write_all(b"ping").await.unwrap();
        let mut data = [0; 4];
        tls.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"pong");
    };
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        tokio::join!(work, client);
    })
    .await
    .unwrap();
}

struct Running {
    address: SocketAddr,
    stop: oneshot::Sender<()>,
    task: JoinHandle<std::io::Result<Observations>>,
    _serial: tokio::sync::MutexGuard<'static, ()>,
}
impl Running {
    async fn start(authority: &Authority, name: &str, expired: bool) -> Self {
        let (cert, key) = authority.leaf(name, expired);
        let config = server_config(
            cert.der().clone(),
            PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        )
        .unwrap();
        let Network { listener, serial } = Network::bind().await;
        let address = listener.local_addr().unwrap();
        let (stop, receiver) = oneshot::channel();
        Self {
            address,
            stop,
            task: tokio::spawn(serve(listener, config, receiver)),
            _serial: serial,
        }
    }
    async fn finish(self) -> Observations {
        self.finish_checked().await.unwrap()
    }
    async fn finish_checked(self) -> Result<Observations, &'static str> {
        let stopped = self.stop.send(()).is_ok();
        let joined = self.task.await;
        // Always attempt listener release after joining, even on a server failure.
        let released = TcpListener::bind(self.address).await.map(drop);
        let observed = joined
            .map_err(|_| "server task join failed")?
            .map_err(|_| "server returned failure")?;
        released.map_err(|_| "probe port not released")?;
        if !stopped {
            return Err("server stopped before stop request");
        }
        Ok(observed)
    }
    async fn post(
        &self,
        client: Arc<ClientConfig>,
        origin: Option<&str>,
        host: &str,
    ) -> std::io::Result<(StatusCode, Bytes)> {
        let mut builder = Request::post("/probe")
            .header("Host", host)
            .header("Content-Type", "application/json");
        if let Some(origin) = origin {
            builder = builder.header("Origin", origin);
        }
        request(
            self.address,
            client,
            ServerName::from(self.address.ip()),
            builder.body(Full::new(Bytes::from_static(b"{}"))).unwrap(),
        )
        .await
    }
}

#[tokio::test]
async fn trusted_ip_san_serves_closed_response() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let host = running.address.to_string();
    let result = running
        .post(ca.client(), Some(&format!("https://{host}")), &host)
        .await;
    let observed = running.finish_checked().await;
    let result = result.unwrap_or_else(|error| panic!("client={error}; server={observed:?}"));
    assert!(
        observed.is_ok(),
        "client_status={}; server={observed:?}",
        result.0
    );
    assert_eq!(
        result,
        (
            StatusCode::OK,
            Bytes::from_static(b"{\"transport\":\"ready\"}")
        )
    );
    assert_eq!(observed.unwrap().accepted, 1);
}

#[tokio::test]
async fn missing_foreign_origin_and_wrong_host_are_rejected() {
    let ca = Authority::new();
    let running = Running::start(&ca, "127.0.0.1", false).await;
    let host = running.address.to_string();
    for origin in [None, Some("null"), Some("https://foreign.invalid")] {
        assert_eq!(
            running.post(ca.client(), origin, &host).await.unwrap().0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        running
            .post(
                ca.client(),
                Some(&format!("https://{host}")),
                "foreign.invalid"
            )
            .await
            .unwrap()
            .0,
        StatusCode::BAD_REQUEST
    );
    running.finish().await;
}

#[tokio::test]
async fn untrusted_root_wrong_san_and_expired_leaf_fail_tls() {
    let ca = Authority::new();
    for (name, expired, trusted) in [
        ("127.0.0.1", false, false),
        ("wrong.invalid", false, true),
        ("127.0.0.1", true, true),
    ] {
        let running = Running::start(&ca, name, expired).await;
        let host = running.address.to_string();
        let client = if trusted {
            ca.client()
        } else {
            Authority::new().client()
        };
        let error = running
            .post(client, Some(&format!("https://{host}")), &host)
            .await
            .unwrap_err();
        let tls = error
            .get_ref()
            .and_then(|e| e.downcast_ref::<rustls::Error>())
            .expect("certificate error, not unrelated transport failure");
        assert!(matches!(tls, rustls::Error::InvalidCertificate(_)), "{tls}");
        use rustls::CertificateError;
        let correct_reason = matches!(
            (name, expired, tls),
            (
                _,
                true,
                rustls::Error::InvalidCertificate(
                    CertificateError::Expired | CertificateError::ExpiredContext { .. },
                ),
            ) | (
                "wrong.invalid",
                false,
                rustls::Error::InvalidCertificate(
                    CertificateError::NotValidForName
                        | CertificateError::NotValidForNameContext { .. },
                ),
            ) | (
                "127.0.0.1",
                false,
                rustls::Error::InvalidCertificate(
                    CertificateError::UnknownIssuer | CertificateError::BadSignature,
                ),
            )
        );
        assert!(correct_reason, "unexpected certificate rejection: {tls}");
        assert_eq!(running.finish().await.protocol_rejected, 1);
    }
}

#[test]
fn mismatched_private_key_fails_config_before_binding() {
    let ca = Authority::new();
    let (leaf, _) = ca.leaf("127.0.0.1", false);
    let wrong = KeyPair::generate().unwrap();
    assert!(matches!(
        server_config(
            leaf.der().clone(),
            PrivatePkcs8KeyDer::from(wrong.serialize_der()).into()
        ),
        Err(rustls::Error::InconsistentKeys(_))
    ));
}

#[tokio::test]
async fn renewed_leaf_works_under_same_root_then_root_removal_denies() {
    let ca = Authority::new();
    let client = ca.client();
    for _ in 0..2 {
        let running = Running::start(&ca, "127.0.0.1", false).await;
        let host = running.address.to_string();
        assert_eq!(
            running
                .post(client.clone(), Some(&format!("https://{host}")), &host)
                .await
                .unwrap()
                .0,
            StatusCode::OK
        );
        let error = running
            .post(
                client_config(None).unwrap(),
                Some(&format!("https://{host}")),
                &host,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error
                .get_ref()
                .and_then(|error| error.downcast_ref::<rustls::Error>()),
            Some(rustls::Error::InvalidCertificate(
                rustls::CertificateError::UnknownIssuer
            ))
        ));
        running.finish().await;
    }
}
