mod common;
use std::{
    error::Error,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::Request;
use kehila_local_tls_probe::{client_config, request};
use rustls::pki_types::{CertificateDer, ServerName};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(args.next().ok_or("expected root DER file and port")?);
    let port: u16 = args
        .next()
        .ok_or("expected port")?
        .to_str()
        .ok_or("invalid port")?
        .parse()?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let config = client_config(Some(CertificateDer::from(common::bounded_file(
        &root,
        128 * 1024,
    )?)))?;
    let body = Request::post("/probe")
        .header("Host", address.to_string())
        .header("Origin", format!("https://{address}"))
        .header("Content-Type", "application/json")
        .header("Content-Length", "2")
        .body(Full::new(Bytes::from_static(b"{}")))?;
    let (status, bytes) = request(address, config, ServerName::from(address.ip()), body).await?;
    if status != hyper::StatusCode::OK || bytes.as_ref() != b"{\"transport\":\"ready\"}" {
        return Err("unexpected probe response".into());
    }
    println!("Verified exact-root/IP TLS and closed probe response: 200");
    Ok(())
}
