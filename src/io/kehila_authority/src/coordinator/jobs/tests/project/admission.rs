//! Real worker/preparation ownership, separately labelled from TCP disconnect.
use super::*;
use crate::http_auth::{adapter, Profile};
use crate::lifecycle::probe::Snapshot as Admissions;
use std::{future::Future, pin::Pin};

struct Held {
    work: Pin<Box<dyn Future<Output = ()> + Send>>,
    release: std::sync::mpsc::Sender<()>,
}
async fn wait_free(handle: &Handle, count: usize) {
    let permits = tokio::time::timeout(
        WORK_DEADLINE,
        handle.capacity.clone().acquire_many_owned(count as u32),
    )
    .await
    .unwrap()
    .unwrap();
    drop(permits);
}
pub(super) fn credential(browser: &Browser) -> Credential {
    Credential {
        kind: Kind::Browser,
        bearer: Digest::from_encoded(
            browser
                .cookie
                .strip_prefix("__Host-kehila-session=")
                .unwrap()
                .as_bytes(),
        )
        .unwrap(),
        csrf: Some(Digest::from_encoded(browser.csrf.as_bytes()).unwrap()),
    }
}
async fn held(handle: &Handle, browser: &Browser, entry: &Value) -> Held {
    wait_free(handle, CAPACITY).await;
    assert_eq!(handle.preparations.active_for_test(), 0);
    assert_eq!(handle.capacity.available_permits(), CAPACITY);
    let engine = handle.engine.upgrade().unwrap();
    let gate = engine.hash.lock().await.hold_next_thread_exit_for_test();
    drop(engine);
    let request = wire_request(Some(browser), "/projects/execute", Some(execute(entry)));
    let jobs = handle.clone();
    let expires = deadline();
    let mut work: Pin<Box<dyn Future<Output = ()> + Send>> = Box::pin(async move {
        let profile = Profile::new("127.0.0.1:16281".parse().unwrap()).unwrap();
        let _ = adapter::handle(request, &profile, &jobs, expires).await;
    });
    tokio::select! {
        published=gate.published=>published.unwrap(),
        ()=&mut work=>panic!("adapter completed while its actual worker thread was held"),
    }
    assert_eq!(handle.capacity.available_permits(), CAPACITY - 1);
    assert_eq!(handle.preparations.active_for_test(), 1);
    Held {
        work,
        release: gate.release,
    }
}

