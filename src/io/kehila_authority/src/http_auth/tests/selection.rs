//! Selection retains the common browser boundary, including its read route.
use super::*;

const PATHS: [&str; 3] = ["/selection/read", "/selection/set", "/selection/reconcile"];

fn authenticated(path: &str) -> Request<()> {
    let mut request = browser(Method::POST, path);
    cookie(&mut request, SESSION);
    request
}

#[test]
fn selection_requires_post_browser_session_origin_and_csrf() {
    for path in PATHS {
        let request = authenticated(path);
        let admitted = profile().admit(&request).unwrap_or_else(|status| {
            panic!("selection edge must admit valid browser request: {path} {status}")
        });
        assert!(admitted.bearer.is_some() && admitted.csrf.is_some() && !admitted.pre_session);
        assert_eq!(admitted.length, 2);
        for (name, expected) in [
            ("cookie", StatusCode::UNAUTHORIZED),
            ("origin", StatusCode::FORBIDDEN),
            (CSRF, StatusCode::FORBIDDEN),
            (CLIENT, StatusCode::FORBIDDEN),
            ("sec-fetch-site", StatusCode::FORBIDDEN),
            ("content-type", StatusCode::BAD_REQUEST),
            ("content-length", StatusCode::BAD_REQUEST),
        ] {
            let mut missing = request.clone();
            missing.headers_mut().remove(name);
            assert_eq!(
                profile().admit(&missing).err(),
                Some(expected),
                "{path} missing {name}"
            );
        }
        let mut foreign = request.clone();
        foreign
            .headers_mut()
            .insert(header::ORIGIN, "https://foreign.invalid".parse().unwrap());
        assert_eq!(profile().admit(&foreign).err(), Some(StatusCode::FORBIDDEN));
        let mut pre_session = browser(Method::POST, path);
        cookie(&mut pre_session, PRE_SESSION);
        assert_eq!(
            profile().admit(&pre_session).err(),
            Some(StatusCode::UNAUTHORIZED)
        );
        let mut native = cli(Method::POST, path);
        bearer(&mut native);
        assert!(
            profile().admit(&native).is_err(),
            "selection has no CLI admission"
        );
        let mut mixed = request.clone();
        bearer(&mut mixed);
        assert_eq!(profile().admit(&mixed).err(), Some(StatusCode::BAD_REQUEST));
        let mut get = browser(Method::GET, path);
        cookie(&mut get, SESSION);
        assert_eq!(profile().admit(&get).err(), Some(StatusCode::NOT_FOUND));
    }
}

#[test]
fn selection_declared_body_limit_is_four_kib_and_has_no_duplicate_length() {
    for path in PATHS {
        let mut at_limit = authenticated(path);
        at_limit.headers_mut().insert(
            header::CONTENT_LENGTH,
            selection_codec::MAX_WIRE_BYTES.to_string().parse().unwrap(),
        );
        assert_eq!(profile().admit(&at_limit).unwrap().length, 4096);
        let mut oversized = at_limit.clone();
        oversized
            .headers_mut()
            .insert(header::CONTENT_LENGTH, "4097".parse().unwrap());
        assert_eq!(
            profile().admit(&oversized).err(),
            Some(StatusCode::PAYLOAD_TOO_LARGE)
        );
        let mut duplicate = at_limit.clone();
        duplicate
            .headers_mut()
            .append(header::CONTENT_LENGTH, "4096".parse().unwrap());
        assert_eq!(
            profile().admit(&duplicate).err(),
            Some(StatusCode::BAD_REQUEST)
        );
        at_limit
            .headers_mut()
            .insert(header::CONTENT_LENGTH, "0".parse().unwrap());
        assert_eq!(
            profile().admit(&at_limit).err(),
            Some(StatusCode::BAD_REQUEST)
        );
    }
}

#[test]
fn selection_has_no_path_query_host_or_credential_aliases() {
    assert!(
        profile().admit(&authenticated("/selection/read")).is_ok(),
        "the valid route is the positive control for alias rejection"
    );
    for path in [
        "/selection",
        "/selection/unknown",
        "/selection/read/",
        "/selection/%72ead",
        "/auth/cli/selection",
        "/items/select",
    ] {
        assert_eq!(
            profile().admit(&authenticated(path)).err(),
            Some(StatusCode::NOT_FOUND)
        );
    }
    let query = authenticated("/selection/read?user_id=owner");
    assert_eq!(profile().admit(&query).err(), Some(StatusCode::BAD_REQUEST));
    let mut host = authenticated("/selection/read");
    host.headers_mut()
        .insert(header::HOST, "localhost:16281".parse().unwrap());
    assert_eq!(profile().admit(&host).err(), Some(StatusCode::BAD_REQUEST));
    let mut cookies = authenticated("/selection/read");
    cookie(&mut cookies, PRE_SESSION);
    assert_eq!(
        profile().admit(&cookies).err(),
        Some(StatusCode::BAD_REQUEST)
    );
}
