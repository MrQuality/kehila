//! Prepared dispatcher, deliberately unconnected to a serving listener.
use super::{json, Admitted, Profile, Route, BODY_LIMIT, PRE_SESSION, SESSION};
use crate::{
    application::{self, jobs, Credential, LoginKind, Operation, Output},
    session::Kind,
};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    body::Body,
    header::{self, HeaderValue},
    Request, Response, StatusCode,
};
use std::time::{Duration, Instant};
use tokio::time::{timeout_at, Instant as TokioInstant};
use zeroize::Zeroizing;

/// The transport must retain this through its observed exchange and cleanup.
/// Failure/cancellation drops the retained reply and retires unconfirmed issuance.
pub(crate) struct Exchange(Option<jobs::Response>);
impl Exchange {
    /// Required immediately before the listener starts outbound delivery. A
    /// future streaming writer must preserve this gate through its own writes.
    pub(crate) fn check_outbound(&self) -> Result<(), application::Error> {
        match self.0.as_ref().map(jobs::Response::result) {
            Some(Ok(Output::Issued(delivery))) => delivery.material().map(|_| ()),
            None
            | Some(Ok(_))
            | Some(Err(
                application::Error::AuditUnconfirmed | application::Error::OtherAuditUnconfirmed,
            )) => Ok(()),
            Some(Err(error)) => Err(error),
        }
    }
    pub(crate) fn confirm(mut self) -> Result<(), application::Error> {
        if let Some(reply) = self.0.as_mut() {
            reply.confirm_exchange()?;
        }
        Ok(())
    }
}

struct WipedBytes(Zeroizing<Vec<u8>>);
impl AsRef<[u8]> for WipedBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

fn response(status: StatusCode, body: Zeroizing<Vec<u8>>) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from_owner(WipedBytes(body))));
    *response.status_mut() = status;
    for (name, value) in [
        (header::CONNECTION, "close"),
        (header::CACHE_CONTROL, "no-store"),
        (header::CONTENT_TYPE, "application/json"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; frame-ancestors 'none'; object-src 'none'; form-action 'self'",
        ),
        (header::REFERRER_POLICY, "no-referrer"),
    ] {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    response.headers_mut().insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    response
}

fn clear_cookies(response: &mut Response<Full<Bytes>>) {
    for value in [
        "__Host-kehila-session=; Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        "__Host-kehila-pre-session=; Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
    ] {
        response
            .headers_mut()
            .append(header::SET_COOKIE, HeaderValue::from_static(value));
    }
}

fn failure(status: StatusCode, clear_stale_cookie: bool) -> (Response<Full<Bytes>>, Exchange) {
    let mut response = response(
        status,
        Zeroizing::new(b"{\"error\":\"request rejected\"}".to_vec()),
    );
    // Explicit denial/clearing allows a subsequent fresh enrollment of browser
    // state after restart. This request never falls through to anonymous work.
    if clear_stale_cookie {
        clear_cookies(&mut response);
    }
    (response, Exchange(None))
}

impl Admitted {
    fn operation(&self, body: &[u8]) -> Result<Operation, StatusCode> {
        if body.len() != self.length {
            return Err(StatusCode::BAD_REQUEST);
        }
        let input = json::decode(self.route, body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let credential = |kind| -> Result<Credential, StatusCode> {
            Ok(Credential {
                bearer: self.bearer.ok_or(StatusCode::UNAUTHORIZED)?,
                kind,
                csrf: self.csrf,
            })
        };
        match (self.route, input) {
            (Route::PreSession, json::Input::Empty) => Ok(Operation::PreSession(self.bearer)),
            (Route::Csrf, json::Input::Empty) if self.pre_session => Ok(Operation::PreCsrf(
                self.bearer.ok_or(StatusCode::UNAUTHORIZED)?,
            )),
            (Route::Csrf, json::Input::Empty) => Ok(Operation::State(credential(Kind::Browser)?)),
            (Route::Login(kind), json::Input::Login(password)) => Ok(Operation::Login {
                password,
                kind: match kind {
                    Kind::Cli => LoginKind::Cli,
                    Kind::Browser => LoginKind::Browser {
                        bearer: self.bearer.ok_or(StatusCode::UNAUTHORIZED)?,
                        csrf: self.csrf.ok_or(StatusCode::FORBIDDEN)?,
                    },
                },
            }),
            (Route::State(kind), json::Input::Empty) => Ok(Operation::State(credential(kind)?)),
            (Route::RevokeOthers(kind), json::Input::Login(password)) => {
                Ok(Operation::RevokeOthers {
                    credential: credential(kind)?,
                    password,
                })
            }
            (Route::Logout(kind), json::Input::Empty) => Ok(Operation::Logout(credential(kind)?)),
            (
                Route::Password(kind),
                json::Input::Change {
                    current,
                    replacement,
                },
            ) => Ok(Operation::Password {
                credential: credential(kind)?,
                current,
                replacement,
            }),
            _ => Err(StatusCode::BAD_REQUEST),
        }
    }
}

fn app_status(error: application::Error) -> StatusCode {
    match error {
        application::Error::Invalid => StatusCode::BAD_REQUEST,
        application::Error::Unauthorized | application::Error::UnknownCredential => {
            StatusCode::UNAUTHORIZED
        }
        application::Error::Busy => StatusCode::TOO_MANY_REQUESTS,
        application::Error::Unavailable
        | application::Error::AuditUnconfirmed
        | application::Error::OtherAuditUnconfirmed => StatusCode::SERVICE_UNAVAILABLE,
    }
}

fn application_failure(
    error: application::Error,
    browser: bool,
) -> (Response<Full<Bytes>>, Exchange) {
    failure(
        app_status(error),
        browser && error == application::Error::UnknownCredential,
    )
}

async fn read<B: Body<Data = Bytes> + Unpin>(
    mut body: B,
    length: usize,
    deadline: Instant,
) -> Result<Zeroizing<Vec<u8>>, StatusCode> {
    let read_deadline = deadline.min(Instant::now() + Duration::from_secs(2));
    let mut bytes = Zeroizing::new(Vec::with_capacity(BODY_LIMIT));
    timeout_at(TokioInstant::from_std(read_deadline), async {
        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(|_| StatusCode::BAD_REQUEST)?;
            let data = frame.into_data().map_err(|_| StatusCode::BAD_REQUEST)?;
            if data.len() > length.saturating_sub(bytes.len()) {
                return Err(StatusCode::BAD_REQUEST);
            }
            bytes.extend_from_slice(&data);
        }
        if bytes.len() != length {
            return Err(StatusCode::BAD_REQUEST);
        }
        Ok(bytes)
    })
    .await
    .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
}