pub(super) async fn ingress(handle: &Handle, browser: &Browser, entry: &Value) {
    let before = handle.live.probe.snapshot();
    let mut deep = Value::Null;
    for _ in 0..30 {
        deep = Value::Array(vec![deep]);
    }
    for (body, status) in [
        (
            json!({"name":"P","prefix":"PR","estimate_unit":"hours","actor_id":"spoof"}),
            400,
        ),
        (
            json!({"name":deep,"prefix":"PR","estimate_unit":"hours"}),
            400,
        ),
        (
            json!({"name":"x".repeat(4097),"prefix":"PR","estimate_unit":"hours"}),
            413,
        ),
        (
            json!({"name":"P","prefix":"lowercase","estimate_unit":"hours"}),
            400,
        ),
    ] {
        assert_eq!(project(handle, browser, "register", body).await.0, status);
        assert_eq!(
            handle.live.probe.snapshot(),
            before,
            "original-byte/domain rejection admitted application work"
        );
    }
    // The fault is scoped to this runtime/request, with no other eligible
    // preparation; the delta below also checks exactly one was registered.
    assert_eq!(handle.preparations.active_for_test(), 0);
    let fault = handle
        .live
        .probe
        .seed_overflow(entry["request_id"].as_str().unwrap().parse().unwrap());
    assert_eq!(
        project(handle, browser, "execute", execute(entry)).await.0,
        503
    );
    assert!(fault.consumed());
    assert_eq!(
        handle.live.probe.snapshot(),
        Admissions {
            preparations: before.preparations + 1,
            workers: before.workers + 1,
            ..before
        }
    );
    assert!(handle.live.is_live());
    drop(fault);
    let (status, opened) = project(
        handle,
        browser,
        "open",
        json!({"request_id":entry["request_id"]}),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(opened["entry"]["state"], "prepared");
}

pub(super) async fn receiver_drop_and_capacity(handle: &Handle, browser: &Browser, entry: &Value) {
    let before = handle.live.probe.snapshot();
    let Held { work, release } = held(handle, browser, entry).await;
    drop(work); // The boxed adapter future actually drops its preparation receiver.
    assert!(handle.live.is_live());
    assert_eq!(handle.capacity.available_permits(), CAPACITY - 1);
    assert_eq!(
        handle.live.probe.snapshot(),
        Admissions {
            preparations: before.preparations + 1,
            workers: before.workers + 1,
            ..before
        }
    );
    release.send(()).unwrap();
    let all = tokio::time::timeout(
        WORK_DEADLINE,
        handle.capacity.clone().acquire_many_owned(CAPACITY as u32),
    )
    .await
    .unwrap()
    .unwrap();
    drop(all);
    assert!(handle.live.is_live());
    assert_eq!(handle.preparations.active_for_test(), 0);

    // One completed but unsubmitted preparation shares the same budget with
    // fifteen actual completed/retained ordinary replies.
    let prepared = handle
        .prepare_project(
            credential(browser),
            crate::native_ingress::Input::Binding,
            deadline(),
        )
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    let unknown = Digest::from_encoded(&[b'A'; 43]).unwrap();
    let mut replies = Vec::new();
    for _ in 0..CAPACITY - 1 {
        replies.push(request(handle, Operation::State(cli(unknown))).await);
    }
    assert_eq!(handle.capacity.available_permits(), 0);
    assert!(matches!(
        handle.submit(Operation::State(cli(unknown)), deadline()),
        Err(Error::Busy)
    ));
    assert!(matches!(
        handle.prepare_project(
            credential(browser),
            crate::native_ingress::Input::Binding,
            deadline()
        ),
        Err(Error::Busy)
    ));
    // Submission transfers the existing reservation even when none are free.
    let response = handle.submit_prepared(prepared).unwrap().await.unwrap();
    assert!(matches!(response.result(), Ok(Output::Project(_))));
    assert_eq!(handle.capacity.available_permits(), 0);
    drop(replies);
    wait_free(handle, CAPACITY - 1).await;
    assert_eq!(handle.capacity.available_permits(), CAPACITY - 1);
    drop(response);
    wait_free(handle, CAPACITY).await;
    assert_eq!(handle.capacity.available_permits(), CAPACITY);
}

pub(super) async fn stop_and_abort(
    endpoint: &crate::postgres::Endpoint,
    installation: &str,
    attempt: &str,
    entry: &Value,
) {
    for exceptional in [false, true] {
        let (handle, owner) = runtime(
            endpoint,
            installation,
            format!(
                "{attempt}-prepare-{}",
                if exceptional { "abort" } else { "stop" }
            ),
        )
        .await;
        let running = tokio::spawn(owner.run());
        let browser = login(&handle).await;
        let Held { work, release } = held(&handle, &browser, entry).await;
        drop(work);
        if exceptional {
            handle.preparations.abort();
            tokio::time::timeout(WORK_DEADLINE, handle.live.stopped())
                .await
                .unwrap();
        } else {
            handle.stop();
        }
        tokio::time::timeout(WORK_DEADLINE, handle.live.probe.wait_settling())
            .await
            .unwrap();
        assert!(
            !running.is_finished(),
            "owner released a held actual worker"
        );
        assert_eq!(handle.capacity.available_permits(), CAPACITY - 1);
        release.send(()).unwrap();
        let result = running.await.unwrap();
        assert_eq!(result.is_err(), exceptional);
        assert_eq!(handle.capacity.available_permits(), CAPACITY);
        assert!(!handle.live.is_live());
    }
}
