//! Opt-in native backend for the separately controlled real-browser campaign.
//! This is not a protected-startup or provider-publication qualification.
use super::*;
use crate::lifecycle::probe::TransactionGate;
use crate::postgres::tests::fixture_admin::Admin;
use serde::Deserialize;
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const PORT: u16 = 16291;
const MAX_COMMANDS: u32 = 64;
const MAX_COMMAND_BYTES: u64 = 6 * 1024 * 1024 + 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    listen: std::net::Ipv4Addr,
    certificate_der: PathBuf,
    private_key_der: PathBuf,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Arm { wire: String },
    Release {},
    Snapshot {},
    Stop {},
}

fn read(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > limit {
        return Err(crate::io_error("invalid browser fixture input file"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(crate::io_error("browser fixture input exceeded limit"));
    }
    Ok(bytes)
}

fn publish(directory: &Path, name: &str, value: Value) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let temporary = directory.join(format!("{name}.partial"));
    let destination = directory.join(format!("{name}.json"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(&value)?)?;
    file.sync_all()?;
    drop(file);
    // Atomic publication with no replacement of retained evidence.
    fs::hard_link(&temporary, destination)?;
    fs::remove_file(temporary)
}

fn canonical_edit(wire: &str) -> io::Result<Vec<u8>> {
    use crate::item_mutation_ingress::{decode, Request, Route};
    if wire.len() > 1024 * 1024 {
        return Err(crate::io_error("browser edit wire exceeded limit"));
    }
    let input = decode(Route::Edit, wire.as_bytes())
        .map_err(|_| crate::io_error("invalid browser fixture edit"))?;
    let Request::Edit {
        project,
        item,
        command,
    } = input.request
    else {
        return Err(crate::io_error("browser fixture expected an edit"));
    };
    work_item_codec::admit_request(&work_item_codec::Request::Edit {
        project_id: task_contract::work_item::ProjectId(project),
        item_id: task_contract::work_item::WorkItemId(item),
        command,
    })
    .map(|record| record.as_bytes().to_vec())
    .map_err(|_| crate::io_error("browser edit canonicalization failed"))
}

fn digest(bytes: &[u8]) -> String {
    let mut encoded = [0; 64];
    data_encoding::HEXLOWER.encode_mut(&work_item_codec::fingerprint(bytes), &mut encoded);
    String::from_utf8(encoded.to_vec()).unwrap()
}

struct Barrier {
    sequence: u32,
    gate: TransactionGate,
    arrived: bool,
    canonical_sha256: String,
    wire_sha256: String,
}

async fn ready_before(
    ready: tokio::sync::oneshot::Receiver<()>,
    deadline: Instant,
) -> io::Result<()> {
    let expired = || {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "browser listener readiness expired",
        )
    };
    if Instant::now() >= deadline {
        return Err(expired());
    }
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), ready)
        .await
        .map_err(|_| expired())?
        .map_err(|_| crate::io_error("browser listener not ready"))?;
    if Instant::now() >= deadline {
        return Err(expired());
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn readiness_requires_a_live_deadline_even_when_already_ready() {
    let (sender, ready) = tokio::sync::oneshot::channel();
    sender.send(()).unwrap();
    ready_before(ready, Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
    let (sender, ready) = tokio::sync::oneshot::channel();
    sender.send(()).unwrap();
    let error = ready_before(ready, Instant::now()).await.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[tokio::test(flavor = "current_thread")]
async fn never_ready_listener_times_out_without_sender_disconnection() {
    let (sender, ready) = tokio::sync::oneshot::channel();
    let deadline = Instant::now() + Duration::from_millis(10);
    let error = tokio::time::timeout(Duration::from_secs(1), ready_before(ready, deadline))
        .await
        .expect("readiness wait escaped its bound")
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(Instant::now() >= deadline);
    assert!(sender.is_closed());
    drop(sender);
}

async fn controls(
    directory: &Path,
    handle: &Handle,
    admin: &Admin,
    deadline: Instant,
) -> io::Result<()> {
    let mut sequence = 1;
    let mut barrier: Option<Barrier> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    loop {
        tick.tick().await;
        if !handle.live.is_live() || Instant::now() >= deadline {
            return Err(crate::io_error("browser fixture lifetime ended"));
        }
        if let Some(active) = barrier.as_mut().filter(|active| !active.arrived) {
            match active.gate.arrived.try_recv() {
                Ok(()) => {
                    active.arrived = true;
                    publish(
                        directory,
                        &format!("arrived-{:04}", active.sequence),
                        json!({
                            "state":"before_commit", "canonical_sha256":active.canonical_sha256,
                            "wire_sha256":active.wire_sha256,
                            "outcome":"unknown", "deadline_extended":false
                        }),
                    )?;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
                Err(_) => {
                    return Err(crate::io_error(
                        "selected edit did not reach COMMIT barrier",
                    ))
                }
            }
        }
        let command_path = directory.join(format!("command-{sequence:04}.json"));
        let bytes = match read(&command_path, MAX_COMMAND_BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let command: Command = serde_json::from_slice(&bytes)?;
        let state = match command {
            Command::Arm { wire } => {
                if barrier.is_some() {
                    return Err(crate::io_error("browser fixture barrier already armed"));
                }
                let canonical = canonical_edit(&wire)?;
                let canonical_sha256 = digest(&canonical);
                let wire_sha256 = digest(wire.as_bytes());
                let gate = handle.live.probe.arm_item_commit(canonical);
                barrier = Some(Barrier {
                    sequence,
                    gate,
                    arrived: false,
                    canonical_sha256: canonical_sha256.clone(),
                    wire_sha256: wire_sha256.clone(),
                });
                json!({"state":"armed", "canonical_sha256":canonical_sha256,
                    "wire_sha256":wire_sha256})
            }
            Command::Release {} => {
                let active = barrier
                    .take()
                    .filter(|active| active.arrived)
                    .ok_or_else(|| crate::io_error("browser fixture release without arrival"))?;
                active
                    .gate
                    .release
                    .send(())
                    .map_err(|_| crate::io_error("browser fixture release owner disappeared"))?;
                json!({"state":"release_sent", "outcome":"unknown"})
            }
            Command::Snapshot {} => {
                if barrier.is_some() {
                    return Err(crate::io_error(
                        "browser fixture snapshot while barrier armed",
                    ));
                }
                let permits = tokio::time::timeout(
                    WORK_DEADLINE,
                    handle.capacity.clone().acquire_many_owned(CAPACITY as u32),
                )
                .await
                .map_err(|_| crate::io_error("browser fixture settlement expired"))?
                .map_err(|_| crate::io_error("browser fixture admission closed"))?;
                let tables = evidence::snapshot(admin).await;
                drop(permits);
                json!({"state":"settled_observed", "tables":tables})
            }
            Command::Stop {} => {
                // Dropping an arrived gate can release work. Only the final
                // joined SQL witness describes the resulting durable state.
                drop(barrier.take());
                publish(
                    directory,
                    &format!("reply-{sequence:04}"),
                    json!({"state":"stop_received", "outcome":"unknown"}),
                )?;
                return Ok(());
            }
        };
        publish(directory, &format!("reply-{sequence:04}"), state)?;
        sequence += 1;
        if sequence > MAX_COMMANDS {
            return Err(crate::io_error("browser fixture command limit reached"));
        }
    }
}

async fn seed(handle: &Handle) -> Value {
    let browser = login(handle).await;
    let (status, pending) = project(
        handle,
        &browser,
        "register",
        json!({
            "name":"Browser recovery fixture", "prefix":"BRF", "estimate_unit":"hours"
        }),
    )
    .await;
    assert_eq!(status, 201);
    let (status, created) = project(handle, &browser, "execute", execute(&pending["entry"])).await;
    assert_eq!(status, 201);
    let created =
        project_create_codec::decode_result(1, &serde_json::to_vec(&created["result"]).unwrap())
            .unwrap();
    let task = &created.configuration.types[0];
    let (status, pending, _) = call(
        handle,
        Some(&browser),
        "/items/pending/register",
        Some(json!({
            "project_id":created.project.id.0, "type_id":task.id.0,
            "workflow_id":null, "expected_configuration_revision":"1", "estimate":null,
            "values":[{"id":task.title_field_id.as_ref().unwrap().0,"value":"Before recovery"}]
        })),
    )
    .await;
    assert_eq!(status, 201);
    let (status, item, _) = call(
        handle,
        Some(&browser),
        "/items/pending/execute",
        Some(json!({
            "project_id":created.project.id.0, "intent_handle":pending["entry"]["intent_handle"]
        })),
    )
    .await;
    assert_eq!(status, 201);
    let work_item_codec::SavedResult::Create(item) =
        work_item_codec::decode_result(1, &serde_json::to_vec(&item["result"]).unwrap()).unwrap()
    else {
        panic!("fixture creation returned a different family")
    };
    json!({"project":created.project.id.0, "item":item.item.id.0,
        "title_field":task.title_field_id.as_ref().unwrap().0})
}

#[test]
#[ignore = "requires an explicitly owned native fixture, TLS route and real-browser controller"]
fn native_browser_recovery_fixture() {
    use std::os::unix::fs::DirBuilderExt;
    assert_eq!(
        std::env::var("KEHILA_NATIVE_FIXTURE_DATABASE").unwrap(),
        "native_item_mutation"
    );
    let (endpoint, installation, attempt, controls_root) = fixture();
    let configuration: Configuration = serde_json::from_slice(
        &read(&controls_root.join("browser-configuration.json"), 4096).unwrap(),
    )
    .unwrap();
    assert!(
        configuration.certificate_der.is_absolute() && configuration.private_key_der.is_absolute()
    );
    let certificate = read(&configuration.certificate_der, 16384).unwrap();
    let key = read(&configuration.private_key_der, 8192).unwrap();
    let directory = controls_root.join("browser");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let endpoint = Arc::new(endpoint);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (handle, owner) =
                runtime(&endpoint, &installation, format!("{attempt}-browser")).await;
            let running = tokio::spawn(owner.run());
            // Setup, readiness and commands share one work allowance. The
            // listener's later cutoff exists only to permit owned shutdown.
            let work_deadline = Instant::now() + Duration::from_secs(900);
            let serving_deadline = work_deadline + Duration::from_secs(30);
            let listener = crate::listener::tests::browser::browser_listener(
                handle.clone(),
                std::net::SocketAddrV4::new(configuration.listen, PORT),
                certificate,
                key,
                serving_deadline,
            );
            let work_handle = handle.clone();
            let work_endpoint = endpoint.clone();
            let work_directory = directory.clone();
            let mut work = tokio::spawn(async move {
                // Panics in legacy seeding/admin helpers become classified task
                // failures. The outer fixture retains both serving owners.
                let admin = Admin::connect(&work_endpoint).await;
                let result = async {
                let counts = admin.client().query_one(
                    "SELECT (SELECT count(*) FROM kehila.projects), (SELECT count(*) FROM kehila.operations)", &[])
                    .await.map_err(|_| crate::io_error("browser fixture freshness unavailable"))?;
                if counts.get::<_, i64>(0) != 0 || counts.get::<_, i64>(1) != 0 {
                    return Err(crate::io_error("browser fixture database is not fresh"));
                }
                    let seeded = seed(&work_handle).await;
                    ready_before(listener.ready, work_deadline).await?;
                    publish(
                        &work_directory,
                        "ready",
                        json!({"state":"ready",
                    "origin":"https://127.0.0.1:16291", "seed":seeded,
                    "tables":evidence::snapshot(&admin).await, "protected_startup":false}),
                    )?;
                    controls(&work_directory, &work_handle, &admin, work_deadline).await
                }
                .await;
                admin.close().await;
                result
            });
            let controller_ok =
                match tokio::time::timeout_at(tokio::time::Instant::from_std(work_deadline), &mut work).await {
                    Ok(Ok(Ok(()))) => "completed",
                    Ok(Ok(Err(_))) => "protocol_failed",
                    Ok(Err(_)) => "task_failed",
                    Err(_) => {
                        work.abort();
                        let _ = work.await;
                        "forced_abort"
                    }
                };
            // Error/stop can release the selected edit. Join all owned work, then
            // retain the independent final state even when the controller failed.
            let _ = listener.stop.send(());
            handle.stop();
            let listener_result = join(listener.running).await;
            let owner_result = join(running).await;
            // A new independent admin task also captures unavailable-witness
            // failures without skipping the preceding owner cleanup attempts.
            let witness = tokio::spawn(async move {
                let admin = Admin::connect(&endpoint).await;
                evidence::runtime_released(&admin).await;
                let tables = evidence::snapshot(&admin).await;
                admin.close().await;
                tables
            });
            let witness = join(witness).await;
            let listener_ok = listener_result
                .as_ref()
                .is_ok_and(|value| value.served.is_ok() && !value.forced_connections);
            let owner_ok = owner_result.as_ref().is_ok_and(|result| result.is_ok());
            let shutdown = if listener_ok && owner_ok && witness.is_ok() {
                "shutdown_observed"
            } else {
                "shutdown_failed"
            };
            publish(
                &directory,
                "final",
                json!({"state":shutdown,
            "controller":controller_ok, "listener_ok":listener_ok,
            "listener_join":listener_result.as_ref().err(),
            "forced_connections":listener_result.as_ref().ok().map(|value|value.forced_connections),
            "owner_ok":owner_ok, "owner_join":owner_result.as_ref().err(),
            "witness_unavailable":witness.as_ref().err(), "tables":witness.as_ref().ok()}),
            )
            .unwrap();
            assert_eq!(controller_ok, "completed");
            assert!(
                listener_ok && owner_ok && witness.is_ok(),
                "browser fixture cleanup/witness failed"
            );
        });
}

async fn join<T: Send + 'static>(mut task: tokio::task::JoinHandle<T>) -> Result<T, &'static str> {
    match tokio::time::timeout(Duration::from_secs(10), &mut task).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err("task_failed"),
        Err(_) => {
            task.abort();
            let _ = task.await;
            Err("forced_abort")
        }
    }
}

#[test]
fn exact_edit_association_preserves_wire_digest_and_rejects_extra_keys() {
    let wire = r#"{"project_id":"project","item_id":"item","operation_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","expected_item_version":"1","expected_configuration_revision":"1","estimate":{"action":"keep"},"fields":[]}"#;
    let canonical = canonical_edit(wire).unwrap();
    assert_ne!(digest(wire.as_bytes()), digest(&canonical));
    let changed = wire.replace("aaaaaaaa", "bbbbbbbb");
    assert_ne!(canonical, canonical_edit(&changed).unwrap());
    for action in ["release", "snapshot", "stop"] {
        assert!(serde_json::from_value::<Command>(json!({"action":action})).is_ok());
        assert!(
            serde_json::from_value::<Command>(json!({"action":action,"wire":"unwanted"})).is_err()
        );
    }
    assert!(serde_json::from_str::<Command>(r#"{"action":"release","action":"stop"}"#).is_err());
}
