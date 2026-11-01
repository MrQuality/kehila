use super::*;
use crate::database::tests::{fixture, mark, wait};
mod delivery;
mod expiry;
mod revoke_others;
mod unknown_commit;

fn password(value: &str) -> NormalizedPassword {
    NormalizedPassword::new(Zeroizing::new(value.to_owned())).unwrap()
}
fn deadline() -> Instant {
    Instant::now() + WORK_DEADLINE
}
async fn request(handle: &Handle, operation: Operation) -> Response {
    let label = match &operation {
        Operation::PreSession(_) => "pre_session",
        Operation::PreCsrf(_) => "pre_csrf",
        Operation::Login { .. } => "login",
        Operation::State(_) => "state",
        Operation::Logout(_) => "logout",
        Operation::Password { .. } => "password_change",
        Operation::RevokeOthers { .. } => "revoke_others",
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
            // Strict read-only bootstrap and a presented unknown pre-session
            // both deny. Neither can fall through into new issuance.
            for operation in [
                Operation::PreCsrf(unknown),
                Operation::PreSession(Some(unknown)),
            ] {
                assert!(matches!(
                    request(&handle, operation).await.result(),
                    Err(Error::UnknownCredential)
                ));
            }

            // Actual completed jobs retain capacity through their buffered replies.
            let mut replies = Vec::new();
            for _ in 0..CAPACITY {
                let response = request(&handle, Operation::State(cli(unknown))).await;
                assert!(matches!(response.result(), Err(Error::UnknownCredential)));
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
                Err(Error::UnknownCredential)
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

            use crate::http_auth::{adapter, Profile};
            use bytes::Bytes;
            use http_body_util::{BodyExt, Full};
            let profile = Profile::new("127.0.0.1:16281".parse().unwrap()).unwrap();
            let login_body = b"{\"password\":\"a fixed test password\"}";
            let wire = hyper::Request::post("/auth/cli/login")
                .header("host", "127.0.0.1:16281")
                .header("content-type", "application/json")
                .header("content-length", login_body.len())
                .body(Full::new(Bytes::from_static(login_body)))
                .unwrap();
            let (response, mut exchange) =
                adapter::handle(wire, &profile, &handle, deadline()).await;
            assert_eq!(response.status(), hyper::StatusCode::OK);
            assert!(!response.headers().contains_key("set-cookie"));
            exchange.check_outbound().unwrap();
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let token = body
                .strip_prefix(b"{\"bearer\":\"")
                .unwrap()
                .strip_suffix(b"\"}")
                .unwrap();
            let cli_bearer = Digest::from_encoded(token).unwrap();
            drop(body);
            // In-memory adapter observation, not a TLS/HTTP exchange.
            let held_capacity = handle.capacity.available_permits();
            exchange.confirm().unwrap();
            assert_eq!(handle.capacity.available_permits(), held_capacity);
            drop(exchange);
            let settled = tokio::time::timeout(
                WORK_DEADLINE,
                handle.capacity.clone().acquire_many_owned(CAPACITY as u32),
            )
            .await
            .unwrap()
            .unwrap();
            drop(settled);
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
            let pre_wire = Zeroizing::new(*pre.encoded());
            let csrf_wire = Zeroizing::new(**csrf);
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
            let cookie = Zeroizing::new(format!(
                "__Host-kehila-pre-session={}",
                std::str::from_utf8(pre_wire.as_slice()).unwrap()
            ));
            let wire = hyper::Request::post("/auth/browser/login")
                .header("host", "127.0.0.1:16281")
                .header("origin", "https://127.0.0.1:16281")
                .header("x-kehila-client", "browser")
                .header("sec-fetch-site", "same-origin")
                .header("x-kehila-csrf", csrf_wire.as_slice())
                .header("cookie", cookie.as_str())
                .header("content-type", "application/json")
                .header("content-length", login_body.len())
                .body(Full::new(Bytes::from_static(login_body)))
                .unwrap();
            let (response, mut retained) =
                adapter::handle(wire, &profile, &handle, deadline()).await;
            assert_eq!(response.status(), hyper::StatusCode::OK);
            let cookie = response.headers()["set-cookie"].to_str().unwrap();
            let token = cookie
                .strip_prefix("__Host-kehila-session=")
                .unwrap()
                .split_once(';')
                .unwrap()
                .0;
            let browser_bearer = Digest::from_encoded(token.as_bytes()).unwrap();
            retained.check_outbound().unwrap();
            // The encoded response is abandoned, retaining its actual exchange
            // guard across a password change to check stale outbound denial.
            drop(response);
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
            assert!(retained.check_outbound().is_err());
            assert!(retained.confirm().is_err());
            drop(retained);
            assert!(matches!(
                request(&handle, Operation::State(cli(cli_bearer)))
                    .await
                    .result(),
                Err(Error::UnknownCredential)
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
                Err(Error::UnknownCredential)
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
                Err(Error::UnknownCredential)
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
