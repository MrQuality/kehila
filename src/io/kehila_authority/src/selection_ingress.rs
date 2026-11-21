//! Closed original-byte selection admission; no caller authority is accepted.
use serde_json::{Map, Value};
use task_contract::{
    relationship::EndpointIdentity,
    work_item::{ProjectId, WorkItemId},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Route {
    Read,
    Set,
    Reconcile,
}
impl Route {
    pub(crate) fn from_path(path: &str) -> Option<Self> {
        match path {
            "/selection/read" => Some(Self::Read),
            "/selection/set" => Some(Self::Set),
            "/selection/reconcile" => Some(Self::Reconcile),
            _ => None,
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Input {
    Read,
    Command {
        reconcile: bool,
        token: String,
        expected_version: u64,
        requested: Option<EndpointIdentity>,
    },
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Invalid;
fn identifier(object: &mut Map<String, Value>, name: &str) -> Result<String, Invalid> {
    match object.remove(name) {
        Some(Value::String(value)) if selection_codec::valid_identifier(&value) => Ok(value),
        _ => Err(Invalid),
    }
}
pub(crate) fn decode(route: Route, bytes: &[u8]) -> Result<Input, Invalid> {
    if bytes.len() > selection_codec::MAX_WIRE_BYTES {
        return Err(Invalid);
    }
    let Value::Object(mut object) = project_create_codec::parse_json(bytes).map_err(|_| Invalid)?
    else {
        return Err(Invalid);
    };
    if route == Route::Read {
        return if object.is_empty() {
            Ok(Input::Read)
        } else {
            Err(Invalid)
        };
    }
    let token = match object.remove("operation_token") {
        Some(Value::String(value)) if selection_codec::valid_operation_token(&value) => value,
        _ => return Err(Invalid),
    };
    let expected_version = match object.remove("expected_version") {
        Some(Value::String(value)) => {
            let number: u64 = value.parse().map_err(|_| Invalid)?;
            if number.to_string() != value {
                return Err(Invalid);
            }
            number
        }
        _ => return Err(Invalid),
    };
    let requested = match object.remove("requested") {
        Some(Value::Null) => None,
        Some(Value::Object(mut target)) => {
            let project_id = ProjectId(identifier(&mut target, "project_id")?);
            let item_id = WorkItemId(identifier(&mut target, "item_id")?);
            if !target.is_empty() {
                return Err(Invalid);
            }
            Some(EndpointIdentity {
                project_id,
                item_id,
            })
        }
        _ => return Err(Invalid),
    };
    if !object.is_empty() {
        return Err(Invalid);
    }
    Ok(Input::Command {
        reconcile: route == Route::Reconcile,
        token,
        expected_version,
        requested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use task_contract::{
        relationship::EndpointIdentity,
        work_item::{ProjectId, WorkItemId},
    };

    fn set() -> &'static [u8] {
        br#"{"operation_token":"0123456789abcdef0123456789abcdef","expected_version":"0","requested":{"project_id":"p","item_id":"a"}}"#
    }

    #[test]
    fn exact_routes_and_read_shape_are_closed() {
        assert_eq!(Route::from_path("/selection/read"), Some(Route::Read));
        assert_eq!(Route::from_path("/selection/set"), Some(Route::Set));
        assert_eq!(
            Route::from_path("/selection/reconcile"),
            Some(Route::Reconcile)
        );
        for path in [
            "/selection",
            "/selection/archive",
            "/selection/set/",
            "/items/select",
        ] {
            assert!(Route::from_path(path).is_none());
        }
        assert_eq!(decode(Route::Read, b"{}"), Ok(Input::Read));
        for body in [b"[]".as_slice(), br#"{"user_id":"owner"}"#, b"null"] {
            assert!(decode(Route::Read, body).is_err());
        }
    }

    #[test]
    fn set_and_reconcile_retain_the_same_exact_command_values() {
        for (route, reconcile) in [(Route::Set, false), (Route::Reconcile, true)] {
            assert_eq!(
                decode(route, set()),
                Ok(Input::Command {
                    reconcile,
                    token: "0123456789abcdef0123456789abcdef".into(),
                    expected_version: 0,
                    requested: Some(EndpointIdentity {
                        project_id: ProjectId("p".into()),
                        item_id: WorkItemId("a".into()),
                    }),
                })
            );
        }
        let clear = br#"{"operation_token":"0123456789abcdef0123456789abcdef","expected_version":"18446744073709551615","requested":null}"#;
        assert!(matches!(
            decode(Route::Set, clear),
            Ok(Input::Command {
                requested: None,
                expected_version: u64::MAX,
                ..
            })
        ));
    }

    #[test]
    fn neither_payload_authority_nor_unknown_fields_are_accepted() {
        let baseline: serde_json::Value = serde_json::from_slice(set()).unwrap();
        for field in [
            "actor_id",
            "user_id",
            "installation_id",
            "authorized",
            "grant",
            "extra",
        ] {
            let mut changed = baseline.clone();
            changed[field] = serde_json::json!("owner");
            assert!(decode(Route::Set, &serde_json::to_vec(&changed).unwrap()).is_err());
        }
        for field in ["operation_token", "expected_version", "requested"] {
            let mut changed = baseline.clone();
            changed.as_object_mut().unwrap().remove(field);
            assert!(decode(Route::Set, &serde_json::to_vec(&changed).unwrap()).is_err());
        }
    }

    #[test]
    fn duplicate_decoded_keys_and_invalid_target_pairs_reject() {
        let duplicate = br#"{"operation_token":"0123456789abcdef0123456789abcdef","expected_version":"0","expected_versio\u006e":"1","requested":null}"#;
        assert!(decode(Route::Set, duplicate).is_err());
        let baseline: serde_json::Value = serde_json::from_slice(set()).unwrap();
        for target in [
            serde_json::json!({"project_id":"p"}),
            serde_json::json!({"project_id":"p","item_id":""}),
            serde_json::json!({"project_id":"p","item_id":"a","extra":null}),
            serde_json::json!({"project_id":"p","item_id":"a\0b"}),
            serde_json::json!({"project_id":"p","item_id":"x".repeat(129)}),
            serde_json::json!(false),
        ] {
            let mut changed = baseline.clone();
            changed["requested"] = target;
            assert!(decode(Route::Set, &serde_json::to_vec(&changed).unwrap()).is_err());
        }
    }

    #[test]
    fn versions_tokens_and_original_byte_limit_do_not_coerce() {
        let baseline: serde_json::Value = serde_json::from_slice(set()).unwrap();
        for version in [
            serde_json::json!(0),
            serde_json::json!("00"),
            serde_json::json!("+1"),
            serde_json::json!("18446744073709551616"),
        ] {
            let mut changed = baseline.clone();
            changed["expected_version"] = version;
            assert!(decode(Route::Set, &serde_json::to_vec(&changed).unwrap()).is_err());
        }
        for token in [
            "",
            "op",
            "0123456789ABCDEF0123456789ABCDEF",
            &"0".repeat(33),
        ] {
            let mut changed = baseline.clone();
            changed["operation_token"] = serde_json::json!(token);
            assert!(decode(Route::Set, &serde_json::to_vec(&changed).unwrap()).is_err());
        }
        let mut at_limit = set().to_vec();
        at_limit.resize(4096, b' ');
        assert!(decode(Route::Set, &at_limit).is_ok());
        at_limit.push(b' ');
        assert!(decode(Route::Set, &at_limit).is_err());
    }
}
