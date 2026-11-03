//! Bounded connection outcomes; no request contents or credentials.
#[derive(Default, Debug)]
pub struct ConnectionMetrics {
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

pub(crate) enum Rejection {
    Tls,
    Http,
    TlsShutdown,
    PeerClose,
}

pub(crate) enum Outcome {
    Complete,
    Rejected(Rejection),
    TimedOut,
}

pub(crate) fn observe(result: Outcome, observations: &mut ConnectionMetrics) {
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
