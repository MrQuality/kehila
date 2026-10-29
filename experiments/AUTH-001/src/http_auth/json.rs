//! Deserialize directly into closed shapes, never an arbitrary JSON value.
use crate::password::NormalizedPassword;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};
use std::fmt;
use zeroize::Zeroizing;

struct Password(Zeroizing<String>);

impl<'de> Deserialize<'de> for Password {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PasswordVisitor;
        impl Visitor<'_> for PasswordVisitor {
            type Value = Password;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a bounded password string")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Password, E> {
                if value.len() > 1024 {
                    return Err(E::custom("password size"));
                }
                let mut owned = Zeroizing::new(String::with_capacity(1024));
                owned.push_str(value);
                Ok(Password(owned))
            }
        }
        deserializer.deserialize_str(PasswordVisitor)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    password: Password,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Change {
    current: Password,
    replacement: Password,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

pub(super) enum Input {
    Empty,
    Login(NormalizedPassword),
    Change {
        current: NormalizedPassword,
        replacement: NormalizedPassword,
    },
}

pub(super) fn decode(route: super::Route, body: &[u8]) -> Result<Input, ()> {
    if body.len() > super::BODY_LIMIT {
        return Err(());
    }
    // Serde structs can also accept positional sequences. The wire contract is
    // explicitly a JSON object, including for the empty logout message.
    if matches!(
        route,
        super::Route::Login(_)
            | super::Route::Password(_)
            | super::Route::Logout(_)
            | super::Route::RevokeOthers(_)
    ) && body.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{')
    {
        return Err(());
    }
    let normalize = |value: Password| NormalizedPassword::new(value.0).map_err(|_| ());
    match route {
        super::Route::Login(_) | super::Route::RevokeOthers(_) => {
            let value: Login = serde_json::from_slice(body).map_err(|_| ())?;
            Ok(Input::Login(normalize(value.password)?))
        }
        super::Route::Password(_) => {
            let value: Change = serde_json::from_slice(body).map_err(|_| ())?;
            Ok(Input::Change {
                current: normalize(value.current)?,
                replacement: normalize(value.replacement)?,
            })
        }
        super::Route::Logout(_) => {
            let _: Empty = serde_json::from_slice(body).map_err(|_| ())?;
            Ok(Input::Empty)
        }
        _ if body.is_empty() => Ok(Input::Empty),
        _ => Err(()),
    }
}
