//! Direct edge preparation is tracked separately from execution submission.
//! Its barrier adds no execution capacity and is never an admission wait queue.
use super::{Error, RuntimeLiveness, CAPACITY};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    time::Instant,
};
use tokio::{
    sync::{oneshot, OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
};

pub(super) struct Reservation {
    pub(super) permit: Arc<OwnedSemaphorePermit>,
    pub(super) deadline: Instant,
    #[cfg(test)]
    pub(super) capacity: Option<crate::lifecycle::probe::capacity::Request>,
}
pub(crate) struct Prepared<T> {
    pub(super) value: T,
    pub(super) reservation: Reservation,
}

pub(super) struct Tracker {
    open: Mutex<bool>,
    slots: Arc<Semaphore>,
    tasks: Mutex<JoinSet<()>>,
    failed: AtomicBool,
}
pub(super) struct Settlement {
    _barrier: OwnedSemaphorePermit,
    pub(super) failed: bool,
}

pub(super) struct Active<E> {
    // Declaration order matters: the strong engine reference must be gone before
    // the barrier permit signals that owner teardown can unwrap the engine.
    pub(super) engine: Arc<E>,
    _barrier: OwnedSemaphorePermit,
    pub(super) reservation: Reservation,
}

impl<E> Active<E> {
    pub(super) fn finish(self) -> Reservation {
        self.reservation
    }
}

impl Tracker {
    #[cfg(test)]
    pub(super) fn active_for_test(&self) -> usize {
        CAPACITY - self.slots.available_permits()
    }
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            open: Mutex::new(true),
            slots: Arc::new(Semaphore::new(CAPACITY)),
            tasks: Mutex::new(JoinSet::new()),
            failed: AtomicBool::new(false),
        })
    }

    pub(super) fn spawn<E, T, F, U>(
        &self,
        active: Active<E>,
        live: Arc<RuntimeLiveness>,
        make: F,
    ) -> Result<oneshot::Receiver<Result<Prepared<T>, Error>>, Error>
    where
        E: Send + Sync + 'static,
        T: Send + 'static,
        F: FnOnce(Arc<E>, Arc<OwnedSemaphorePermit>, Instant) -> U + Send + 'static,
        U: Future<Output = Result<T, Error>> + Send + 'static,
    {
        #[cfg(test)]
        let capacity = active.reservation.capacity.clone();
        // Keep the same lock order as registration: open, then task set. There
        // is no await between begin and storing the owned continuation.
        let open = self.open.lock().map_err(|_| {
            self.failed.store(true, Ordering::Release);
            live.stop();
            Error::Unavailable
        })?;
        if !*open || !live.is_live() || Instant::now() >= active.reservation.deadline {
            return Err(Error::Unavailable);
        }
        let mut tasks = self.tasks.lock().map_err(|_| {
            self.failed.store(true, Ordering::Release);
            live.stop();
            Error::Unavailable
        })?;
        while let Some(joined) = tasks.try_join_next() {
            if joined.is_err() {
                self.failed.store(true, Ordering::Release);
                live.stop();
            }
        }
        if !live.is_live() {
            return Err(Error::Unavailable);
        }
        if tasks.len() >= CAPACITY {
            #[cfg(test)]
            if let Some(request) = &capacity {
                request.event(crate::lifecycle::probe::capacity::Stage::PreparationBusy);
            }
            return Err(Error::Busy);
        }
        let (sender, receiver) = oneshot::channel();
        let work = async move {
            let mut failure = super::StopOnAbandonment {
                live: live.clone(),
                completed: false,
            };
            let result = make(
                active.engine.clone(),
                active.reservation.permit.clone(),
                active.reservation.deadline,
            )
            .await;
            // A disconnected transport owns no work. Its undelivered sealed
            // result/reservation is dropped after the real preparation finishes.
            let result = result.map(|value| Prepared {
                value,
                reservation: active.finish(),
            });
            let _ = sender.send(result);
            failure.completed = true;
        };
        #[cfg(test)]
        tasks.spawn(crate::lifecycle::probe::capacity::scope(
            capacity,
            async move {
                crate::lifecycle::probe::capacity::event(
                    crate::lifecycle::probe::capacity::Stage::Preparation,
                );
                work.await
            },
        ));
        #[cfg(not(test))]
        tasks.spawn(work);
        Ok(receiver)
    }

    pub(super) fn begin<E>(
        &self,
        engine: &Weak<E>,
        live: &RuntimeLiveness,
        reservation: Reservation,
    ) -> Result<Active<E>, Error> {
        let open = self.open.lock().map_err(|_| {
            self.failed.store(true, Ordering::Release);
            live.stop();
            Error::Unavailable
        })?;
        if !*open || !live.is_live() || Instant::now() >= reservation.deadline {
            return Err(Error::Unavailable);
        }
        let barrier = self.slots.clone().try_acquire_owned().map_err(|_| {
            #[cfg(test)]
            if let Some(request) = &reservation.capacity {
                request.event(crate::lifecycle::probe::capacity::Stage::PreparationBusy);
            }
            Error::Busy
        })?;
        let engine = engine.upgrade().ok_or(Error::Unavailable)?;
        if !live.is_live() || Instant::now() >= reservation.deadline {
            return Err(Error::Unavailable);
        }
        Ok(Active {
            engine,
            _barrier: barrier,
            reservation,
        })
    }

    pub(super) fn close(&self) {
        // Closing is cleanup only. Poisoned registration is never reopened or
        // used to admit work, but must not prevent joining previously owned work.
        *self.open.lock().unwrap_or_else(|poison| {
            self.failed.store(true, Ordering::Release);
            poison.into_inner()
        }) = false;
    }

    pub(super) async fn settle(&self) -> Result<Settlement, Error> {
        if *self
            .open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            return Err(Error::Unavailable);
        }
        let mut tasks = {
            let mut owned = self.tasks.lock().unwrap_or_else(|poison| {
                self.failed.store(true, Ordering::Release);
                poison.into_inner()
            });
            std::mem::replace(&mut *owned, JoinSet::new())
        };
        while let Some(joined) = tasks.join_next().await {
            if joined.is_err() {
                self.failed.store(true, Ordering::Release);
            }
        }
        // Do not close this semaphore: closed semaphores reject acquisition
        // immediately rather than waiting for already-held permits to return.
        let barrier = self
            .slots
            .clone()
            .acquire_many_owned(CAPACITY as u32)
            .await
            .map_err(|_| Error::Unavailable)?;
        Ok(Settlement {
            _barrier: barrier,
            failed: self.failed.load(Ordering::Acquire),
        })
    }

    pub(super) fn abort(&self) {
        self.close();
        self.tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .abort_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::{poll_fn, Future},
        task::Poll,
        time::Duration,
    };

    fn reserve(capacity: &Arc<Semaphore>) -> Reservation {
        Reservation {
            permit: Arc::new(capacity.clone().try_acquire_owned().unwrap()),
            deadline: Instant::now() + Duration::from_secs(5),
            capacity: None,
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shutdown_waits_until_registered_engine_references_are_gone() {
        let tracker = Tracker::new();
        let engine = Arc::new(7);
        let weak = Arc::downgrade(&engine);
        let capacity = Arc::new(Semaphore::new(CAPACITY));
        let live = RuntimeLiveness::new();
        let active = tracker.begin(&weak, &live, reserve(&capacity)).unwrap();
        tracker.close();
        assert!(tracker.begin(&weak, &live, reserve(&capacity)).is_err());
        let mut settled = Box::pin(tracker.settle());
        assert!(poll_fn(|cx| Poll::Ready(settled.as_mut().poll(cx)))
            .await
            .is_pending());
        assert_eq!(Arc::strong_count(&engine), 2);
        drop(active);
        let _barrier = settled.await.unwrap();
        assert_eq!(Arc::try_unwrap(engine), Ok(7));
        assert_eq!(capacity.available_permits(), CAPACITY);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn preparation_and_retained_submission_share_the_existing_capacity() {
        let tracker = Tracker::new();
        let engine = Arc::new(7);
        let weak = Arc::downgrade(&engine);
        let capacity = Arc::new(Semaphore::new(CAPACITY));
        let live = RuntimeLiveness::new();
        let mut active = Vec::new();
        for _ in 0..CAPACITY {
            active.push(tracker.begin(&weak, &live, reserve(&capacity)).unwrap());
        }
        assert!(capacity.clone().try_acquire_owned().is_err());
        let submission = active.pop().unwrap().finish();
        assert_eq!(capacity.available_permits(), 0);
        drop(active);
        tracker.close();
        let _barrier = tracker.settle().await.unwrap();
        assert_eq!(Arc::strong_count(&engine), 1);
        assert_eq!(capacity.available_permits(), CAPACITY - 1);
        drop(submission);
        assert_eq!(capacity.available_permits(), CAPACITY);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stopped_expired_and_missing_owner_never_register_preparation() {
        let tracker = Tracker::new();
        let engine = Arc::new(7);
        let weak = Arc::downgrade(&engine);
        let capacity = Arc::new(Semaphore::new(CAPACITY));
        let live = RuntimeLiveness::new();
        let mut expired = reserve(&capacity);
        expired.deadline = Instant::now();
        assert!(tracker.begin(&weak, &live, expired).is_err());
        assert!(tracker
            .begin(&Weak::<i32>::new(), &live, reserve(&capacity))
            .is_err());
        live.stop();
        assert!(tracker.begin(&weak, &live, reserve(&capacity)).is_err());
        tracker.close();
        let _barrier = tracker.settle().await.unwrap();
        assert_eq!(Arc::strong_count(&engine), 1);
        assert_eq!(capacity.available_permits(), CAPACITY);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn poisoned_registration_denies_work_but_does_not_skip_owned_settlement() {
        let tracker = Tracker::new();
        let failed = tracker.clone();
        assert!(std::thread::spawn(move || {
            let _guard = failed.open.lock().unwrap();
            panic!("controlled registration failure");
        })
        .join()
        .is_err());
        let engine = Arc::new(7);
        let live = RuntimeLiveness::new();
        let capacity = Arc::new(Semaphore::new(CAPACITY));
        assert!(tracker
            .begin(&Arc::downgrade(&engine), &live, reserve(&capacity))
            .is_err());
        tracker.close();
        let _barrier = tracker.settle().await.unwrap();
        assert_eq!(capacity.available_permits(), CAPACITY);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn disconnected_receiver_does_not_abandon_owned_preparation() {
        let tracker = Tracker::new();
        let engine = Arc::new(7);
        let capacity = Arc::new(Semaphore::new(CAPACITY));
        let live = RuntimeLiveness::new();
        let active = tracker
            .begin(&Arc::downgrade(&engine), &live, reserve(&capacity))
            .unwrap();
        let (started, observed) = oneshot::channel();
        let (release, held) = oneshot::channel();
        let receiver = tracker
            .spawn(
                active,
                live.clone(),
                move |engine, _permit, _deadline| async move {
                    started.send(()).unwrap();
                    held.await.unwrap();
                    assert_eq!(*engine, 7);
                    Ok(())
                },
            )
            .unwrap();
        observed.await.unwrap();
        drop(receiver);
        assert!(live.is_live());
        assert_eq!(capacity.available_permits(), CAPACITY - 1);
        tracker.close();
        let mut settled = Box::pin(tracker.settle());
        assert!(poll_fn(|cx| Poll::Ready(settled.as_mut().poll(cx)))
            .await
            .is_pending());
        release.send(()).unwrap();
        let settled = settled.await.unwrap();
        assert!(!settled.failed);
        assert!(live.is_live());
        assert_eq!(capacity.available_permits(), CAPACITY);
        assert_eq!(Arc::strong_count(&engine), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    #[cfg(target_os = "linux")]
    async fn capacity_context_crosses_real_preparation_and_cpu_owners_without_retaining_permits() {
        use crate::lifecycle::probe::capacity::{self as evidence, Stage};
        use crate::password::worker::Worker;
        let tracker = Tracker::new();
        let engine = Arc::new(7);
        let capacity = Arc::new(Semaphore::new(CAPACITY));
        let live = RuntimeLiveness::new();
        live.probe.capacity.start(1, 16);
        let mut reservation = reserve(&capacity);
        let original_permit = Arc::downgrade(&reservation.permit);
        reservation.capacity = live.probe.capacity.begin();
        let active = tracker
            .begin(&Arc::downgrade(&engine), &live, reservation)
            .unwrap();
        let worker_live = live.clone();
        let receiver = tracker
            .spawn(
                active,
                live.clone(),
                move |_, permit, deadline| async move {
                    assert!(
                        evidence::capture().is_some(),
                        "preparation lost its original context"
                    );
                    let mut worker = Worker::new(worker_live);
                    let pending = worker.owned_preparation(deadline, permit, || 27).unwrap();
                    let value = pending.result().await.unwrap().unwrap();
                    while !worker.finished() {
                        tokio::task::yield_now().await;
                    }
                    worker.shutdown().unwrap();
                    Ok(value)
                },
            )
            .unwrap();
        let prepared = receiver.await.unwrap().unwrap();
        assert_eq!(prepared.value, 27);
        assert!(
            live.probe.capacity.finish().is_err(),
            "live prepared result was frozen"
        );
        drop(prepared);
        tracker.close();
        let settled = tracker.settle().await.unwrap();
        assert!(!settled.failed);
        assert!(
            original_permit.upgrade().is_none(),
            "evidence retained original admission"
        );
        assert_eq!(capacity.available_permits(), CAPACITY);
        let rows = live.probe.capacity.finish().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0]
                .events
                .iter()
                .map(|event| event.stage)
                .collect::<Vec<_>>(),
            [Stage::Preparation, Stage::CpuWork]
        );
        assert!(rows[0].events[1].completed);
        assert!(rows[0].events[1].cpu_time_ns.is_some());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn failed_preparation_is_joined_and_reported_after_its_reservations_are_released() {
        let tracker = Tracker::new();
        let engine = Arc::new(7);
        let capacity = Arc::new(Semaphore::new(CAPACITY));
        let live = RuntimeLiveness::new();
        let active = tracker
            .begin(&Arc::downgrade(&engine), &live, reserve(&capacity))
            .unwrap();
        let receiver = tracker
            .spawn::<_, (), _, _>(active, live.clone(), |_, _, _| async move {
                panic!("controlled preparation task failure");
            })
            .unwrap();
        assert!(receiver.await.is_err());
        tracker.close();
        let settled = tracker.settle().await.unwrap();
        assert!(settled.failed);
        assert!(!live.is_live());
        assert_eq!(capacity.available_permits(), CAPACITY);
        assert_eq!(Arc::strong_count(&engine), 1);
    }
}