fn secret_field(body: &mut Vec<u8>, field: &[u8], value: &[u8; 43]) {
    body.extend_from_slice(b"{\"");
    body.extend_from_slice(field);
    body.extend_from_slice(b"\":\"");
    body.extend_from_slice(value);
    body.extend_from_slice(b"\"}");
}

fn cookie(
    response: &mut Response<Full<Bytes>>,
    name: &str,
    value: &[u8; 43],
) -> Result<(), application::Error> {
    let mut encoded = Zeroizing::new(Vec::with_capacity(160));
    encoded.extend_from_slice(name.as_bytes());
    encoded.push(b'=');
    encoded.extend_from_slice(value);
    encoded.extend_from_slice(b"; Secure; HttpOnly; SameSite=Strict; Path=/");
    if name == PRE_SESSION {
        encoded.extend_from_slice(b"; Max-Age=300");
    }
    let mut value =
        HeaderValue::from_bytes(&encoded).map_err(|_| application::Error::Unavailable)?;
    value.set_sensitive(true);
    response.headers_mut().append(header::SET_COOKIE, value);
    Ok(())
}

fn encode(
    route: Route,
    reply: &jobs::Response,
) -> Result<Response<Full<Bytes>>, application::Error> {
    let mut body = Zeroizing::new(Vec::with_capacity(256));
    let mut issued_cookie = None;
    let mut clear = false;
    match (route, reply.result()) {
        (Route::Logout(_), Err(application::Error::AuditUnconfirmed)) => {
            return Ok(response(
                StatusCode::SERVICE_UNAVAILABLE,
                Zeroizing::new(b"{\"signed_out\":true,\"audit_confirmed\":false}".to_vec()),
            ));
        }
        (Route::RevokeOthers(_), Err(application::Error::OtherAuditUnconfirmed)) => {
            return Ok(response(
                StatusCode::SERVICE_UNAVAILABLE,
                Zeroizing::new(
                    b"{\"other_sessions_revoked\":true,\"audit_confirmed\":false}".to_vec(),
                ),
            ))
        }
        (_, Err(error)) => return Err(error),
        (Route::PreSession | Route::Csrf, Ok(Output::PreSession { bearer, csrf })) => {
            secret_field(&mut body, b"csrf", csrf);
            if let Some(bearer) = bearer {
                issued_cookie = Some((PRE_SESSION, bearer.encoded()));
            }
        }
        (
            Route::Csrf,
            Ok(Output::State {
                csrf: Some(csrf), ..
            }),
        ) => secret_field(&mut body, b"csrf", csrf),
        (Route::Login(kind), Ok(Output::Issued(delivery))) => {
            let (bearer, csrf) = delivery.material()?;
            match kind {
                Kind::Browser => {
                    secret_field(
                        &mut body,
                        b"csrf",
                        csrf.ok_or(application::Error::Unavailable)?,
                    );
                    issued_cookie = Some((SESSION, bearer));
                }
                Kind::Cli => secret_field(&mut body, b"bearer", bearer),
            }
        }
        (Route::State(_), Ok(Output::State { active, .. })) => body.extend_from_slice(if *active {
            b"{\"active\":true}"
        } else {
            b"{\"active\":false}"
        }),
        (Route::Logout(_), Ok(Output::LoggedOut)) => {
            body.extend_from_slice(b"{\"signed_out\":true,\"audit_confirmed\":true}");
            clear = true;
        }
        (Route::Password(_), Ok(Output::PasswordChanged)) => {
            body.extend_from_slice(b"{\"password_changed\":true}");
            clear = true;
        }
        (Route::RevokeOthers(_), Ok(Output::OthersRevoked { count })) => body.extend_from_slice(
            format!(
                "{{\"other_sessions_revoked\":true,\"audit_confirmed\":true,\"count\":{count}}}"
            )
            .as_bytes(),
        ),
        _ => return Err(application::Error::Unavailable),
    }
    let mut response = response(StatusCode::OK, body);
    if let Some((name, value)) = issued_cookie {
        cookie(&mut response, name, value)?;
        if name == SESSION {
            response.headers_mut().append(header::SET_COOKIE, HeaderValue::from_static("__Host-kehila-pre-session=; Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"));
        }
    }
    if clear
        && matches!(
            route,
            Route::Logout(Kind::Browser) | Route::Password(Kind::Browser)
        )
    {
        clear_cookies(&mut response);
    }
    Ok(response)
}

