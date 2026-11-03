//! Finite outer API envelope, distinct from each versioned storage record.
use super::Error;
use project_create_codec::{
    MAX_COLLECTION_ENTRIES, MAX_DEPTH, MAX_NODES, MAX_RECORD_BYTES, MAX_STRING_BYTES,
};
use serde_json::Value;
use std::io::{self, Write};
use zeroize::Zeroizing;

pub(crate) const MAX_WIRE_BYTES: usize = MAX_RECORD_BYTES + 8192;
const MAX_WIRE_DEPTH: usize = MAX_DEPTH + 2;
const MAX_WIRE_NODES: usize = MAX_NODES + 256;

#[derive(Default)]
struct Budget {
    nodes: usize,
    strings: usize,
}
impl Budget {
    fn text(&mut self, text: &str) -> Result<(), Error> {
        self.strings = self
            .strings
            .checked_add(text.len())
            .ok_or(Error::Unavailable)?;
        if text.len() > MAX_STRING_BYTES || self.strings > MAX_WIRE_BYTES {
            return Err(Error::Unavailable);
        }
        Ok(())
    }
    fn value(&mut self, value: &Value, depth: usize) -> Result<(), Error> {
        if depth > MAX_WIRE_DEPTH || self.nodes == MAX_WIRE_NODES {
            return Err(Error::Unavailable);
        }
        self.nodes += 1;
        match value {
            Value::String(text) => self.text(text)?,
            Value::Array(array) => {
                if array.len() > MAX_COLLECTION_ENTRIES {
                    return Err(Error::Unavailable);
                }
                for value in array {
                    self.value(value, depth + 1)?;
                }
            }
            Value::Object(object) => {
                if object.len() > MAX_COLLECTION_ENTRIES {
                    return Err(Error::Unavailable);
                }
                for (key, value) in object {
                    self.text(key)?;
                    self.value(value, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
struct Encoded(Zeroizing<Vec<u8>>);
impl Write for Encoded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_WIRE_BYTES - self.0.len() {
            return Err(io::Error::other("wire record limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(crate) fn encode(value: &Value) -> Result<Zeroizing<Vec<u8>>, Error> {
    Budget::default().value(value, 0)?;
    let mut encoded = Encoded(Zeroizing::new(Vec::new()));
    serde_json::to_writer(&mut encoded, value).map_err(|_| Error::Unavailable)?;
    Ok(encoded.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outer_wire_has_its_own_finite_budget_without_relaxing_stored_records() {
        let value = Value::Array(vec![Value::String("x".repeat(MAX_STRING_BYTES)); 16]);
        let wire = encode(&value).unwrap();
        assert!(wire.len() > MAX_RECORD_BYTES && wire.len() <= MAX_WIRE_BYTES);
        assert!(project_create_codec::parse_json(&wire).is_err());
        let mut overflow = match value {
            Value::Array(array) => array,
            _ => unreachable!(),
        };
        overflow.push(Value::String("x".repeat(8192)));
        assert!(encode(&Value::Array(overflow)).is_err()); // Encoding punctuation counts too.
    }
    #[test]
    fn outer_wire_rejects_depth_nodes_collections_and_strings_before_encoding() {
        let mut deep = Value::Null;
        for _ in 0..MAX_WIRE_DEPTH + 1 {
            deep = Value::Array(vec![deep]);
        }
        assert!(encode(&deep).is_err());
        let nodes = Value::Array(vec![Value::Array(vec![Value::Null; 1024]); 17]);
        assert!(encode(&nodes).is_err());
        assert!(encode(&Value::Array(vec![Value::Null; MAX_COLLECTION_ENTRIES + 1])).is_err());
        assert!(encode(&Value::String("x".repeat(MAX_STRING_BYTES + 1))).is_err());
    }
}
