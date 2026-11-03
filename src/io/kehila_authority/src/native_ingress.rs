//! Closed original-byte project ingress, before application or database admission.
use task_contract::project::EstimateUnit;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Route {
    Binding,
    Register,
    List,
    Open,
    Execute,
    Reconcile,
    Cancel,
    Read,
}
impl Route {
    pub(crate) fn from_path(path: &str) -> Option<Self> {
        Some(match path {
            "/projects/binding" => Self::Binding,
            "/projects/register" => Self::Register,
            "/projects/list" => Self::List,
            "/projects/open" => Self::Open,
            "/projects/execute" => Self::Execute,
            "/projects/reconcile" => Self::Reconcile,
            "/projects/cancel" => Self::Cancel,
            "/projects/read" => Self::Read,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Form {
    pub(crate) name: String,
    pub(crate) prefix: String,
    pub(crate) estimate_unit: EstimateUnit,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Input {
    Binding,
    Register(Form),
    List(Option<i64>),
    Open(i64),
    Execute {
        request_id: i64,
        token: String,
        form: Form,
    },
    Reconcile(i64),
    Cancel(i64),
    Read(String),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Invalid;

type Object = serde_json::Map<String, serde_json::Value>;

fn string(object: &mut Object, key: &'static str, limit: usize) -> Result<String, Invalid> {
    match object.remove(key) {
        Some(serde_json::Value::String(value)) if value.len() <= limit && !value.contains('\0') => {
            Ok(value)
        }
        _ => Err(Invalid),
    }
}

fn number(value: &str) -> Result<i64, Invalid> {
    if value.is_empty()
        || value.len() > 19
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Invalid);
    }
    value.parse().map_err(|_| Invalid)
}

fn request_id(object: &mut Object) -> Result<i64, Invalid> {
    number(&string(object, "request_id", 19)?)
}

fn form(object: &mut Object) -> Result<Form, Invalid> {
    let name = string(object, "name", 256)?;
    let prefix = string(object, "prefix", 12)?;
    let estimate_unit = match string(object, "estimate_unit", 6)?.as_str() {
        "hours" => EstimateUnit::Hours,
        "points" => EstimateUnit::Points,
        _ => return Err(Invalid),
    };
    // These are transport/storage shapes. Prospective domain validation belongs
    // to native preparation; historical reconciliation only takes a locator.
    Ok(Form {
        name,
        prefix,
        estimate_unit,
    })
}

pub(crate) fn decode(route: Route, body: &[u8]) -> Result<Input, Invalid> {
    if body.len() > crate::http_auth::BODY_LIMIT {
        return Err(Invalid);
    }
    let serde_json::Value::Object(mut object) =
        project_create_codec::parse_json(body).map_err(|_| Invalid)?
    else {
        return Err(Invalid);
    };
    let input = match route {
        Route::Binding => Input::Binding,
        Route::Register => Input::Register(form(&mut object)?),
        Route::List => Input::List(match object.remove("before") {
            Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(value)) => Some(number(&value)?),
            _ => return Err(Invalid),
        }),
        Route::Open => Input::Open(request_id(&mut object)?),
        Route::Execute => {
            let request_id = request_id(&mut object)?;
            let token = string(&mut object, "token", 32)?;
            if token.len() != 32
                || !token
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(Invalid);
            }
            Input::Execute {
                request_id,
                token,
                form: form(&mut object)?,
            }
        }
        Route::Reconcile => Input::Reconcile(request_id(&mut object)?),
        Route::Cancel => Input::Cancel(request_id(&mut object)?),
        Route::Read => {
            let project_id = string(&mut object, "project_id", 128)?;
            if project_id.is_empty() {
                return Err(Invalid);
            }
            Input::Read(project_id)
        }
    };
    if object.is_empty() {
        Ok(input)
    } else {
        Err(Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_form_is_preserved_and_no_caller_authority_is_accepted() {
        let body = br#"{"name":"  A project  ","prefix":"ROAD","estimate_unit":"points"}"#;
        assert_eq!(
            decode(Route::Register, body).unwrap(),
            Input::Register(Form {
                name: "  A project  ".into(),
                prefix: "ROAD".into(),
                estimate_unit: EstimateUnit::Points,
            })
        );
        for extra in [
            "actor_id",
            "project_id",
            "grant",
            "seed",
            "token",
            "request_id",
        ] {
            let body =
                format!(r#"{{"name":"P","prefix":"PR","estimate_unit":"hours","{extra}":"x"}}"#);
            assert!(decode(Route::Register, body.as_bytes()).is_err());
        }
    }

    #[test]
    fn historical_lookup_has_only_a_canonical_server_locator() {
        assert_eq!(
            decode(Route::Reconcile, br#"{"request_id":"9223372036854775807"}"#).unwrap(),
            Input::Reconcile(i64::MAX)
        );
        for value in [
            "0",
            "-1",
            "+1",
            "01",
            "1.0",
            "1e2",
            "9223372036854775808",
            " 1",
            "",
        ] {
            let body = format!(r#"{{"request_id":"{value}"}}"#);
            assert!(decode(Route::Reconcile, body.as_bytes()).is_err());
        }
        for body in [
            br#"{"request_id":1}"#.as_slice(),
            br#"{"request_id":null}"#,
            br#"{"request_id":"1","name":"old"}"#,
            br#"{}"#,
        ] {
            assert!(decode(Route::Reconcile, body).is_err());
        }
    }

    #[test]
    fn fixed_routes_have_closed_distinct_shapes() {
        assert_eq!(decode(Route::Binding, b"{}").unwrap(), Input::Binding);
        assert_eq!(
            decode(Route::List, br#"{"before":null}"#).unwrap(),
            Input::List(None)
        );
        assert_eq!(
            decode(Route::List, br#"{"before":"3"}"#).unwrap(),
            Input::List(Some(3))
        );
        assert_eq!(
            decode(Route::Open, br#"{"request_id":"2"}"#).unwrap(),
            Input::Open(2)
        );
        assert_eq!(
            decode(Route::Cancel, br#"{"request_id":"2"}"#).unwrap(),
            Input::Cancel(2)
        );
        assert_eq!(
            decode(Route::Read, br#"{"project_id":"target"}"#).unwrap(),
            Input::Read("target".into())
        );
        assert!(decode(Route::List, b"{}").is_err());
        assert!(decode(Route::Binding, br#"{"before":null}"#).is_err());
        assert!(decode(Route::Read, br#"{"project_id":"\u0000"}"#).is_err());
    }

    #[test]
    fn execution_requires_the_disclosed_token_and_exact_form() {
        let body = br#"{"request_id":"1","token":"0123456789abcdef0123456789abcdef","name":"P","prefix":"PR","estimate_unit":"hours"}"#;
        assert!(matches!(
            decode(Route::Execute, body),
            Ok(Input::Execute { request_id: 1, .. })
        ));
        for token in [
            "",
            "abc",
            "0123456789ABCDEF0123456789ABCDEF",
            "g123456789abcdef0123456789abcdef",
        ] {
            let body = format!(
                r#"{{"request_id":"1","token":"{token}","name":"P","prefix":"PR","estimate_unit":"hours"}}"#
            );
            assert!(decode(Route::Execute, body.as_bytes()).is_err());
        }
    }

    #[test]
    fn original_bytes_reject_duplicates_nesting_and_oversized_input() {
        for body in [
            br#"{"request_id":"1","request_id":"2"}"#.as_slice(),
            br#"{"request_id":"1","request_\u0069d":"2"}"#,
            br#"{"request_id":[[[[[[[[[[[[[[[[[["1"]]]]]]]]]]]]]]]]]]}"#,
            br#"{"request_id":"1"} {}"#,
            br#"["1"]"#,
            br#"null"#,
        ] {
            assert!(decode(Route::Open, body).is_err());
        }
        let oversized = vec![b' '; 4097];
        assert!(decode(Route::Binding, &oversized).is_err());
        for body in [
            br#"{"name":null,"prefix":"PR","estimate_unit":"hours"}"#.as_slice(),
            br#"{"name":"P","prefix":"PR","estimate_unit":"days"}"#,
            br#"{"name":"P","prefix":{},"estimate_unit":"hours"}"#,
        ] {
            assert!(decode(Route::Register, body).is_err());
        }
    }
}
