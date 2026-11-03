//! Authenticated native product authority for the bounded fresh-database profile.
//! Domain codecs remain pure; only this Linux authority owns serving state/I/O.
#![cfg(target_os = "linux")]
mod bundle;
mod coordinator;
mod http_auth;
mod lifecycle;
mod listener;
mod native_ingress;
mod password;
mod postgres;
mod pre_session;
mod project;
mod protected_file;
mod secret;
mod session;
mod startup;
mod transport;
pub use transport::ConnectionMetrics;
pub(crate) use transport::{observe, Outcome, Rejection};
#[cfg(test)]
mod test_exchange;
use std::{io, time::Duration};
#[cfg(test)]
pub(crate) use test_exchange::{client_config, server_config};
use tokio::sync::oneshot;
pub(crate) const CONNECTION_LIMIT: usize = 16;
pub(crate) const CONNECTION_DEADLINE: Duration = Duration::from_secs(5);
pub(crate) const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(2);
fn io_error(message: &'static str) -> io::Error {
    io::Error::other(message)
}
/// Fresh startup audit identity. Construct before entering an async runtime.
/// OS entropy may block; no database, listener or runtime resources exist yet.
pub struct StartupAttempt(String);

pub fn prepare_startup_attempt() -> io::Result<StartupAttempt> {
    startup_attempt_with(secret::Secret::generate)
}

fn startup_attempt_with(
    make: impl FnOnce() -> Result<secret::Secret, secret::SecretError>,
) -> io::Result<StartupAttempt> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(io_error("startup entropy requires synchronous composition"));
    }
    let value = make().map_err(|_| io_error("startup entropy unavailable"))?;
    let value =
        std::str::from_utf8(value.encoded()).map_err(|_| io_error("startup identity invalid"))?;
    Ok(StartupAttempt(value.to_owned()))
}
/// Serve only the product authority; there is no diagnostic probe entry point.
pub async fn serve(
    arguments: Vec<String>,
    attempt: StartupAttempt,
    stop: oneshot::Receiver<()>,
) -> io::Result<ConnectionMetrics> {
    listener::run(arguments, attempt, stop).await
}

#[cfg(test)]
mod composition_tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn startup_refuses_async_entropy_before_calling_the_source() {
        let called = std::cell::Cell::new(false);
        assert!(startup_attempt_with(|| {
            called.set(true);
            secret::Secret::generate()
        })
        .is_err());
        assert!(!called.get());
    }

    #[test]
    fn synchronous_startup_prepares_bounded_audit_identity() {
        let value = prepare_startup_attempt().unwrap();
        assert_eq!(value.0.len(), 43);
        assert!(value
            .0
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'));
    }
}
