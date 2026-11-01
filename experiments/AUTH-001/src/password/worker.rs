//! One owned CPU worker. Normal shutdown joins after stopping admission and
//! outside request execution, shared locks and database fences.
use super::{
    blocklist::AllowedPassword,
    hash::{HashError, Hasher, Verifier},
    NormalizedPassword,
};
use crate::lifecycle::RuntimeLiveness;
#[cfg(target_os = "linux")]
use crate::{secret::Secret, session::Kind};
use ring::rand::{SecureRandom, SystemRandom};
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Instant,
};
use tokio::sync::oneshot;

#[cfg(target_os = "linux")]
pub(crate) struct LoginSecrets {
    pub(crate) bearer: Secret,
    pub(crate) csrf: Option<Secret>,
    pub(crate) public_id: String,
}

#[cfg(target_os = "linux")]
fn secret(control: &Control) -> Result<Secret, HashError> {
    if !control.allowed() {
        return Err(HashError::Cancelled);
    }
    Secret::generate().map_err(|_| HashError::Unavailable)
}

#[cfg(target_os = "linux")]
fn public_id(control: &Control) -> Result<String, HashError> {
    let id = secret(control)?;
    Ok(std::str::from_utf8(id.encoded())
        .map_err(|_| HashError::Unavailable)?
        .to_owned())
}

pub(crate) enum Change {
    Mismatch,
    Replacement(Verifier),
}
pub(crate) type HashPending<T> = Pending<Result<T, HashError>>;

fn random_salt() -> Result<[u8; 16], HashError> {
    let mut salt = [0; 16];
    SystemRandom::new()
        .fill(&mut salt)
        .map_err(|_| HashError::Unavailable)?;
    Ok(salt)
}

fn salted_stage<T>(
    control: &Control,
    random: impl FnOnce() -> Result<[u8; 16], HashError>,
    hash: impl FnOnce([u8; 16]) -> Result<T, HashError>,
) -> Result<T, HashError> {
    if !control.allowed() {
        return Err(HashError::Cancelled);
    }
    let salt = random()?;
    if !control.allowed() {
        return Err(HashError::Cancelled);
    }
    hash(salt)
}

