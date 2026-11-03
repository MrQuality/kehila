use bytes::Bytes;
use http_body_util::Full;
use hyper::{header, Method, Request, Response, StatusCode, Version};

pub(super) fn response<B>(request: &Request<B>, authority: &str) -> Option<Response<Full<Bytes>>> {
    let (content, content_type): (&'static [u8], &'static str) = match request.uri().path() {
        "/" => (include_bytes!("page.html"), "text/html; charset=utf-8"),
        "/owner.js" => (include_bytes!("page.js"), "text/javascript; charset=utf-8"),
        _ => return None,
    };
    let exact = |name: &str, expected: &str, optional: bool| {
        let mut values = request.headers().get_all(name).iter();
        match values.next() {
            None => optional,
            Some(value) => value == expected && values.next().is_none(),
        }
    };
    let valid = request.method() == Method::GET
        && request.version() == Version::HTTP_11
        && request.uri().authority().is_none()
        && request.uri().scheme().is_none()
        && request.uri().query().is_none()
        && exact("host", authority, false)
        && exact("content-length", "0", true)
        && exact("origin", &format!("https://{authority}"), true)
        && !request.headers().keys().any(|name| {
            matches!(
                name.as_str(),
                "authorization"
                    | "transfer-encoding"
                    | "content-type"
                    | "forwarded"
                    | "x-forwarded-for"
                    | "x-forwarded-proto"
                    | "x-forwarded-host"
            )
        });
    let mut result = Response::new(Full::new(if valid {
        Bytes::from_static(content)
    } else {
        Bytes::new()
    }));
    if !valid {
        *result.status_mut() = StatusCode::BAD_REQUEST;
    }
    for (name, value) in [
        (header::CONTENT_TYPE, content_type),
        (header::CONNECTION, "close"),
        (header::CACHE_CONTROL, "no-store"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::REFERRER_POLICY, "no-referrer"),
        (header::CONTENT_SECURITY_POLICY, "default-src 'none'; script-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; object-src 'none'; form-action 'none'"),
    ] { result.headers_mut().insert(name, header::HeaderValue::from_static(value)); }
    result.headers_mut().insert(
        "cross-origin-resource-policy",
        header::HeaderValue::from_static("same-origin"),
    );
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_page_requires_exact_origin_and_bodyless_get() {
        let request = Request::get("/")
            .header("host", "127.0.0.1:16291")
            .body(())
            .unwrap();
        assert_eq!(
            response(&request, "127.0.0.1:16291").unwrap().status(),
            StatusCode::OK
        );
        for (name, value) in [
            ("host", "foreign.invalid"),
            ("origin", "null"),
            ("content-length", "1"),
            ("forwarded", "for=127.0.0.1"),
        ] {
            let mut invalid = request.clone();
            invalid.headers_mut().insert(
                header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
            assert_eq!(
                response(&invalid, "127.0.0.1:16291").unwrap().status(),
                StatusCode::BAD_REQUEST
            );
        }
    }
}
