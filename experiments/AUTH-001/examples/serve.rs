mod common;
use std::{error::Error, path::PathBuf};

use kehila_local_tls_probe::{serve, server_config};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use tokio::{net::TcpListener, sync::oneshot};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory = PathBuf::from(
        args.next()
            .ok_or("expected protected fixture directory and port")?,
    );
    let port: u16 = args
        .next()
        .ok_or("expected port")?
        .to_str()
        .ok_or("invalid port")?
        .parse()?;
    let stop_file = args.next().map(PathBuf::from);
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let certificate = CertificateDer::from(common::bounded_file(
        &directory.join("server.cer"),
        128 * 1024,
    )?);
    let key = PrivatePkcs8KeyDer::from(common::bounded_file(
        &directory.join("server-key.der"),
        16 * 1024,
    )?);
    let config = server_config(certificate, key.into())?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!(
        "HTTPS probe ready on https://{}/ ; automatic lifetime 180 seconds",
        listener.local_addr()?
    );
    let (stop, receiver) = oneshot::channel();
    let mut stop = Some(stop);
    let work = serve(listener, config, receiver);
    tokio::pin!(work);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
    let observed = loop {
        tokio::select! {
            result = &mut work => break result?,
            _ = tick.tick() => {
                if stop_file.as_ref().is_some_and(|path| path.exists()) { drop(stop.take()); }
            }
        }
    };
    println!("Probe stopped: {observed:?}");
    Ok(())
}