// Keep stage admission shared by production and deterministic cancellation tests.
fn replacement_stages<C, T>(
    control: &Control,
    context: &mut C,
    verify: impl FnOnce(&mut C) -> Result<bool, HashError>,
    random: impl FnOnce() -> Result<[u8; 16], HashError>,
    replace: impl FnOnce(&mut C, [u8; 16]) -> Result<T, HashError>,
) -> Result<Option<T>, HashError> {
    if !control.allowed() {
        return Err(HashError::Cancelled);
    }
    if !verify(context)? {
        return Ok(None);
    }
    salted_stage(control, random, |salt| replace(context, salt)).map(Some)
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum WorkerError {
    Busy,
    Unavailable,
    Expired,
    Panicked,
    Stopped,
}

struct Control {
    live: Arc<RuntimeLiveness>,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}
struct Active {
    cancelled: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}
pub(crate) struct Worker {
    live: Arc<RuntimeLiveness>,
    active: Option<Active>,
    stopped: bool,
}
pub(crate) struct Pending<T> {
    receiver: Option<oneshot::Receiver<Result<T, WorkerError>>>,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
    received: bool,
}

impl Control {
    fn allowed(&self) -> bool {
        self.live.is_live()
            && !self.cancelled.load(Ordering::Acquire)
            && Instant::now() < self.deadline
    }
}
impl Worker {
    #[cfg(target_os = "linux")]
    pub(crate) fn public_id(
        &mut self,
        deadline: Instant,
    ) -> Result<HashPending<String>, WorkerError> {
        self.start(deadline, public_id)
    }
    #[cfg(target_os = "linux")]
    pub(crate) fn finished(&self) -> bool {
        self.active
            .as_ref()
            .is_none_or(|active| active.thread.is_finished())
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn join_completed(&mut self) -> Result<(), WorkerError> {
        if !self.finished() {
            return Err(WorkerError::Busy);
        }
        self.join_active()
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn anonymous(
        &mut self,
        deadline: Instant,
    ) -> Result<HashPending<(Secret, Secret)>, WorkerError> {
        self.start(deadline, |control| Ok((secret(control)?, secret(control)?)))
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn login(
        &mut self,
        deadline: Instant,
        password: NormalizedPassword,
        verifier: Verifier,
        kind: Kind,
    ) -> Result<HashPending<Option<LoginSecrets>>, WorkerError> {
        self.start(deadline, move |control| {
            let mut hasher = Hasher::new()?;
            if !control.allowed() {
                return Err(HashError::Cancelled);
            }
            if !hasher.verify(&password, &verifier)? {
                return Ok(None);
            }
            let bearer = secret(control)?;
            let csrf = match kind {
                Kind::Browser => Some(secret(control)?),
                Kind::Cli => None,
            };
            let public_id = public_id(control)?;
            Ok(Some(LoginSecrets {
                bearer,
                csrf,
                public_id,
            }))
        })
    }
    pub(crate) fn new(live: Arc<RuntimeLiveness>) -> Self {
        Self {
            live,
            active: None,
            stopped: false,
        }
    }

    pub(crate) fn new_verifier(
        &mut self,
        deadline: Instant,
        password: AllowedPassword,
    ) -> Result<HashPending<Verifier>, WorkerError> {
        let password = password.into_password();
        self.start(deadline, move |control| {
            let mut hasher = Hasher::new()?;
            salted_stage(control, random_salt, |salt| hasher.hash(&password, salt))
        })
    }

    pub(crate) fn verify(
        &mut self,
        deadline: Instant,
        password: NormalizedPassword,
        verifier: Verifier,
    ) -> Result<HashPending<bool>, WorkerError> {
        self.start(deadline, move |control| {
            let mut hasher = Hasher::new()?;
            if !control.allowed() {
                return Err(HashError::Cancelled);
            }
            hasher.verify(&password, &verifier)
        })
    }

    pub(crate) fn replacement(
        &mut self,
        deadline: Instant,
        current: NormalizedPassword,
        replacement: AllowedPassword,
        verifier: Verifier,
    ) -> Result<HashPending<Change>, WorkerError> {
        let replacement = replacement.into_password();
        // Only the blocklist can construct an admitted new password.
        // No database or session state enters the CPU worker.
        self.start(deadline, move |control| {
            let mut hasher = Hasher::new()?;
            replacement_stages(
                control,
                &mut hasher,
                |hasher| hasher.verify(&current, &verifier),
                random_salt,
                |hasher, salt| hasher.hash(&replacement, salt),
            )
            .map(|result| result.map_or(Change::Mismatch, Change::Replacement))
        })
    }
    fn start<T: Send + 'static>(
        &mut self,
        deadline: Instant,
        work: impl FnOnce(&Control) -> T + Send + 'static,
    ) -> Result<Pending<T>, WorkerError> {
        self.start_with(deadline, work, |job| {
            thread::Builder::new()
                .name("kehila-password".to_owned())
                .stack_size(2 * 1024 * 1024)
                .spawn(job)
        })
    }

    fn start_with<T: Send + 'static>(
        &mut self,
        deadline: Instant,
        work: impl FnOnce(&Control) -> T + Send + 'static,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> io::Result<JoinHandle<()>>,
    ) -> Result<Pending<T>, WorkerError> {
        if self.stopped || !self.live.is_live() {
            return Err(WorkerError::Stopped);
        }
        if Instant::now() >= deadline {
            return Err(WorkerError::Expired);
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.thread.is_finished())
        {
            self.join_active()?;
        }
        if self.active.is_some() {
            return Err(WorkerError::Busy);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let control = Control {
            live: self.live.clone(),
            cancelled: cancelled.clone(),
            deadline,
        };
        let (sender, receiver) = oneshot::channel();
        let job = Box::new(move || {
            let result = if control.allowed() {
                let value = work(&control);
                if control.allowed() {
                    Ok(value)
                } else {
                    Err(WorkerError::Expired)
                }
            } else {
                Err(WorkerError::Expired)
            };
            // A cancelled receiver discards and drops the owned result; it does
            // not stop this thread or release its owner's capacity early.
            let _ = sender.send(result);
        });
        let thread = spawn(job).map_err(|_| WorkerError::Unavailable)?;
        self.active = Some(Active {
            cancelled: cancelled.clone(),
            thread,
        });
        Ok(Pending {
            receiver: Some(receiver),
            cancelled,
            deadline,
            received: false,
        })
    }
    fn join_active(&mut self) -> Result<(), WorkerError> {
        if let Some(active) = self.active.take() {
            active.thread.join().map_err(|_| WorkerError::Panicked)?;
        }
        Ok(())
    }
    pub(crate) fn shutdown(&mut self) -> Result<(), WorkerError> {
        self.stopped = true;
        if let Some(active) = &self.active {
            active.cancelled.store(true, Ordering::Release);
        }
        self.join_active()
    }
}
impl<T> Pending<T> {
    // Completion-oriented: the request coordinator must wrap this future in
    // its absolute timeout. Dropping it cancels admission of later stages but
    // cannot interrupt an active hash or release the owner's thread slot.
    pub(crate) async fn result(mut self) -> Result<T, WorkerError> {
        let result = self
            .receiver
            .take()
            .ok_or(WorkerError::Unavailable)?
            .await
            .map_err(|_| WorkerError::Panicked)??;
        if self.cancelled.load(Ordering::Acquire) || Instant::now() >= self.deadline {
            return Err(WorkerError::Expired);
        }
        self.received = true;
        Ok(result)
    }
}

impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        if !self.received {
            self.cancelled.store(true, Ordering::Release);
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Emergency owned cleanup, not the request cancellation path. Normal
        // lifecycle uses shutdown outside locks/fences and handles its result.
        // Drop cannot report a panic, but still joins rather than detaching.
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            mpsc, Arc,
        },
        time::Duration,
    };

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(3)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shared_runtime_stop_denies_later_worker_stages_and_new_jobs() {
        let live = RuntimeLiveness::new();
        let mut owner = Worker::new(live.clone());
        let (started, observed) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let pending = owner
            .start(deadline(), move |control| {
                started.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(2)).unwrap();
                control.allowed()
            })
            .unwrap();
        observed.recv_timeout(Duration::from_secs(2)).unwrap();
        live.stop();
        release.send(()).unwrap();
        assert!(matches!(pending.result().await, Err(WorkerError::Expired)));
        owner.join_active().unwrap();
        assert!(matches!(
            owner.start(deadline(), |_| ()),
            Err(WorkerError::Stopped)
        ));
    }

    fn allowed(value: &str) -> AllowedPassword {
        super::super::blocklist::Blocklist::synthetic_for_tests()
            .admit(NormalizedPassword::new(zeroize::Zeroizing::new(value.to_owned())).unwrap())
            .unwrap()
    }

    fn reference_verifier() -> Verifier {
        Verifier::from_stored(
            zeroize::Zeroizing::new(
                concat!(
                    "$argon2id$v=19$m=65536,t=3,p=1$AQEBAQEBAQEBAQEBAQEBAQ$",
                    "6d50UfWIhEf6nFrBV/eh7X/wOi7ToEKiEY6e0UdWGa8"
                )
                .to_owned(),
            ),
            1,
        )
        .unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn admitted_new_password_hashes_and_existing_password_verifies_without_blocklist() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let verifier = owner
            .new_verifier(
                Instant::now() + Duration::from_secs(30),
                allowed("a synthetic new password"),
            )
            .unwrap()
            .result()
            .await
            .unwrap()
            .unwrap();
        owner.join_active().unwrap();
        let password = NormalizedPassword::new(zeroize::Zeroizing::new(
            "a synthetic new password".to_owned(),
        ))
        .unwrap();
        assert!(owner
            .verify(Instant::now() + Duration::from_secs(30), password, verifier)
            .unwrap()
            .result()
            .await
            .unwrap()
            .unwrap());
        owner.shutdown().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn blocklisted_current_password_still_verifies_but_cannot_be_new_password() {
        let password = || {
            NormalizedPassword::new(zeroize::Zeroizing::new(
                "blocked synthetic password".to_owned(),
            ))
            .unwrap()
        };
        let list = super::super::blocklist::Blocklist::synthetic_for_tests();
        assert!(matches!(
            list.admit(password()),
            Err(super::super::blocklist::BlocklistError::Blocked)
        ));
        // Represents a verifier created before this choice entered the list.
        let verifier = Hasher::new().unwrap().hash(&password(), [3; 16]).unwrap();
        let mut owner = Worker::new(RuntimeLiveness::new());
        assert!(owner
            .verify(
                Instant::now() + Duration::from_secs(30),
                password(),
                verifier,
            )
            .unwrap()
            .result()
            .await
            .unwrap()
            .unwrap());
        owner.shutdown().unwrap();
    }

    #[test]
    fn replacement_cancellation_after_verification_prevents_random_and_hash() {
        let control = Control {
            live: RuntimeLiveness::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: deadline(),
        };
        let result = replacement_stages(
            &control,
            &mut (),
            |_| {
                control.cancelled.store(true, Ordering::Release);
                Ok(true)
            },
            || panic!("randomness started after cancellation"),
            |_, _| -> Result<(), HashError> { panic!("hash started after cancellation") },
        );
        assert!(matches!(result, Err(HashError::Cancelled)));
    }

    #[test]
    fn replacement_cancellation_during_randomness_prevents_hash() {
        let control = Control {
            live: RuntimeLiveness::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: deadline(),
        };
        let result = replacement_stages(
            &control,
            &mut (),
            |_| Ok(true),
            || {
                control.cancelled.store(true, Ordering::Release);
                Ok([0; 16])
            },
            |_, _| -> Result<(), HashError> { panic!("hash started after cancellation") },
        );
        assert!(matches!(result, Err(HashError::Cancelled)));
    }

    #[test]
    fn new_and_replacement_hash_share_cancellation_after_randomness() {
        let control = Control {
            live: RuntimeLiveness::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: deadline(),
        };
        let result = salted_stage(
            &control,
            || {
                control.cancelled.store(true, Ordering::Release);
                Ok([0; 16])
            },
            |_| -> Result<(), HashError> { panic!("new hash started after cancellation") },
        );
        assert!(matches!(result, Err(HashError::Cancelled)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn real_verify_and_replacement_share_one_owned_workspace() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let current =
            NormalizedPassword::new(zeroize::Zeroizing::new("a fixed test password".to_owned()))
                .unwrap();
        let replacement = allowed("a different replacement");
        // Unoptimized mechanism test; five-second operating qualification uses
        // the separately measured release profile rather than this debug bound.
        let started = Instant::now();
        let pending = owner
            .replacement(
                Instant::now() + Duration::from_secs(30),
                current,
                replacement,
                reference_verifier(),
            )
            .unwrap();
        let outcome = pending.result().await.unwrap().unwrap();
        owner.join_active().unwrap();
        eprintln!(
            "replacement_work_elapsed_us={}",
            started.elapsed().as_micros()
        );
        let Change::Replacement(verifier) = outcome else {
            panic!("correct password rejected");
        };
        let expected = NormalizedPassword::new(zeroize::Zeroizing::new(
            "a different replacement".to_owned(),
        ))
        .unwrap();
        assert!(Hasher::new().unwrap().verify(&expected, &verifier).unwrap());
        owner.shutdown().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wrong_current_password_produces_no_replacement() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let current = NormalizedPassword::new(zeroize::Zeroizing::new(
            "the wrong test password".to_owned(),
        ))
        .unwrap();
        let replacement = allowed("a different replacement");
        let outcome = owner
            .replacement(
                Instant::now() + Duration::from_secs(30),
                current,
                replacement,
                reference_verifier(),
            )
            .unwrap()
            .result()
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, Change::Mismatch));
        owner.shutdown().unwrap();
    }

    struct Dropped(Arc<AtomicUsize>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn spawn_failure_drops_owned_inputs_and_leaves_no_active_slot() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let dropped = Arc::new(AtomicUsize::new(0));
        let witness = Dropped(dropped.clone());
        let result = owner.start_with(
            deadline(),
            move |_| {
                drop(witness);
                7
            },
            |_job| Err(io::Error::other("controlled spawn failure")),
        );
        assert!(matches!(result, Err(WorkerError::Unavailable)));
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert_eq!(
            owner
                .start(deadline(), |_| 3)
                .unwrap()
                .result()
                .await
                .unwrap(),
            3
        );
        owner.shutdown().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropping_an_old_pending_result_cannot_cancel_a_new_job() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let old = owner.start(deadline(), |_| 1).unwrap();
        owner.join_active().unwrap();
        let (release, gate) = mpsc::channel();
        let next = owner
            .start(deadline(), move |control| {
                gate.recv_timeout(Duration::from_secs(2)).unwrap();
                control.allowed()
            })
            .unwrap();
        drop(old);
        release.send(()).unwrap();
        assert!(next.result().await.unwrap());
        owner.shutdown().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ready_result_is_rejected_when_receiving_deadline_has_expired() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let mut pending = owner.start(deadline(), |_| 1).unwrap();
        owner.join_active().unwrap(); // Result is now ready, without timing sleeps.
        pending.deadline = Instant::now(); // Exercise the receiving coordinator guard.
        assert!(matches!(pending.result().await, Err(WorkerError::Expired)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shutdown_cancels_and_joins_before_owned_destructors_are_reported_done() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let dropped = Arc::new(AtomicUsize::new(0));
        let witness = Dropped(dropped.clone());
        let (started, ready) = mpsc::channel();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = cancelled.clone();
        let _pending = owner
            .start(deadline(), move |control| {
                let _witness = witness;
                started.send(()).unwrap();
                let guard = Instant::now() + Duration::from_secs(2);
                while control.allowed() && Instant::now() < guard {
                    std::thread::yield_now();
                }
                observed.store(!control.allowed(), Ordering::SeqCst);
            })
            .unwrap();
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        owner.shutdown().unwrap();
        assert!(cancelled.load(Ordering::SeqCst));
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancellation_never_releases_running_capacity() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let (started, ready) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let pending = owner
            .start(deadline(), move |control| {
                started.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(2)).unwrap();
                control.allowed()
            })
            .unwrap();
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(pending);
        assert!(matches!(
            owner.start(deadline(), |_| ()),
            Err(WorkerError::Busy)
        ));
        release.send(()).unwrap();
        owner.join_active().unwrap();
        assert_eq!(
            owner
                .start(deadline(), |_| 7)
                .unwrap()
                .result()
                .await
                .unwrap(),
            7
        );
        owner.shutdown().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_first_stage_never_starts_the_second_stage() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let (started, ready) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let second = Arc::new(AtomicUsize::new(0));
        let observed = second.clone();
        let pending = owner
            .start(deadline(), move |control| {
                started.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(2)).unwrap();
                if control.allowed() {
                    observed.fetch_add(1, Ordering::SeqCst);
                }
            })
            .unwrap();
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(pending);
        release.send(()).unwrap();
        owner.join_active().unwrap();
        assert_eq!(second.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn expired_admission_never_runs_work() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        assert!(matches!(
            owner.start(Instant::now(), move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
            }),
            Err(WorkerError::Expired)
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shutdown_is_terminal_and_joins_admitted_work() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let pending = owner
            .start(deadline(), move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        // Confirm completion before testing normal cleanup; no timing-only sleep.
        pending.result().await.unwrap();
        owner.shutdown().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            owner.start(deadline(), |_| ()),
            Err(WorkerError::Stopped)
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn panic_is_reported_and_the_owned_handle_is_joined() {
        let mut owner = Worker::new(RuntimeLiveness::new());
        let pending = owner
            .start(deadline(), |_| panic!("controlled worker panic"))
            .unwrap();
        assert!(matches!(pending.result().await, Err(WorkerError::Panicked)));
        assert!(matches!(owner.join_active(), Err(WorkerError::Panicked)));
        assert_eq!(
            owner
                .start(deadline(), |_| 9)
                .unwrap()
                .result()
                .await
                .unwrap(),
            9
        );
        owner.shutdown().unwrap();
    }
}
