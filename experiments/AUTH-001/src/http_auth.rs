//! Closed authentication request admission, before application scheduling.
#[cfg(target_os = "linux")]
pub(crate) mod adapter;
mod json;
#[cfg(test)]
mod tests;

use crate::{secret::Digest, session::Kind};
use hyper::{header, Method, Request, StatusCode, Version};
use std::net::{Ipv4Addr, SocketAddr};

pub(crate) const BODY_LIMIT: usize = 4096;
const SESSION: &str = "__Host-kehila-session";
const PRE_SESSION: &str = "__Host-kehila-pre-session";
const CSRF: &str = "x-kehila-csrf";
const CLIENT: &str = "x-kehila-client";

#[derive(Clone, Copy)]
pub(crate) enum Route {
    PreSession,
    Csrf,
    Login(Kind),
    State(Kind),
    Logout(Kind),
    Password(Kind),
    RevokeOthers(Kind),
}

pub(crate) struct Admitted {
    pub(crate) route: Route,
    pub(crate) bearer: Option<Digest>,
    pub(crate) csrf: Option<Digest>,
    pub(crate) pre_session: bool,
    pub(crate) length: usize,
}

/// The authority is the configured Windows-facing literal loopback address,
/// not the Linux listener's address or an untrusted forwarded assertion.
pub(crate) struct Profile {
    host: String,
    origin: String,
}

fn rejected() -> StatusCode {
    StatusCode::BAD_REQUEST
}

fn one<'a, B>(request: &'a Request<B>, name: &str) -> Result<Option<&'a str>, StatusCode> {
    let mut values = request.headers().get_all(name).iter();
    let value = values
        .next()
        .map(|value| value.to_str().map_err(|_| rejected()))
        .transpose()?;
    if values.next().is_some() {
        return Err(rejected());
    }
    Ok(value)
}

fn digest(value: &str) -> Result<Digest, StatusCode> {
    Digest::from_encoded(value.as_bytes()).map_err(|_| StatusCode::UNAUTHORIZED)
}

struct Cookies {
    session: Option<Digest>,
    pre: Option<Digest>,
}

fn cookies<B>(request: &Request<B>) -> Result<Cookies, StatusCode> {
    let mut result = Cookies {
        session: None,
        pre: None,
    };
    // HTTP/1 Cookie fields can be repeated. Inspect every pair across every
    // field; never select a first/last authentication cookie silently.
    for field in request.headers().get_all(header::COOKIE) {
        for pair in field.to_str().map_err(|_| rejected())?.split(';') {
            let (name, value) = pair.trim().split_once('=').ok_or_else(rejected)?;
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            {
                return Err(rejected());
            }
            let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                &value[1..value.len() - 1]
            } else {
                value
            };
            if !value
                .bytes()
                .all(|b| matches!(b, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e))
            {
                return Err(rejected());
            }
            let slot = match name {
                SESSION => &mut result.session,
                PRE_SESSION => &mut result.pre,
                _ => continue,
            };
            if slot.is_some() {
                return Err(rejected());
            }
            *slot = Some(digest(value)?);
        }
    }
    if result.session.is_some() && result.pre.is_some() {
        return Err(rejected());
    }
    Ok(result)
}

impl Profile {
    pub(crate) fn new(authority: SocketAddr) -> Result<Self, StatusCode> {
        if authority.ip() != Ipv4Addr::LOCALHOST || authority.port() == 0 {
            return Err(rejected());
        }
        Ok(Self {
            host: authority.to_string(),
            origin: format!("https://{authority}"),
        })
    }