/// Caller supplies the original absolute deadline established at connection
/// admission. Neither body parsing nor coordinator submission restarts it.
pub(crate) async fn handle<B: Body<Data = Bytes> + Unpin>(
    request: Request<B>,
    profile: &Profile,
    jobs: &jobs::Handle,
    deadline: Instant,
) -> (Response<Full<Bytes>>, Exchange) {
    let browser = request.uri().path().starts_with("/auth/browser/");
    if Instant::now() >= deadline {
        return failure(StatusCode::REQUEST_TIMEOUT, false);
    }
    let admitted = match profile.admit(&request) {
        Ok(admitted) => admitted,
        Err(status) => return failure(status, false),
    };
    let body = match read(request.into_body(), admitted.length, deadline).await {
        Ok(body) => body,
        Err(status) => return failure(status, false),
    };
    let operation = match admitted.operation(&body) {
        Ok(operation) => operation,
        Err(status) => return failure(status, false),
    };
    drop(body);
    let receiver = match jobs.submit(operation, deadline) {
        Ok(receiver) => receiver,
        Err(error) => return application_failure(error, browser),
    };
    // Timeout drops only the reply receiver. The owned manager retains the real
    // database/hash future and its permit until completion and join.
    let reply = match timeout_at(TokioInstant::from_std(deadline), receiver).await {
        Ok(Ok(reply)) => reply,
        _ => return failure(StatusCode::SERVICE_UNAVAILABLE, false),
    };
    match encode(admitted.route, &reply) {
        Ok(response) => (response, Exchange(Some(reply))),
        Err(error) => application_failure(error, browser),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stalled;
    impl Body for Stalled {
        type Data = Bytes;
        type Error = std::convert::Infallible;
        fn poll_frame(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<hyper::body::Frame<Bytes>, Self::Error>>> {
            std::task::Poll::Pending
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn admitted_body_length_and_absolute_deadline_are_enforced() {
        let now = Instant::now();
        assert_eq!(
            read(
                Full::new(Bytes::from_static(b"{}")),
                1,
                now + Duration::from_secs(1)
            )
            .await
            .err(),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            read(
                Full::new(Bytes::from_static(b"{}")),
                3,
                now + Duration::from_secs(1)
            )
            .await
            .err(),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            read(
                Full::new(Bytes::from_static(b"{}")),
                2,
                now + Duration::from_secs(1)
            )
            .await
            .unwrap()
            .as_slice(),
            b"{}"
        );
        assert_eq!(
            read(Stalled, 2, Instant::now() + Duration::from_millis(1))
                .await
                .err(),
            Some(StatusCode::REQUEST_TIMEOUT)
        );
    }

    #[test]
    fn response_security_and_cookie_policy_are_explicit() {
        let mut result = response(StatusCode::OK, Zeroizing::new(b"{}".to_vec()));
        cookie(&mut result, SESSION, &[b'A'; 43]).unwrap();
        assert_eq!(result.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            result.headers()["cross-origin-resource-policy"],
            "same-origin"
        );
        assert_eq!(result.headers()[header::SET_COOKIE], "__Host-kehila-session=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA; Secure; HttpOnly; SameSite=Strict; Path=/");
        assert!(result.headers()[header::SET_COOKIE].is_sensitive());
        assert!(!result
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
        let (preserved, _) = application_failure(application::Error::Unauthorized, true);
        assert!(!preserved.headers().contains_key(header::SET_COOKIE));
        let (cleared, _) = application_failure(application::Error::UnknownCredential, true);
        assert_eq!(
            cleared.headers().get_all(header::SET_COOKIE).iter().count(),
            2
        );
    }
}
