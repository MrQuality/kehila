use super::*;
use crate::database::tests::{fixture, mark, wait};

fn password(value: &str) -> NormalizedPassword {
    NormalizedPassword::new(Zeroizing::new(value.to_owned())).unwrap()
}
fn deadline() -> Instant {
    Instant::now() + WORK_DEADLINE
}
async fn request(handle: &Handle, operation: Operation) -> Response {
    let label = match &operation {
        Operation::PreSession(_) => "pre_session",
        Operation::Login { .. } => "login",
        Operation::State(_) => "state",
        Operation::Logout(_) => "logout",
        Operation::Password { .. } => "password_change",
    };
    let started = Instant::now();
    let response =
        tokio::time::timeout(WORK_DEADLINE, handle.submit(operation, deadline()).unwrap())
            .await
            .unwrap()
            .unwrap();
    eprintln!(
        "owned_job {label} elapsed_us={}",
        started.elapsed().as_micros()
    );
    response
}
fn cli(bearer: Digest) -> Credential {
    Credential {
        bearer,
        kind: Kind::Cli,
        csrf: None,
    }
}
fn issued(response: &mut Response) -> Digest {
    let Output::Issued(delivery) = response.result().unwrap() else {
        panic!("login did not issue a credential")
    };
    let bearer = Digest::from_encoded(delivery.material().unwrap().0).unwrap();
    // This fixture's observed typed-reply boundary is not an HTTP observation.
    response.confirm_exchange().unwrap();
    bearer
}