    pub(crate) fn admit<B>(&self, request: &Request<B>) -> Result<Admitted, StatusCode> {
        let header_bytes: usize = request
            .headers()
            .iter()
            .map(|(name, value)| name.as_str().len() + value.len() + 4)
            .sum();
        if request.version() != Version::HTTP_11
            || request.headers().len() > 32
            || header_bytes > 8192
            || request.uri().to_string().len() > 2048
            || request.uri().authority().is_some()
            || request.uri().scheme().is_some()
            || request.uri().query().is_some()
            || one(request, "host")? != Some(self.host.as_str())
            || request.headers().keys().any(|name| {
                name == "forwarded"
                    || name.as_str().starts_with("x-forwarded-")
                    || matches!(
                        name.as_str(),
                        "transfer-encoding" | "content-encoding" | "expect" | "trailer" | "upgrade"
                    )
            })
        {
            return Err(rejected());
        }
        let (route, kind, write) = match (request.method(), request.uri().path()) {
            (&Method::GET, "/auth/browser/pre-session") => {
                (Route::PreSession, Kind::Browser, false)
            }
            (&Method::GET, "/auth/browser/csrf") => (Route::Csrf, Kind::Browser, false),
            (&Method::POST, "/auth/browser/login") => {
                (Route::Login(Kind::Browser), Kind::Browser, true)
            }
            (&Method::GET, "/auth/browser/state") => {
                (Route::State(Kind::Browser), Kind::Browser, false)
            }
            (&Method::POST, "/auth/browser/logout") => {
                (Route::Logout(Kind::Browser), Kind::Browser, true)
            }
            (&Method::POST, "/auth/browser/password") => {
                (Route::Password(Kind::Browser), Kind::Browser, true)
            }
            (&Method::POST, "/auth/browser/revoke-others") => {
                (Route::RevokeOthers(Kind::Browser), Kind::Browser, true)
            }
            (&Method::POST, "/auth/cli/login") => (Route::Login(Kind::Cli), Kind::Cli, true),
            (&Method::GET, "/auth/cli/state") => (Route::State(Kind::Cli), Kind::Cli, false),
            (&Method::POST, "/auth/cli/logout") => (Route::Logout(Kind::Cli), Kind::Cli, true),
            (&Method::POST, "/auth/cli/password") => (Route::Password(Kind::Cli), Kind::Cli, true),
            (&Method::POST, "/auth/cli/revoke-others") => {
                (Route::RevokeOthers(Kind::Cli), Kind::Cli, true)
            }
            _ => return Err(StatusCode::NOT_FOUND),
        };
        let length = match one(request, "content-length")? {
            Some(value)
                if value == "0"
                    || (!value.starts_with('0') && value.bytes().all(|b| b.is_ascii_digit())) =>
            {
                value.parse::<usize>().map_err(|_| rejected())?
            }
            None if !write => 0,
            _ => return Err(rejected()),
        };
        if length > BODY_LIMIT {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        if write {
            if length == 0 || one(request, "content-type")? != Some("application/json") {
                return Err(rejected());
            }
        } else if length != 0 || request.headers().contains_key(header::CONTENT_TYPE) {
            return Err(rejected());
        }
        let mut admitted = Admitted {
            route,
            bearer: None,
            csrf: None,
            pre_session: false,
            length,
        };
        match kind {
            Kind::Browser => {
                if request.headers().contains_key(header::AUTHORIZATION) {
                    return Err(rejected());
                }
                let origin = one(request, "origin")?;
                if origin.is_some_and(|value| value != self.origin) || (write && origin.is_none()) {
                    return Err(StatusCode::FORBIDDEN);
                }
                if one(request, CLIENT)? != Some("browser")
                    || one(request, "sec-fetch-site")? != Some("same-origin")
                {
                    return Err(StatusCode::FORBIDDEN);
                }
                let cookies = cookies(request)?;
                admitted.pre_session = cookies.pre.is_some();
                admitted.bearer = cookies.session.or(cookies.pre);
                match route {
                    Route::PreSession if cookies.session.is_some() => {
                        return Err(StatusCode::UNAUTHORIZED)
                    }
                    Route::PreSession => {}
                    Route::Login(_) if cookies.pre.is_none() => {
                        return Err(StatusCode::UNAUTHORIZED)
                    }
                    Route::Login(_) => {}
                    Route::Csrf if admitted.bearer.is_some() => {}
                    Route::State(_)
                    | Route::Logout(_)
                    | Route::Password(_)
                    | Route::RevokeOthers(_)
                        if cookies.session.is_some() => {}
                    _ => return Err(StatusCode::UNAUTHORIZED),
                }
                let csrf = one(request, CSRF)?;
                if write {
                    admitted.csrf = Some(digest(csrf.ok_or(StatusCode::FORBIDDEN)?)?);
                } else if csrf.is_some() {
                    return Err(rejected());
                }
            }
            Kind::Cli => {
                if request.headers().keys().any(|name| {
                    matches!(name.as_str(), "cookie" | "origin" | CLIENT | CSRF)
                        || name.as_str().starts_with("sec-fetch-")
                }) {
                    return Err(rejected());
                }
                let authorization = one(request, "authorization")?;
                if matches!(route, Route::Login(_)) {
                    if authorization.is_some() {
                        return Err(rejected());
                    }
                } else {
                    let token = authorization
                        .and_then(|value| value.strip_prefix("Bearer "))
                        .ok_or(StatusCode::UNAUTHORIZED)?;
                    admitted.bearer = Some(digest(token)?);
                }
            }
        }
        Ok(admitted)
    }
}
