//! Fixed typed owner-fixture transactions. This module is test-only.
//! It reuses the marker-verified Admin client and its irreversible liveness.
use super::*;
#[derive(Clone, Copy)]
enum Mode {
    Snapshot,
    Write,
}
pub(in crate::postgres) async fn read_snapshot<'a>(
    client: &'a mut Client,
    expected: &Authority,
    live: Arc<RuntimeLiveness>,
    deadline: Instant,
) -> Result<Fenced<'a, ReadProof>, DbError> {
    open(client, expected, live, deadline, Mode::Snapshot).await
}
pub(in crate::postgres) async fn write_transaction<'a>(
    client: &'a mut Client,
    expected: &Authority,
    live: Arc<RuntimeLiveness>,
    deadline: Instant,
) -> Result<Fenced<'a, SessionProof>, DbError> {
    open(client, expected, live, deadline, Mode::Write).await
}
pub(in crate::postgres) async fn item_write_transaction<'a>(
    client: &'a mut Client,
    expected: &Authority,
    live: Arc<RuntimeLiveness>,
    deadline: Instant,
) -> Result<Fenced<'a, ItemWriteProof>, DbError> {
    open(client, expected, live, deadline, Mode::Write).await
}
pub(in crate::postgres) async fn item_replay_transaction<'a>(
    client: &'a mut Client,
    expected: &Authority,
    live: Arc<RuntimeLiveness>,
    deadline: Instant,
) -> Result<Fenced<'a, ItemReplayProof>, DbError> {
    open(client, expected, live, deadline, Mode::Write).await
}
async fn open<'a, P>(
    client: &'a mut Client,
    expected: &Authority,
    live: Arc<RuntimeLiveness>,
    deadline: Instant,
    mode: Mode,
) -> Result<Fenced<'a, P>, DbError> {
    if !live.is_live() {
        return Err(DbError::Unavailable);
    }
    if Instant::now() >= deadline {
        return Err(DbError::Expired);
    }
    let pending = PendingOperation::new(live);
    let read_only = matches!(mode, Mode::Snapshot);
    let isolation = if read_only {
        IsolationLevel::RepeatableRead
    } else {
        IsolationLevel::ReadCommitted
    };
    let transaction = timeout_at(
        TokioInstant::from_std(deadline),
        client
            .build_transaction()
            .isolation_level(isolation)
            .read_only(read_only)
            .start(),
    )
    .await
    .map_err(|_| DbError::Expired)?
    .map_err(query_error)?;
    let runtime = expected.runtime.to_string();
    let authentication = expected.authentication.to_string();
    let revision = Some(expected.revision.to_string());
    let row = match mode {
        Mode::Snapshot => {
            query(
                &transaction,
                &pending.live,
                deadline,
                READ_AUTHORITY,
                &[&expected.installation, &runtime, &authentication],
            )
            .await
        }
        Mode::Write => {
            query(
                &transaction,
                &pending.live,
                deadline,
                FENCE,
                &[&expected.installation, &runtime, &authentication, &revision],
            )
            .await
        }
    };
    let checked = row.and_then(|row| authority(&row)).and_then(|fresh| {
        if fresh.installation != expected.installation
            || fresh.owner != expected.owner
            || fresh.runtime != expected.runtime
            || fresh.authentication != expected.authentication
            || fresh.revision != expected.revision
        {
            Err(DbError::Unavailable)
        } else {
            Ok(fresh)
        }
    });
    match checked {
        Ok(authority) => Ok(Fenced {
            proof: std::marker::PhantomData,
            transaction: Some(transaction),
            authority,
            pending,
            deadline,
            failed: false,
            lose_confirmation: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }),
        Err(error) => {
            rollback(transaction, pending).await?;
            Err(error)
        }
    }
}

// Only a fresh item-write owner can request this existing confirmation seam.
// COMMIT still executes; dropping the unfinished PendingOperation poisons owner.
impl Fenced<'_, ItemWriteProof> {
    pub(crate) fn lose_item_commit_confirmation_for_test(&mut self) {
        self.lose_confirmation
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