#[test]
#[ignore = "requires the explicitly provisioned disposable PostgreSQL TLS fixture"]
fn native_owned_login() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let database = Database::start(
                &endpoint,
                installation.clone(),
                attempt.clone(),
                Instant::now() + Duration::from_secs(30),
            )
            .await
            .unwrap();
            let (handle, owner) = Owner::new(database, Blocklist::synthetic_for_tests())
                .await
                .unwrap();
            let running = tokio::spawn(owner.run());
            let unknown = Secret::generate().unwrap().digest();

            // Actual completed jobs retain capacity through their buffered replies.
            let mut replies = Vec::new();
            for _ in 0..CAPACITY {
                let response = request(&handle, Operation::State(cli(unknown))).await;
                assert!(matches!(response.result(), Err(Error::Unauthorized)));
                replies.push(response);
            }
            assert!(matches!(
                handle.submit(Operation::State(cli(unknown)), deadline()),
                Err(Error::Busy)
            ));
            drop(replies.pop());
            let available =
                tokio::time::timeout(WORK_DEADLINE, handle.capacity.clone().acquire_owned())
                    .await
                    .unwrap()
                    .unwrap();
            drop(available);
            assert!(matches!(
                request(&handle, Operation::State(cli(unknown)))
                    .await
                    .result(),
                Err(Error::Unauthorized)
            ));
            drop(replies);

            let wrong = request(
                &handle,
                Operation::Login {
                    password: password("this password is wrong"),
                    kind: LoginKind::Cli,
                },
            )
            .await;
            assert!(matches!(wrong.result(), Err(Error::Unauthorized)));
            drop(wrong);

            // Abandon the receiver itself. The manager must finish and join the
            // admitted job, retire its unused output and return accounted capacity.
            let abandoned = handle
                .submit(
                    Operation::Login {
                        password: password("a fixed test password"),
                        kind: LoginKind::Cli,
                    },
                    deadline(),
                )
                .unwrap();
            drop(abandoned);
            let settled = tokio::time::timeout(
                WORK_DEADLINE,
                handle.capacity.clone().acquire_many_owned(CAPACITY as u32),
            )
            .await
            .unwrap()
            .unwrap();
            drop(settled);

            let mut response = request(
                &handle,
                Operation::Login {
                    password: password("a fixed test password"),
                    kind: LoginKind::Cli,
                },
            )
            .await;
            let cli_bearer = issued(&mut response);
            drop(response);
            assert!(matches!(
                request(&handle, Operation::State(cli(cli_bearer)))
                    .await
                    .result(),
                Ok(Output::State { active: true, .. })
            ));

            let response = request(&handle, Operation::PreSession(None)).await;
            let Output::PreSession {
                bearer: Some(pre),
                csrf,
            } = response.result().unwrap()
            else {
                panic!("missing pre-session")
            };
            let pre = pre.digest();
            let csrf = Digest::from_encoded(csrf.as_slice()).unwrap();
            drop(response);
            let response = request(&handle, Operation::PreSession(Some(pre))).await;
            let Output::PreSession {
                bearer: None,
                csrf: reused,
            } = response.result().unwrap()
            else {
                panic!("pre-session rotated")
            };
            assert!(Digest::from_encoded(reused.as_slice())
                .unwrap()
                .matches(&csrf));
            drop(response);
            let response = request(
                &handle,
                Operation::Login {
                    password: password("a fixed test password"),
                    kind: LoginKind::Browser { bearer: pre, csrf },
                },
            )
            .await;
            let mut retained = response;
            let Output::Issued(delivery) = retained.result().unwrap() else {
                panic!("missing browser credential")
            };
            let browser_bearer = Digest::from_encoded(delivery.material().unwrap().0).unwrap();
            let response = request(
                &handle,
                Operation::State(Credential {
                    bearer: browser_bearer,
                    kind: Kind::Browser,
                    csrf: None,
                }),
            )
            .await;
            assert!(matches!(
                response.result(),
                Ok(Output::State {
                    active: true,
                    csrf: Some(_)
                })
            ));
            drop(response);

            let response = request(
                &handle,
                Operation::Password {
                    credential: cli(cli_bearer),
                    current: password("a fixed test password"),
                    replacement: password("a newly chosen owner password"),
                },
            )
            .await;
            assert!(matches!(response.result(), Ok(Output::PasswordChanged)));
            drop(response);
            let Output::Issued(delivery) = retained.result().unwrap() else {
                panic!("missing retained credential")
            };
            assert!(delivery.material().is_err());
            assert!(retained.confirm_exchange().is_err());
            drop(retained);
            assert!(matches!(
                request(&handle, Operation::State(cli(cli_bearer)))
                    .await
                    .result(),
                Err(Error::Unauthorized)
            ));
            assert!(matches!(
                request(
                    &handle,
                    Operation::State(Credential {
                        bearer: browser_bearer,
                        kind: Kind::Browser,
                        csrf: None
                    })
                )
                .await
                .result(),
                Err(Error::Unauthorized)
            ));
            let mut response = request(
                &handle,
                Operation::Login {
                    password: password("a newly chosen owner password"),
                    kind: LoginKind::Cli,
                },
            )
            .await;
            let next_bearer = issued(&mut response);
            drop(response);
            assert!(matches!(
                request(&handle, Operation::Logout(cli(next_bearer)))
                    .await
                    .result(),
                Ok(Output::LoggedOut)
            ));
            assert!(matches!(
                request(&handle, Operation::Logout(cli(next_bearer)))
                    .await
                    .result(),
                Ok(Output::LoggedOut)
            ));
            assert!(matches!(
                request(&handle, Operation::State(cli(next_bearer)))
                    .await
                    .result(),
                Ok(Output::State { active: false, .. })
            ));
            handle.stop();
            assert!(running.await.unwrap().is_ok());

            let database = Database::start(
                &endpoint,
                installation,
                format!("{attempt}-restart"),
                Instant::now() + Duration::from_secs(30),
            )
            .await
            .unwrap();
            let (handle, owner) = Owner::new(database, Blocklist::synthetic_for_tests())
                .await
                .unwrap();
            let running = tokio::spawn(owner.run());
            assert!(matches!(
                request(&handle, Operation::State(cli(next_bearer)))
                    .await
                    .result(),
                Err(Error::Unauthorized)
            ));
            let mut response = request(
                &handle,
                Operation::Login {
                    password: password("a newly chosen owner password"),
                    kind: LoginKind::Cli,
                },
            )
            .await;
            let _ = issued(&mut response);
            // Retain a response across actual authority-connection loss. Both the
            // manager and captured adapter reply must observe the same stop state.
            mark(&controls, "authority-ready");
            wait(&controls, "authority-lost").await;
            assert!(running.await.unwrap().is_err());
            assert!(!handle.live.is_live());
            assert!(matches!(response.result(), Err(Error::Unavailable)));
            assert!(matches!(
                handle.submit(Operation::State(cli(next_bearer)), deadline()),
                Err(Error::Unavailable)
            ));
            drop(response);
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                let peak = status
                    .lines()
                    .find(|line| line.starts_with("VmHWM:"))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .and_then(|value| value.parse::<u64>().ok());
                if let Some(peak) = peak {
                    eprintln!("owned_job_process_peak_kib={peak}");
                }
            }
            mark(&controls, "passed");
        });
}
