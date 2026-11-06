//! Fixed native project operations; no client/SQL capability escapes the owner.
use super::*;
use crate::{
    native_ingress::Form,
    project::{self, Failure, Prepared},
};
use task_contract::project::EstimateUnit;
macro_rules! proof { () => {
    "ROW($1::text::kehila.ident,$2::text::kehila.ident,$3::text::kehila.u64,$4::text::kehila.u64,$5::text::kehila.u64)::kehila_auth.authority"
}; }
mod item_pending;
mod item_read;
pub(super) mod item_write;
mod read;
mod replay;
mod write;
pub(crate) use item_read::Item;
pub(crate) use read::Current;
pub(crate) use replay::Replay;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum State {
    Prepared,
    Completed,
    Cancelled,
    Expired,
}

pub(crate) struct Pending {
    pub(crate) id: i64,
    pub(crate) handle: String,
    pub(crate) token: String,
    pub(crate) form: Option<Form>,
    pub(crate) registered_ms: u64,
    pub(crate) expires_ms: u64,
    pub(crate) state: State,
    pub(crate) project: Option<String>,
    pub(crate) operation: Option<i64>,
}
pub(crate) struct Page {
    pub(crate) entries: Vec<Pending>,
    pub(crate) before: Option<i64>,
}

pub(crate) enum NativeError {
    Database(DbError),
    Rejected(Failure),
    Integrity,
}
impl From<DbError> for NativeError {
    fn from(error: DbError) -> Self {
        Self::Database(error)
    }
}
type Result<T> = std::result::Result<T, NativeError>;

macro_rules! fields { () => {
    "SELECT request_id,installation_id::text,actor_id::text,intent_handle::text,token::text,form_version,name,prefix::text,estimate_unit,registered_ms::text,expires_ms::text,state,target_project_id::text,operation_row_id FROM "
}; }
const REGISTER: &str = concat!(fields!(), "kehila_auth.register_project(",proof!(),
    ",$6::text::kehila_auth.intent_key,$7::text::kehila_auth.intent_key,$8::text,$9::text::kehila.project_prefix,$10::text)");
const OPEN: &str = concat!(
    fields!(),
    "kehila_auth.open_project(",
    proof!(),
    ",$6::bigint)"
);
const CANCEL: &str = concat!(
    fields!(),
    "kehila_auth.cancel_project(",
    proof!(),
    ",$6::bigint)"
);
const LIST: &str = concat!(
    fields!(),
    "kehila_auth.list_projects(",
    proof!(),
    ",$6::bigint)"
);
const COMPLETE: &str = concat!("SELECT kehila_auth.complete_project(",proof!(),
    ",$6::bigint,$7::text::kehila_auth.intent_key,$8::text,$9::text::kehila.project_prefix,$10::text,$11::text::kehila.ident,$12::bigint)");

fn scalar(value: String) -> Result<u64> {
    let number = value.parse::<u64>().map_err(|_| NativeError::Integrity)?;
    if number.to_string() != value {
        return Err(NativeError::Integrity);
    }
    Ok(number)
}
fn get<T>(row: &Row, index: usize) -> Result<T>
where
    T: for<'a> tokio_postgres::types::FromSql<'a>,
{
    row.try_get(index).map_err(|_| NativeError::Integrity)
}
fn key(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn pending(row: Row, authority: &Authority) -> Result<Pending> {
    let id: i64 = get(&row, 0)?;
    let installation: String = get(&row, 1)?;
    let actor: String = get(&row, 2)?;
    let handle: String = get(&row, 3)?;
    let token: String = get(&row, 4)?;
    let version: i32 = get(&row, 5)?;
    let name: Option<String> = get(&row, 6)?;
    let prefix: Option<String> = get(&row, 7)?;
    let unit: Option<String> = get(&row, 8)?;
    let form = match (name, prefix, unit) {
        (Some(name), Some(prefix), Some(unit))
            if !name.is_empty()
                && name.len() <= 256
                && !prefix.is_empty()
                && prefix.len() <= 12 =>
        {
            Some(Form {
                name,
                prefix,
                estimate_unit: match unit.as_str() {
                    "hours" => EstimateUnit::Hours,
                    "points" => EstimateUnit::Points,
                    _ => return Err(NativeError::Integrity),
                },
            })
        }
        (None, None, None) => None,
        _ => return Err(NativeError::Integrity),
    };
    let registered_ms = scalar(get(&row, 9)?)?;
    let expires_ms = scalar(get(&row, 10)?)?;
    let state = match get::<String>(&row, 11)?.as_str() {
        "prepared" => State::Prepared,
        "completed" => State::Completed,
        "cancelled" => State::Cancelled,
        "expired" => State::Expired,
        _ => return Err(NativeError::Integrity),
    };
    let project: Option<String> = get(&row, 12)?;
    let operation: Option<i64> = get(&row, 13)?;
    let shape = match state {
        State::Prepared => form.is_some() && project.is_none() && operation.is_none(),
        State::Completed => {
            project
                .as_ref()
                .is_some_and(|id| !id.is_empty() && id.len() <= 128)
                && operation.is_some_and(|id| id > 0)
        }
        State::Cancelled | State::Expired => {
            form.is_none() && project.is_none() && operation.is_none()
        }
    };
    if id <= 0
        || installation != authority.installation
        || actor != authority.owner
        || !key(&handle)
        || !key(&token)
        || version != 1
        || !expires_ms
            .checked_sub(registered_ms)
            .is_some_and(|delta| (60_000..=604_800_000).contains(&delta))
        || !shape
    {
        return Err(NativeError::Integrity);
    }
    Ok(Pending {
        id,
        handle,
        token,
        form,
        registered_ms,
        expires_ms,
        state,
        project,
        operation,
    })
}

impl<P> Fenced<'_, P> {
    fn native_proof(&self) -> [String; 5] {
        let a = &self.authority;
        [
            a.installation.clone(),
            a.owner.clone(),
            a.runtime.to_string(),
            a.authentication.to_string(),
            a.revision.to_string(),
        ]
    }
}

