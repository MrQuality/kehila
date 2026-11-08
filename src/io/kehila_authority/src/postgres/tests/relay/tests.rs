//! Deterministic downstream I/O controls; the native fixture covers TCP/TLS composition.
use super::*;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::ReadBuf;

struct WriteFailure(io::ErrorKind);
impl AsyncWrite for WriteFailure {
    fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, _: &[u8]) -> Poll<io::Result<usize>> {
        Poll::Ready(Err(io::Error::from(self.0)))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

struct ReadFailure;
impl AsyncRead for ReadFailure {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Err(io::Error::from(io::ErrorKind::ConnectionReset)))
    }
}

async fn released(
    reader: impl AsyncRead + Unpin,
    writer: impl AsyncWrite + Unpin,
    cleanup: bool,
) -> io::Result<()> {
    let (held, observed) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let phase = Arc::new(AtomicBool::new(false));
    let gate = Gate {
        armed: Arc::new(AtomicBool::new(true)),
        held: Some(held),
        release: released,
        cleanup: phase.clone(),
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(1), async {
        tokio::join!(downstream(reader, writer, Some(gate)), async {
            // The selected bytes must actually reach the gate before cleanup.
            observed.await.unwrap();
            phase.store(cleanup, Ordering::SeqCst);
            release.send(()).unwrap();
        })
    })
    .await
    .unwrap();
    result
}

#[tokio::test]
async fn released_client_disconnect_requires_explicit_cleanup() {
    for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::ConnectionReset] {
        assert_eq!(
            released(&b"held"[..], WriteFailure(kind), false)
                .await
                .unwrap_err()
                .kind(),
            kind
        );
        released(&b"held"[..], WriteFailure(kind), true)
            .await
            .unwrap();
        assert_eq!(
            downstream(&b"early"[..], WriteFailure(kind), None)
                .await
                .unwrap_err()
                .kind(),
            kind
        );
    }
}

#[tokio::test]
async fn closed_peer_after_cleanup_release_is_terminal() {
    let (writer, peer) = tokio::io::duplex(64);
    drop(peer);
    released(&b"held"[..], writer, true).await.unwrap();
}

#[tokio::test]
async fn cleanup_preserves_other_write_and_database_read_errors() {
    assert_eq!(
        released(
            &b"held"[..],
            WriteFailure(io::ErrorKind::PermissionDenied),
            true
        )
        .await
        .unwrap_err()
        .kind(),
        io::ErrorKind::PermissionDenied
    );
    // One held frame is successfully written before the database-side reset.
    assert_eq!(
        released((&b"held"[..]).chain(ReadFailure), tokio::io::sink(), true)
            .await
            .unwrap_err()
            .kind(),
        io::ErrorKind::ConnectionReset
    );
}
