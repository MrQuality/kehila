use crate::{
    Error, MAX_COLLECTION_ENTRIES, MAX_DEPTH, MAX_NODES, MAX_RECORD_BYTES, MAX_STRING_BYTES,
};
use serde_json::Value;
use task_contract::project::Project;
use task_contract::project_create::ProjectCreateResult;
use task_contract::work_item::Configuration;

#[derive(Default)]
struct Budget {
    strings: usize,
    entries: usize,
    native: bool,
}
impl Budget {
    fn text(&mut self, value: &str) -> Result<(), Error> {
        if self.native && value.contains('\0') {
            return Err(Error::InvalidNativeSnapshot("JSONB cannot store NUL"));
        }
        self.strings = self
            .strings
            .checked_add(value.len())
            .ok_or(Error::LimitExceeded)?;
        if value.len() > MAX_STRING_BYTES || self.strings > MAX_RECORD_BYTES {
            return Err(Error::LimitExceeded);
        }
        Ok(())
    }
    fn collection<T>(&mut self, values: &[T]) -> Result<(), Error> {
        self.entries = self
            .entries
            .checked_add(values.len())
            .ok_or(Error::LimitExceeded)?;
        if values.len() > MAX_COLLECTION_ENTRIES || self.entries > MAX_NODES {
            return Err(Error::LimitExceeded);
        }
        Ok(())
    }
    fn project(&mut self, p: &Project) -> Result<(), Error> {
        self.text(&p.id.0)?;
        self.text(&p.name)?;
        self.text(&p.prefix)
    }
    fn configuration(&mut self, c: &Configuration) -> Result<(), Error> {
        self.text(&c.project_id.0)?;
        self.collection(&c.statuses)?;
        for s in &c.statuses {
            self.text(&s.id.0)?;
            self.text(&s.name)?;
            if let Some(id) = &s.group_id {
                self.text(&id.0)?;
            }
        }
        self.collection(&c.status_groups)?;
        for g in &c.status_groups {
            self.text(&g.id.0)?;
            self.text(&g.name)?;
        }
        self.collection(&c.workflows)?;
        for w in &c.workflows {
            self.text(&w.id.0)?;
            self.text(&w.initial_status_id.0)?;
            self.collection(&w.status_ids)?;
            for id in &w.status_ids {
                self.text(&id.0)?;
            }
            self.collection(&w.permitted_phase_changes)?;
        }
        self.collection(&c.types)?;
        for t in &c.types {
            self.text(&t.id.0)?;
            self.text(&t.default_workflow_id.0)?;
            if let Some(id) = &t.title_field_id {
                self.text(&id.0)?;
            }
            self.collection(&t.permitted_workflows)?;
            for id in &t.permitted_workflows {
                self.text(&id.0)?;
            }
        }
        self.collection(&c.fields)?;
        for f in &c.fields {
            self.text(&f.id.0)?;
            self.text(&f.owner_type.0)?;
            self.text(&f.name)?;
            self.collection(&f.options)?;
            for o in &f.options {
                self.text(&o.id.0)?;
                self.text(&o.name)?;
            }
        }
        self.collection(&c.relationship_types)?;
        for r in &c.relationship_types {
            self.text(&r.owner_project_id.0)?;
            self.text(&r.id.0)?;
            self.text(&r.name)?;
        }
        Ok(())
    }
}

pub(crate) fn result(result: &ProjectCreateResult) -> Result<(), Error> {
    let mut budget = Budget::default();
    budget.project(&result.project)?;
    budget.configuration(&result.configuration)
}
pub(crate) fn project(p: &Project) -> Result<(), Error> {
    Budget::default().project(p)
}
pub(crate) fn configuration(c: &Configuration) -> Result<(), Error> {
    Budget::default().configuration(c)
}

pub(crate) fn native_project(p: &Project) -> Result<(), Error> {
    Budget {
        native: true,
        ..Budget::default()
    }
    .project(p)
}

pub(crate) fn native_configuration(c: &Configuration) -> Result<(), Error> {
    Budget {
        native: true,
        ..Budget::default()
    }
    .configuration(c)
}

// The typed preflight above bounds tree construction. Count the resulting exact
// shape before serialization, so encoded records obey the decoder's node limit.
fn shape(value: &Value, depth: usize, nodes: &mut usize, spaces: &mut usize) -> Result<(), Error> {
    if depth > MAX_DEPTH || *nodes == MAX_NODES {
        return Err(Error::LimitExceeded);
    }
    *nodes += 1;
    match value {
        Value::Array(values) => {
            *spaces += values.len().saturating_sub(1);
            if values.len() > MAX_COLLECTION_ENTRIES {
                return Err(Error::LimitExceeded);
            }
            for child in values {
                shape(child, depth + 1, nodes, spaces)?;
            }
        }
        Value::Object(values) => {
            *spaces += values.len() + values.len().saturating_sub(1);
            if values.len() > MAX_COLLECTION_ENTRIES {
                return Err(Error::LimitExceeded);
            }
            for child in values.values() {
                shape(child, depth + 1, nodes, spaces)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// A bounded in-memory sink; implementing Write performs no external I/O.
struct Record {
    bytes: Vec<u8>,
    limit: usize,
}
impl std::io::Write for Record {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit - self.bytes.len() {
            return Err(std::io::Error::other("record limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) fn serialize(value: &Value) -> Result<Vec<u8>, Error> {
    serialize_bounded(value, false)
}

/// PostgreSQL 16 JSONB adds one space after each structural colon/comma.
/// Count it in the existing shape traversal before allocating encoded bytes.
/// Typed native preflight already rejected NUL; supported scalar escaping has
/// the same byte length as compact serde_json output.
pub(crate) fn serialize_native(value: &Value) -> Result<Vec<u8>, Error> {
    serialize_bounded(value, true)
}

fn serialize_bounded(value: &Value, native: bool) -> Result<Vec<u8>, Error> {
    let mut spaces = 0;
    shape(value, 0, &mut 0, &mut spaces)?;
    let mut record = Record {
        bytes: Vec::new(),
        limit: MAX_RECORD_BYTES - if native { spaces } else { 0 },
    };
    // Value serialization has no user callbacks or fallible external I/O; the
    // bounded sink is its only possible failure for these supported values.
    serde_json::to_writer(&mut record, value).map_err(|_| Error::LimitExceeded)?;
    Ok(record.bytes)
}