impl Fenced<'_, SessionProof> {
    pub(crate) async fn register_project(
        &mut self,
        form: &Form,
        handle: &str,
        token: &str,
    ) -> Result<Pending> {
        let a = self.native_proof();
        let unit = project::unit_name(form.estimate_unit);
        let mut rows = self
            .rows(
                REGISTER,
                &[
                    &a[0],
                    &a[1],
                    &a[2],
                    &a[3],
                    &a[4],
                    &handle,
                    &token,
                    &form.name,
                    &form.prefix,
                    &unit,
                ],
            )
            .await?;
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        pending(rows.remove(0), &self.authority)
    }
    pub(crate) async fn list_projects(&mut self, before: Option<i64>) -> Result<Page> {
        let a = self.native_proof();
        let policy = self
            .run(
                "SELECT page_limit FROM kehila_auth.pending_policy WHERE singleton",
                &[],
            )
            .await?;
        let selected: i32 = get(&policy, 0)?;
        if !(1..=32).contains(&selected) {
            return Err(NativeError::Integrity);
        }
        let rows = self
            .rows(LIST, &[&a[0], &a[1], &a[2], &a[3], &a[4], &before])
            .await?;
        if rows.len() > selected as usize + 1 {
            return Err(NativeError::Integrity);
        }
        let mut entries: Vec<Pending> = rows
            .into_iter()
            .map(|row| pending(row, &self.authority))
            .collect::<Result<_>>()?;
        if entries.windows(2).any(|pair| pair[0].id <= pair[1].id)
            || before.is_some_and(|cursor| entries.first().is_some_and(|entry| entry.id >= cursor))
        {
            return Err(NativeError::Integrity);
        }
        let before = if entries.len() > selected as usize {
            entries.pop();
            entries.last().map(|entry| entry.id)
        } else {
            None
        };
        Ok(Page { entries, before })
    }
    pub(crate) async fn open_project(&mut self, id: i64, cancel: bool) -> Result<Option<Pending>> {
        let a = self.native_proof();
        let mut rows = self
            .rows(
                if cancel { CANCEL } else { OPEN },
                &[&a[0], &a[1], &a[2], &a[3], &a[4], &id],
            )
            .await?;
        if rows.len() > 1 {
            return Err(NativeError::Integrity);
        }
        rows.pop()
            .map(|row| pending(row, &self.authority))
            .transpose()
    }
    pub(crate) async fn complete_project(
        &mut self,
        id: i64,
        prepared: &Prepared,
        operation: i64,
    ) -> Result<()> {
        let a = self.native_proof();
        let command = prepared.command();
        let unit = project::unit_name(command.estimate_unit);
        self.run(
            COMPLETE,
            &[
                &a[0],
                &a[1],
                &a[2],
                &a[3],
                &a[4],
                &id,
                &command.operation_id,
                &command.name,
                &command.prefix,
                &unit,
                &command.project_id.0,
                &operation,
            ],
        )
        .await?;
        Ok(())
    }
}

impl<P> Fenced<'_, P> {
    pub(crate) async fn trusted_millis(&mut self) -> Result<u64> {
        let row = self
            .run("SELECT kehila_auth.trusted_millis()::text", &[])
            .await?;
        scalar(get(&row, 0)?)
    }
    pub(crate) async fn force_project_constraints(&mut self) -> Result<()> {
        if !self
            .rows("SET CONSTRAINTS ALL IMMEDIATE", &[])
            .await?
            .is_empty()
        {
            return Err(NativeError::Integrity);
        }
        Ok(())
    }
}
