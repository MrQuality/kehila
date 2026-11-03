use super::*;
use tokio::{
    sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore},
    task::{Id, JoinSet},
};

const CAPACITY: usize = 16;

struct Message {
    operation: Operation,
    deadline: Instant,
    reply: oneshot::Sender<Response>,
    permit: Arc<OwnedSemaphorePermit>,
}

pub(crate) struct Response {
    result: Result<Output, Error>,
    deadline: Instant,
    live: Arc<RuntimeLiveness>,
    _permit: Arc<OwnedSemaphorePermit>,
}
impl Response {
    // Borrowing preserves the accounted reply/credential lifetime. Adapters
    // retain Response until their exchange/cleanup finishes.
    pub(crate) fn result(&self) -> Result<&Output, Error> {
        if let Err(error @ (Error::AuditUnconfirmed | Error::OtherAuditUnconfirmed)) = &self.result
        {
            return Err(*error);
        }
        if !self.live.is_live() || Instant::now() >= self.deadline {
            return Err(Error::Unavailable);
        }
        self.result.as_ref().map_err(|error| *error)
    }
    pub(crate) fn confirm_exchange(&mut self) -> Result<(), Error> {
        self.result()?;
        if let Ok(Output::Issued(delivery)) = &mut self.result {
            delivery.confirm_exchange()?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct Handle {
    sender: mpsc::Sender<Message>,
    capacity: Arc<Semaphore>,
    live: Arc<RuntimeLiveness>,
}
impl Handle {
    pub(crate) fn submit(
        &self,
        operation: Operation,
        deadline: Instant,
    ) -> Result<oneshot::Receiver<Response>, Error> {
        let now = Instant::now();
        if !self.live.is_live() || now >= deadline {
            return Err(Error::Unavailable);
        }
        let deadline = deadline.min(now.checked_add(WORK_DEADLINE).ok_or(Error::Unavailable)?);
        let permit = Arc::new(
            self.capacity
                .clone()
                .try_acquire_owned()
                .map_err(|_| Error::Busy)?,
        );
        let (reply, receiver) = oneshot::channel();
        self.sender
            .try_send(Message {
                operation,
                deadline,
                reply,
                permit,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => Error::Busy,
                mpsc::error::TrySendError::Closed(_) => Error::Unavailable,
            })?;
        Ok(receiver)
    }
    pub(crate) fn stop(&self) {
        self.live.stop();
        self.capacity.close();
    }
}

pub(crate) struct Owner {
    engine: Option<Arc<Engine>>,
    receiver: mpsc::Receiver<Message>,
    jobs: JoinSet<()>,
    slots: Vec<(Id, Arc<OwnedSemaphorePermit>)>,
}

impl Owner {
    fn engine(&self) -> &Arc<Engine> {
        self.engine
            .as_ref()
            .expect("owner engine already shut down")
    }

    pub(crate) async fn new(
        database: Database,
        blocklist: Blocklist,
    ) -> Result<(Handle, Self), Error> {
        let live = database.liveness();
        let binding = match database.authority() {
            Ok(authority) => snapshot(authority).binding,
            Err(_) => {
                database.shutdown().await;
                return Err(Error::Unavailable);
            }
        };
        let mut slots = Vec::new();
        if slots.try_reserve_exact(CAPACITY).is_err() {
            database.shutdown().await;
            return Err(Error::Unavailable);
        }
        let sessions = match Registry::new(binding, live.clone()) {
            Ok(registry) => registry,
            Err(_) => {
                database.shutdown().await;
                return Err(Error::Unavailable);
            }
        };
        let pre_sessions = match Pool::new(live.clone(), Instant::now()) {
            Ok(pool) => pool,
            Err(_) => {
                database.shutdown().await;
                return Err(Error::Unavailable);
            }
        };
        let engine = Arc::new(Engine {
            live: live.clone(),
            database: AsyncMutex::new(database),
            sessions: Mutex::new(sessions),
            pre_sessions,
            hash: Arc::new(AsyncMutex::new(Worker::new(live.clone()))),
            login_allowance: Mutex::new(Bucket::new(Instant::now(), 5)),
            management_allowance: Mutex::new(Bucket::new(Instant::now(), 5)),
            blocklist,
        });
        let (sender, receiver) = mpsc::channel(1);
        let handle = Handle {
            sender,
            capacity: Arc::new(Semaphore::new(CAPACITY)),
            live,
        };
        Ok((
            handle,
            Self {
                engine: Some(engine),
                receiver,
                jobs: JoinSet::new(),
                slots,
            },
        ))
    }

    fn joined(&mut self, result: Result<(Id, ()), tokio::task::JoinError>) {
        let id = match result {
            Ok((id, ())) => id,
            Err(error) => {
                self.engine().live.stop();
                error.id()
            }
        };
        if let Some(index) = self.slots.iter().position(|(stored, _)| *stored == id) {
            self.slots.swap_remove(index);
        } else {
            self.engine().live.stop();
        }
    }

    pub(crate) async fn run(mut self) -> Result<(), Error> {
        loop {
            let live = self.engine().live.clone();
            tokio::select! {
                biased;
                _ = live.stopped() => break,
                Some(result) = self.jobs.join_next_with_id(), if !self.jobs.is_empty() => self.joined(result),
                message = self.receiver.recv(), if self.jobs.len() < CAPACITY => {
                    let Some(message) = message else { break; };
                    let engine = self.engine().clone();
                    let permit = message.permit.clone();
                    let task = self.jobs.spawn(async move {
                        let mut failure = StopOnAbandonment { live: engine.live.clone(), completed: false };
                        let result = engine.execute(message.operation, message.deadline).await;
                        let result = if engine.check(message.deadline).is_ok()
                            || matches!(result, Err(Error::AuditUnconfirmed | Error::OtherAuditUnconfirmed)) { result }
                                     else { drop(result); Err(Error::Unavailable) };
                        let _ = message.reply.send(Response { result, deadline: message.deadline,
                            live: engine.live.clone(), _permit: message.permit });
                        failure.completed = true;
                    });
                    self.slots.push((task.id(), permit));
                }
            }
        }
        let stop_deadline = self.engine().live.shutdown_deadline();
        self.receiver.close();
        // Queued inputs/replies own permits and zeroizing values; draining them
        // never starts work after stop and does not touch an active job future.
        while self.receiver.try_recv().is_ok() {}
        while let Some(result) = self.jobs.join_next_with_id().await {
            self.joined(result);
        }
        let engine = Arc::try_unwrap(self.engine.take().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        let hash = Arc::try_unwrap(engine.hash).map_err(|_| Error::Unavailable)?;
        let mut worker = hash.into_inner();
        while !worker.finished() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        // No shared guard remains and the OS thread has actually finished.
        // Keeping Worker here also retains its joining Drop on exceptional exit.
        let hash = worker.shutdown();
        let database = engine.database.into_inner().shutdown().await;
        if hash.is_err() || database.aborted || database.failed || Instant::now() > stop_deadline {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
}

struct StopOnAbandonment {
    live: Arc<RuntimeLiveness>,
    completed: bool,
}
impl Drop for StopOnAbandonment {
    fn drop(&mut self) {
        if !self.completed {
            self.live.stop();
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(engine) = &self.engine {
            engine.live.stop();
        }
        // Exceptional fallback requests task abort; only run observes joins.
    }
}

#[cfg(test)]
mod tests;
