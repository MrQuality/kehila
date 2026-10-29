use super::*;

const TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
fn profile() -> Profile {
    Profile::new("127.0.0.1:16281".parse().unwrap()).unwrap()
}
fn browser(method: Method, path: &str) -> Request<()> {
    let write = method == Method::POST;
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:16281")
        .header(CLIENT, "browser")
        .header("sec-fetch-site", "same-origin");
    if write {
        request = request
            .header("origin", "https://127.0.0.1:16281")
            .header("content-type", "application/json")
            .header("content-length", "2")
            .header(CSRF, TOKEN);
    }
    request.body(()).unwrap()
}
fn cli(method: Method, path: &str) -> Request<()> {
    let write = method == Method::POST;
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:16281");
    if write {
        request = request
            .header("content-type", "application/json")
            .header("content-length", "2");
    }
    request.body(()).unwrap()
}
fn cookie(request: &mut Request<()>, name: &str) {
    request
        .headers_mut()
        .append(header::COOKIE, format!("{name}={TOKEN}").parse().unwrap());
}
fn bearer(request: &mut Request<()>) {
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {TOKEN}").parse().unwrap(),
    );
}

#[test]
fn explicit_browser_and_cli_routes_admit_only_their_credentials() {
    let mut login = browser(Method::POST, "/auth/browser/login");
    cookie(&mut login, PRE_SESSION);
    let admitted = profile().admit(&login).unwrap();
    assert!(matches!(admitted.route, Route::Login(Kind::Browser)) && admitted.pre_session);
    for path in ["/auth/browser/state", "/auth/browser/csrf"] {
        let mut state = browser(Method::GET, path);
        cookie(&mut state, SESSION);
        assert!(profile().admit(&state).is_ok()); // No old CSRF needed.
    }
    assert!(profile()
        .admit(&browser(Method::GET, "/auth/browser/pre-session"))
        .is_ok());
    assert!(profile()
        .admit(&cli(Method::POST, "/auth/cli/login"))
        .is_ok());
    let mut state = cli(Method::GET, "/auth/cli/state");
    bearer(&mut state);
    assert!(profile().admit(&state).is_ok());
    bearer(&mut login);
    assert!(profile().admit(&login).is_err());
    for name in ["origin", "cookie", "sec-fetch-mode", CLIENT, CSRF] {
        let mut request = state.clone();
        request.headers_mut().insert(
            header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            "x".parse().unwrap(),
        );
        assert!(profile().admit(&request).is_err(), "{name}");
    }
}

#[test]
fn browser_rejects_missing_foreign_duplicate_or_mixed_proofs() {
    let mut login = browser(Method::POST, "/auth/browser/login");
    cookie(&mut login, PRE_SESSION);
    for name in ["origin", CLIENT, "sec-fetch-site", CSRF] {
        let mut request = login.clone();
        request.headers_mut().remove(name);
        assert!(profile().admit(&request).is_err(), "missing {name}");
        request.headers_mut().insert(
            header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            "null".parse().unwrap(),
        );
        assert!(profile().admit(&request).is_err(), "wrong {name}");
        let mut duplicate = login.clone();
        duplicate.headers_mut().append(
            header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            login.headers()[name].clone(),
        );
        assert!(profile().admit(&duplicate).is_err(), "duplicate {name}");
    }
    let mut duplicate = login.clone();
    cookie(&mut duplicate, PRE_SESSION);
    assert!(profile().admit(&duplicate).is_err());
    login.headers_mut().insert(
        header::COOKIE,
        format!("{PRE_SESSION}={TOKEN}; {PRE_SESSION}={TOKEN}")
            .parse()
            .unwrap(),
    );
    assert!(profile().admit(&login).is_err());
    let mut mixed = browser(Method::GET, "/auth/browser/csrf");
    cookie(&mut mixed, PRE_SESSION);
    cookie(&mut mixed, SESSION);
    assert!(profile().admit(&mixed).is_err());
    assert!(profile()
        .admit(&browser(Method::GET, "/auth/browser/csrf"))
        .is_err());
    let mut state = cli(Method::GET, "/auth/cli/state");
    bearer(&mut state);
    bearer(&mut state);
    state.headers_mut().append(
        header::AUTHORIZATION,
        format!("Bearer {TOKEN}").parse().unwrap(),
    );
    assert!(profile().admit(&state).is_err());
}

#[test]
fn rejects_ambiguous_framing_proxies_and_admission_overruns() {
    let login = cli(Method::POST, "/auth/cli/login");
    for (name, value) in [
        ("host", "localhost:16281"),
        ("transfer-encoding", "chunked"),
        ("content-encoding", "identity"),
        ("x-forwarded-port", "16281"),
        ("expect", "100-continue"),
        ("content-length", "02"),
        ("content-length", "4097"),
        ("content-type", "text/plain"),
    ] {
        let mut request = login.clone();
        request.headers_mut().insert(
            header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
        assert!(profile().admit(&request).is_err(), "{name}");
    }
    for uri in [
        "https://127.0.0.1:16281/auth/cli/login",
        "/auth/cli/login?x=1",
        "/auth/cli/%6cogin",
    ] {
        let mut request = login.clone();
        *request.uri_mut() = uri.parse().unwrap();
        assert!(profile().admit(&request).is_err());
    }
    let mut duplicate = login.clone();
    duplicate
        .headers_mut()
        .append(header::CONTENT_LENGTH, "2".parse().unwrap());
    assert!(profile().admit(&duplicate).is_err());
    let mut large = login.clone();
    large
        .headers_mut()
        .insert("x-extra", "x".repeat(8192).parse().unwrap());
    assert!(profile().admit(&large).is_err());
    let mut old = login;
    *old.version_mut() = Version::HTTP_10;
    assert!(profile().admit(&old).is_err());
}

#[test]
fn json_requires_exact_object_shape_without_duplicate_keys_or_type_coercion() {
    let route = Route::Login(Kind::Cli);
    for body in [
        br#"{"password":"a fixed test password"}"#.as_slice(),
        br#"{"pass\u0077ord":"a fixed test password"}"#,
    ] {
        assert!(matches!(
            json::decode(route, body),
            Ok(json::Input::Login(_))
        ));
    }
    for body in [
        br#"["a fixed test password"]"#.as_slice(),
        br#"{}"#,
        br#"{"password":null}"#,
        br#"{"password":{"password":"a fixed test password"}}"#,
        br#"{"password":"a fixed test password","extra":0}"#,
        br#"{"password":"a fixed test password","pass\u0077ord":"a fixed test password"}"#,
        br#"{"password":"a fixed test password"}{}"#,
        br#"{"password":"short"}"#,
    ] {
        assert!(json::decode(route, body).is_err());
    }
    let too_large = format!("{{\"password\":\"{}\"}}", "x".repeat(1025));
    assert!(json::decode(route, too_large.as_bytes()).is_err());
    assert!(json::decode(route, &vec![b' '; 4097]).is_err());
    assert!(json::decode(Route::Logout(Kind::Cli), b"[]").is_err());
    assert!(json::decode(Route::Logout(Kind::Cli), b"{}").is_ok());
    assert!(matches!(
        json::decode(
            Route::Password(Kind::Cli),
            br#"{"current":"a fixed test password","replacement":"another fixed password"}"#
        ),
        Ok(json::Input::Change { .. })
    ));
}
