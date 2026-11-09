//! Thread CPU intervals stay within one synchronous OS-worker closure.
use super::{Request, Span, Stage};
use rustix::time::{clock_gettime, ClockId, Timespec};
use std::{marker::PhantomData, rc::Rc};

pub(crate) struct CpuSpan {
    span: Span,
    started: Timespec,
    // Request/Span can cross owners. A thread-clock interval cannot migrate.
    _same_thread: PhantomData<Rc<()>>,
}

impl Request {
    pub(crate) fn cpu_span(&self) -> CpuSpan {
        // Start wall first and read the ending CPU clock before wall destruction.
        // Clock reads and observer bookkeeping still have measurable overhead.
        let span = self.span(Stage::CpuWork);
        let started = clock_gettime(ClockId::ThreadCPUTime);
        CpuSpan {
            span,
            started,
            _same_thread: PhantomData,
        }
    }
}

fn timestamp(value: Timespec) -> Option<u128> {
    let seconds = u128::try_from(value.tv_sec).ok()?;
    let nanos = u128::try_from(value.tv_nsec)
        .ok()
        .filter(|nanos| *nanos < 1_000_000_000)?;
    seconds.checked_mul(1_000_000_000)?.checked_add(nanos)
}

fn elapsed(start: Timespec, end: Timespec) -> Option<u64> {
    u64::try_from(timestamp(end)?.checked_sub(timestamp(start)?)?).ok()
}

impl CpuSpan {
    pub(crate) fn complete(mut self) {
        self.span.completed = true;
    }
}

impl Drop for CpuSpan {
    fn drop(&mut self) {
        let end = clock_gettime(ClockId::ThreadCPUTime);
        match elapsed(self.started, end) {
            Some(nanos) => self.span.cpu_time_ns = Some(nanos),
            None => self
                .span
                .request
                .invalidate("capacity thread CPU timestamp invalid"),
        }
        // The inner wall span drops exactly once after this method, including
        // during unwinding. No runtime resource or permit is owned here.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::probe::capacity::Collector;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn time(seconds: i64, nanos: i64) -> Timespec {
        Timespec {
            tv_sec: seconds,
            tv_nsec: nanos,
        }
    }

    #[test]
    fn capacity_thread_cpu_conversion_rejects_invalid_backward_and_wide_intervals() {
        assert_eq!(
            elapsed(time(2, 900_000_000), time(3, 100_000_000)),
            Some(200_000_000)
        );
        assert_eq!(elapsed(time(0, 0), time(0, 0)), Some(0));
        for invalid in [time(-1, 0), time(0, -1), time(0, 1_000_000_000)] {
            assert!(elapsed(invalid, time(1, 0)).is_none());
            assert!(elapsed(time(0, 0), invalid).is_none());
        }
        assert!(elapsed(time(1, 1), time(1, 0)).is_none());
        assert!(elapsed(time(0, 0), time(i64::MAX, 999_999_999)).is_none());
    }

    #[test]
    fn capacity_thread_cpu_unwind_preserves_original_failure_and_owned_cleanup() {
        struct Cleanup<'a>(&'a AtomicBool);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let collector = Collector::default();
        collector.start(1, 4);
        let request = collector.begin().unwrap();
        let cleaned = AtomicBool::new(false);
        let outcome = std::panic::catch_unwind(|| {
            let _cleanup = Cleanup(&cleaned);
            let _cpu = request.cpu_span();
            panic!("original CPU fixture failure");
        });
        assert_eq!(
            outcome.unwrap_err().downcast_ref::<&str>(),
            Some(&"original CPU fixture failure")
        );
        assert!(cleaned.load(Ordering::SeqCst));
        drop(request);
        let rows = collector.finish().unwrap();
        assert_eq!(rows[0].events.len(), 1);
        assert!(!rows[0].events[0].completed);
        assert!(rows[0].events[0].cpu_time_ns.is_some());
        assert!(rows[0].events[0].duration_ns.is_some());
    }

    #[test]
    fn capacity_thread_cpu_numeric_failure_invalidates_the_cell_without_panicking() {
        let collector = Collector::default();
        collector.start(1, 4);
        let request = collector.begin().unwrap();
        let mut cpu = request.cpu_span();
        cpu.started = time(-1, 0);
        cpu.complete();
        drop(request);
        assert_eq!(
            collector.finish().unwrap_err(),
            "capacity thread CPU timestamp invalid"
        );
    }
}
