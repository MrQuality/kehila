use crate::{
    Error, MAX_COLLECTION_ENTRIES, MAX_DEPTH, MAX_NODES, MAX_RECORD_BYTES, MAX_STRING_BYTES,
};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

#[derive(Default)]
struct Budget {
    nodes: usize,
    failure: Option<Error>,
}

impl Budget {
    fn fail<E: de::Error>(&mut self, failure: Error) -> E {
        self.failure = Some(failure);
        E::custom("invalid bounded JSON")
    }
}

struct Seed<'a> {
    budget: &'a mut Budget,
    depth: usize,
    capacity: bool,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        if !self.capacity || self.depth > MAX_DEPTH || self.budget.nodes == MAX_NODES {
            return Err(self.budget.fail(Error::LimitExceeded));
        }
        self.budget.nodes += 1;
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| self.budget.fail(Error::Malformed("number")))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        if v.len() > MAX_STRING_BYTES {
            return Err(self.budget.fail(Error::LimitExceeded));
        }
        Ok(Value::String(v.to_owned()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        if v.len() > MAX_STRING_BYTES {
            return Err(self.budget.fail(Error::LimitExceeded));
        }
        Ok(Value::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        // Ignore untrusted size hints. A seed rejects the next value before
        // allocating it when capacity is exhausted; an empty tail stays valid.
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(Seed {
            budget: self.budget,
            depth: self.depth + 1,
            capacity: values.len() < MAX_COLLECTION_ENTRIES,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if key.len() > MAX_STRING_BYTES || values.len() == MAX_COLLECTION_ENTRIES {
                return Err(self.budget.fail(Error::LimitExceeded));
            }
            // Compare decoded keys, including escaped spellings, before the
            // next value is parsed or a duplicate can overwrite its predecessor.
            if values.contains_key(&key) {
                return Err(self.budget.fail(Error::Malformed("duplicate key")));
            }
            let value = map.next_value_seed(Seed {
                budget: self.budget,
                depth: self.depth + 1,
                capacity: true,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

/// First-parse boundary for raw JSON: reject duplicate decoded keys, trailing
/// input and resource overflow before a Value can lose that information.
/// This validates representation only; callers must still validate typed shape.
/// Transport readers must reject overflow before collecting the supplied slice.
pub fn parse_json(bytes: &[u8]) -> Result<Value, Error> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(Error::LimitExceeded);
    }
    let mut budget = Budget::default();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let result = Seed {
        budget: &mut budget,
        depth: 0,
        capacity: true,
    }
    .deserialize(&mut deserializer);
    match result {
        Ok(value) => {
            deserializer
                .end()
                .map_err(|_| Error::Malformed("trailing JSON"))?;
            Ok(value)
        }
        Err(_) => Err(budget.failure.unwrap_or(Error::Malformed("JSON syntax"))),
    }
}
