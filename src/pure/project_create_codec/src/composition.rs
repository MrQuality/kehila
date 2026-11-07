//! Narrow composition boundary for other versioned native storage records.
//!
//! Callers preflight their typed aggregate with RecordBudget before allocating a
//! JSON tree. These functions then validate the complete tree/output. They grant
//! no authority, coherence or transaction outcome. Project mapping remains v1.
use crate::{limits, v1, Error};
use serde_json::Value;
use task_contract::project::Project;

pub fn project_value(project: &Project) -> Result<Value, Error> {
    limits::project(project)?;
    Ok(v1::encode_project(project))
}

/// For values already admitted by parse_json or the complete record boundary.
pub fn project_from_value(value: &Value) -> Result<Project, Error> {
    v1::decode_project(value).map_err(Error::Malformed)
}

/// Exact recursive object-key sorting, preserving array order and scalar bytes.
/// Only integer JSON numbers are supported; exact decimal values use strings.
pub fn encode_record(value: &Value) -> Result<Vec<u8>, Error> {
    limits::canonical(value, false)
}

/// Also rejects NUL and reserves PostgreSQL 16 JSONB structural spacing.
pub fn encode_native_record(value: &Value) -> Result<Vec<u8>, Error> {
    limits::canonical(value, true)
}
