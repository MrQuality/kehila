//! Closed original-byte admission for authorized WorkItem reads.
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Route {
    Read,
    List,
}
impl Route {
    pub(crate) fn from_path(path: &str) -> Option<Self> {
        match path {
            "/items/read" => Some(Self::Read),
            "/items/list" => Some(Self::List),
            _ => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Input {
    Read {
        project: String,
        item: String,
    },
    List {
        project: String,
        before: Option<u64>,
    },
}
impl Input {
    pub(crate) fn project(&self) -> &str {
        match self {
            Self::Read { project, .. } | Self::List { project, .. } => project,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Invalid;

fn identifier(object: &mut Map<String, Value>, key: &str) -> Result<String, Invalid> {
    match object.remove(key) {
        Some(Value::String(value))
            if !value.is_empty() && value.len() <= 128 && !value.contains('\0') =>
        {
            Ok(value)
        }
        _ => Err(Invalid),
    }
}

pub(crate) fn decode(route: Route, bytes: &[u8]) -> Result<Input, Invalid> {
    if bytes.len() > crate::http_auth::BODY_LIMIT {
        return Err(Invalid);
    }
    let Value::Object(mut object) = project_create_codec::parse_json(bytes).map_err(|_| Invalid)?
    else {
        return Err(Invalid);
    };
    let project = identifier(&mut object, "project_id")?;
    let input = match route {
        Route::Read => Input::Read {
            project,
            item: identifier(&mut object, "item_id")?,
        },
        Route::List => {
            let before = match object.remove("before") {
                Some(Value::Null) => None,
                Some(Value::String(text)) => {
                    let number: u64 = text.parse().map_err(|_| Invalid)?;
                    if number == 0 || number.to_string() != text {
                        return Err(Invalid);
                    }
                    Some(number)
                }
                _ => return Err(Invalid),
            };
            Input::List { project, before }
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
    fn scoped_reads_accept_no_caller_authority_or_extra_fields() {
        let bytes = br#"{"project_id":"p","item_id":"i"}"#;
        assert_eq!(
            decode(Route::Read, bytes).unwrap(),
            Input::Read {
                project: "p".into(),
                item: "i".into(),
            }
        );
        for bytes in [
            br#"{"project_id":"p","item_id":"i","actor_id":"owner"}"#.as_slice(),
            br#"{"project_id":"p","project_id":"q","item_id":"i"}"#,
            br#"{"project_id":"p","item_id":""}"#,
            br#"{"project_id":"p","item_id":"i\u0000"}"#,
            br#"{"project_id":"p","item_id":1}"#,
        ] {
            assert!(decode(Route::Read, bytes).is_err());
        }
        assert!(decode(Route::Read, &vec![b' '; crate::http_auth::BODY_LIMIT + 1]).is_err());
        assert!(Route::from_path("/items/edit").is_none());
    }

    #[test]
    fn cursor_is_an_exact_positive_u64_string_and_is_always_explicit() {
        for before in ["null", "\"1\"", "\"18446744073709551615\""] {
            let bytes = format!(r#"{{"project_id":"p","before":{before}}}"#);
            assert!(decode(Route::List, bytes.as_bytes()).is_ok());
        }
        for before in [
            "0",
            "1",
            "\"0\"",
            "\"01\"",
            "\"+1\"",
            "\"18446744073709551616\"",
        ] {
            let bytes = format!(r#"{{"project_id":"p","before":{before}}}"#);
            assert!(decode(Route::List, bytes.as_bytes()).is_err());
        }
        assert!(decode(Route::List, br#"{"project_id":"p"}"#).is_err());
        assert!(decode(
            Route::List,
            br#"{"project_id":"p","before":null,"item_id":"i"}"#
        )
        .is_err());
    }
}
