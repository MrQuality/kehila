//! One observed serving-response stall; TLS remains opaque and verified end to end.
use super::super::Endpoint;
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};

mod tests;

struct Gate {
    armed: Arc<AtomicBool>,
    held: Option<oneshot::Sender<()>>,
    release: oneshot::Receiver<()>,
    cleanup: Arc<AtomicBool>,
}
pub(crate) struct Control {
    armed: Arc<AtomicBool>,
    held: oneshot::Receiver<()>,
}
impl Control {
    pub(crate) fn arm(self) -> oneshot::Receiver<()> {
        assert!(!self.armed.swap(true, Ordering::SeqCst));
        self.held
    }
}
pub(crate) struct Relay {
    running: JoinHandle<io::Result<()>>,
    release: Option<oneshot::Sender<()>>,
    cleanup: Arc<AtomicBool>,
}
impl Relay {
    pub(crate) async fn start(endpoint: &Endpoint) -> (Endpoint, Self, Control) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        // Only ephemeral fixture inputs are copied. No production Clone API or
        // host/name/root substitution is introduced by the transparent relay.
        let relayed = Endpoint {
            address: listener.local_addr().unwrap(),
            database: endpoint.database.clone(),
            password: zeroize::Zeroizing::new(endpoint.password.to_vec()),
            root: endpoint.root.clone(),
        };
        let address = endpoint.address;
        let armed = Arc::new(AtomicBool::new(false));
        let (held, observation) = oneshot::channel();
        let (release, released) = oneshot::channel();
        let cleanup = Arc::new(AtomicBool::new(false));
        let gate = Gate {
            armed: armed.clone(),
            held: Some(held),
            release: released,
            cleanup: cleanup.clone(),
        };
        let running = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            // Database::initialize awaits dedicated connect before serving
            // connect. Refuse any third connection rather than guessing roles.
            let (dedicated, _) = listener.accept().await?;
            let upstream = TcpStream::connect(address).await?;
            tasks.spawn(forward(dedicated, upstream, None));
            let (serving, _) = listener.accept().await?;
            let upstream = TcpStream::connect(address).await?;
            tasks.spawn(forward(serving, upstream, Some(gate)));
            drop(listener);
            while let Some(result) = tasks.join_next().await {
                result.map_err(io::Error::other)??;
            }
            Ok(())
        });
        (
            relayed,
            Self {
                running,
                release: Some(release),
                cleanup,
            },
            Control {
                armed,
                held: observation,
            },
        )
    }

    pub(crate) fn begin_cleanup(&mut self) {
        // This is a phase transition, not proof that held bytes were delivered.
        self.cleanup.store(true, Ordering::SeqCst);
        self.release();
    }

    fn release(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }

    pub(crate) async fn join(&mut self) -> io::Result<()> {
        self.release();
        match tokio::time::timeout(Duration::from_secs(5), &mut self.running).await {
            Ok(result) => result.map_err(io::Error::other)?,
            Err(_) => {
                self.running.abort();
                let _ = (&mut self.running).await;
                Err(io::Error::other("relay join deadline expired"))
            }
        }
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        self.release();
        // The running future owns its connection JoinSet. Failure drops/aborts
        // all relay sockets; successful evidence additionally requires join().
        self.running.abort();
    }
}

async fn forward(
    mut client: TcpStream,
    mut server: TcpStream,
    gate: Option<Gate>,
) -> io::Result<()> {
    let (mut client_read, mut client_write) = client.split();
    let (mut server_read, mut server_write) = server.split();
    tokio::select! {
        result = tokio::io::copy(&mut client_read, &mut server_write) => result.map(|_| ()),
        result = downstream(&mut server_read, &mut client_write, gate) => result,
    }
}

async fn downstream(
    mut server_read: impl AsyncRead + Unpin,
    mut client_write: impl AsyncWrite + Unpin,
    mut gate: Option<Gate>,
) -> io::Result<()> {
    let mut bytes = [0; 4096];
    let mut cleanup = false;
    loop {
        let count = server_read.read(&mut bytes).await?;
        if count == 0 {
            return Ok(());
        }
        if let Some(gate) = gate.as_mut() {
            if gate.armed.swap(false, Ordering::SeqCst) {
                gate.held
                    .take()
                    .ok_or_else(|| io::Error::other("gate already consumed"))?
                    .send(())
                    .map_err(|_| io::Error::other("stall observer dropped"))?;
                (&mut gate.release)
                    .await
                    .map_err(|_| io::Error::other("stall release dropped"))?;
                // Implicit release from join/Drop does not establish cleanup.
                // Capture eligibility here rather than reclassifying old errors.
                cleanup = gate.cleanup.load(Ordering::SeqCst);
            }
        }
        if let Err(error) = client_write.write_all(&bytes[..count]).await {
            // Only the gated serving client's downstream write may terminate
            // normally this way. Database reads and upstream I/O remain strict.
            if cleanup
                && matches!(
                    error.kind(),
                    io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
                )
            {
                return Ok(());
            }
            return Err(error);
        }
    }
}
