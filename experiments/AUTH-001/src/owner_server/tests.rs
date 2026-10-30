use super::*;
use rustls::pki_types::{PrivatePkcs8KeyDer, ServerName};
use tokio_rustls::TlsConnector;

#[tokio::test(flavor = "current_thread")]
async fn tls_closure_keeps_tcp_open_until_peer_close_notify() {
    timeout(Duration::from_secs(3), async {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
        let acceptor = TlsAcceptor::from(
            crate::server_config(
                cert.der().clone(),
                PrivatePkcs8KeyDer::from(signing_key.serialize_der()).into(),
            )
            .unwrap(),
        );
        let connector = TlsConnector::from(crate::client_config(Some(cert.der().clone())).unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = async {
            let (socket, _) = listener.accept().await.unwrap();
            let tls = acceptor.accept(socket).await.unwrap();
            let mut guarded = Guarded {
                tls,
                exchange: Arc::new(Mutex::new(None)),
            };
            assert!(guarded.close().await.is_ok());
        };
        let client = async {
            let socket = TcpStream::connect(address).await.unwrap();
            let mut tls = connector
                .connect(ServerName::IpAddress(address.ip().into()), socket)
                .await
                .unwrap();
            let mut byte = [0];
            assert_eq!(tls.read(&mut byte).await.unwrap(), 0);
            // TLS EOF must not be accompanied by TCP FIN while the server
            // still requires this peer's authenticated TLS close notification.
            assert!(
                timeout(Duration::from_millis(100), tls.get_ref().0.peek(&mut byte))
                    .await
                    .is_err(),
                "TCP closed before the peer could send its TLS close notification"
            );
            tls.shutdown().await.unwrap();
        };
        tokio::join!(server, client);
    })
    .await
    .unwrap();
}
