use std::{
    net::TcpListener,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const SUFFIXES: &[&str] = &[
    "MONGO_URL",
    "TASK_DB",
    "TASK_COLLECTION",
    "OPERATION_ADMISSION_DAYS",
    "OPERATION_REPLAY_DAYS",
    "WORKER_LISTEN_ADDR",
];

fn retired_key(suffix: &str) -> String {
    format!("{}_{suffix}", concat!("YA", "JA"))
}

fn startup(process: &mut Command) -> Output {
    let mut child = process
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "startup did not fail promptly: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn retired_settings_fail_before_database_access_for_every_command() {
    for command in [None, Some("install-indexes"), Some("drop-test-collection")] {
        for suffix in SUFFIXES {
            for value in ["", "secret-old-value"] {
                for current_present in [false, true] {
                    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                    listener.set_nonblocking(true).unwrap();
                    let mut process = Command::new(env!("CARGO_BIN_EXE_task_worker"));
                    for name in SUFFIXES {
                        process.env_remove(retired_key(name));
                        process.env_remove(format!("KEHILA_{name}"));
                    }
                    if *suffix != "MONGO_URL" || current_present {
                        process.env(
                            "KEHILA_MONGO_URL",
                            format!("mongodb://{}", listener.local_addr().unwrap()),
                        );
                    }
                    process.env(retired_key(suffix), value);
                    if current_present && *suffix != "MONGO_URL" {
                        process.env(format!("KEHILA_{suffix}"), "current-secret");
                    }
                    if let Some(command) = command {
                        process.arg(command);
                    }
                    let output = startup(&mut process);
                    assert!(!output.status.success());
                    let diagnostics = String::from_utf8_lossy(&output.stderr);
                    assert!(
                        diagnostics.contains(&format!(
                            "retired environment variable {}; use KEHILA_{suffix}",
                            retired_key(suffix)
                        )),
                        "{suffix}: {diagnostics}"
                    );
                    assert!(!diagnostics.contains("secret-old-value"));
                    assert!(!diagnostics.contains("current-secret"));
                    assert_eq!(
                        listener.accept().unwrap_err().kind(),
                        std::io::ErrorKind::WouldBlock
                    );
                }
            }
        }
    }
}

#[test]
fn unrelated_retired_prefix_is_ignored_and_current_configuration_is_read() {
    let mut process = Command::new(env!("CARGO_BIN_EXE_task_worker"));
    for suffix in SUFFIXES {
        process.env_remove(retired_key(suffix));
        process.env_remove(format!("KEHILA_{suffix}"));
    }
    process.env(retired_key("UNRELATED_SETTING"), "secret");
    process.env("KEHILA_MONGO_URL", "mongodb://127.0.0.1:1");
    process.env("KEHILA_OPERATION_ADMISSION_DAYS", "0");
    let output = startup(&mut process);
    assert!(!output.status.success());
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostics.contains("operation admission and replay periods must be positive"),
        "{diagnostics}"
    );
    assert!(!diagnostics.contains("retired environment variable"));
}

#[test]
fn multiple_retired_settings_report_the_first_key_in_a_fixed_order() {
    let mut process = Command::new(env!("CARGO_BIN_EXE_task_worker"));
    for suffix in SUFFIXES {
        process.env(retired_key(suffix), "secret");
    }
    let output = startup(&mut process);
    assert!(!output.status.success());
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostics.contains(&format!(
            "retired environment variable {}; use KEHILA_MONGO_URL",
            retired_key("MONGO_URL")
        )),
        "{diagnostics}"
    );
    assert!(!diagnostics.contains("secret"));
}
