//! Selection read witnesses are constructed only inside an authorized snapshot.
//! Owned bounded records are verified after rollback, outside the database lane.
use super::*;
use task_contract::{
    current_work::{CurrentWork, UserId},
    relationship::EndpointIdentity,
    work_item::{ProjectId, WorkItemId},
};
mod command;
pub(crate) use command::{
    CommandDraft, CommandPacket, Identity, PacketKind, Resolution, SaveOutcome, SelectionFailure,
};

fn pair(project: Option<String>, item: Option<String>) -> Result<Option<EndpointIdentity>> {
    match (project, item) {
        (None, None) => Ok(None),
        (Some(project), Some(item))
            if selection_codec::valid_identifier(&project)
                && selection_codec::valid_identifier(&item) =>
        {
            Ok(Some(EndpointIdentity {
                project_id: ProjectId(project),
                item_id: WorkItemId(item),
            }))
        }
        _ => Err(NativeError::Integrity),
    }
}

pub(crate) struct ReadDraft {
    installation: String,
    current: CurrentWork,
    ordinal: u64,
    operation: Option<i64>,
    event: Option<Row>,
    visible: bool,
}

impl ReadDraft {
    fn verify(self) -> Result<(CurrentWork, u64, bool)> {
        match (self.operation, self.event) {
            (None, None)
                if self.ordinal == 0
                    && self.current.version == 0
                    && self.current.selected.is_none() => {}
            (Some(operation), Some(row)) => {
                let event =
                    verified_event(&self.installation, &self.current.user_id.0, operation, &row)?;
                if event.ordinal != self.ordinal || event.result.next != self.current {
                    return Err(NativeError::Integrity);
                }
            }
            _ => return Err(NativeError::Integrity),
        }
        Ok((self.current, self.ordinal, self.visible))
    }
    pub(crate) fn admit(self) -> Result<serde_json::Value> {
        let (current, _, visible) = self.verify()?;
        let target = match current.selected {
            None => serde_json::json!({"state":"none"}),
            Some(value) if visible => serde_json::json!({"state":"available",
                "project_id":value.project_id.0,"item_id":value.item_id.0}),
            Some(_) => serde_json::json!({"state":"unavailable"}),
        };
        Ok(serde_json::json!({"codec_version":1,"outcome":"read",
            "version":current.version.to_string(),"target":target}))
    }
}

fn verified_event(
    installation: &str,
    actor: &str,
    operation: i64,
    row: &Row,
) -> Result<selection_codec::Event> {
    let codec = u32::try_from(get::<i64>(row, 12)?).map_err(|_| NativeError::Integrity)?;
    let history = get::<Option<String>>(row, 13)?.ok_or(NativeError::Integrity)?;
    let digest = get::<Option<Vec<u8>>>(row, 20)?.ok_or(NativeError::Integrity)?;
    let event = crate::selection::verify_retained_event(
        installation,
        actor,
        codec,
        history.as_bytes(),
        &digest,
    )
    .map_err(|_| NativeError::Integrity)?;
    let ordinal = scalar(get(row, 4)?)?;
    let previous = get::<Option<String>>(row, 5)?.map(scalar).transpose()?;
    let before = pair(get(row, 7)?, get(row, 8)?)?;
    let after = pair(get(row, 10)?, get(row, 11)?)?;
    let origin = scalar(get(row, 24)?)?;
    let deadline = scalar(get(row, 25)?)?;
    if operation <= 0
        || get::<i64>(row, 3)? != operation
        || get::<String>(row, 0)? != actor
        || get::<String>(row, 1)? != actor
        || get::<String>(row, 2)? != event.operation_id
        || ordinal != event.ordinal
        || previous
            != if ordinal == 1 {
                None
            } else {
                ordinal.checked_sub(1)
            }
        || scalar(get(row, 6)?)? != event.before.version
        || before != event.before.selected
        || scalar(get(row, 9)?)? != event.result.next.version
        || after != event.result.next.selected
        || get::<String>(row, 14)? != actor
        || get::<String>(row, 15)? != "selection"
        || get::<String>(row, 16)? != "user"
        || get::<Option<String>>(row, 17)?.as_deref() != Some(actor)
        || get::<String>(row, 18)? != event.operation_id
        || get::<i64>(row, 19)? != 1
        || get::<String>(row, 21)? != "selection"
        || get::<i32>(row, 22)? != 1
        || !get::<bool>(row, 23)?
        || deadline.checked_sub(origin) != Some(7_776_000_000)
    {
        return Err(NativeError::Integrity);
    }
    Ok(event)
}

impl Fenced<'_, ReadProof> {
    async fn selection_head(&mut self) -> Result<ReadDraft> {
        let owner = self.authority.owner.clone();
        let rows = self.rows("SELECT h.selection_version::numeric(20,0)::text,h.selected_project_id::text, \
            h.selected_item_id::text,h.event_ordinal::numeric(20,0)::text, \
            (SELECT e.operation_row_id FROM kehila.selection_events e WHERE e.user_id=h.user_id AND e.event_ordinal=h.event_ordinal), \
            (SELECT e.event_ordinal::numeric(20,0)::text FROM kehila.selection_events e WHERE e.user_id=h.user_id ORDER BY e.event_ordinal DESC LIMIT 1) \
            FROM kehila.current_work_selections h WHERE h.user_id=$1::text::kehila.ident LIMIT 2", &[&owner]).await?;
        if rows.len() > 1 {
            return Err(NativeError::Integrity);
        }
        let mut draft = ReadDraft {
            installation: self.authority.installation.clone(),
            current: CurrentWork {
                user_id: UserId(owner.clone()),
                version: 0,
                selected: None,
            },
            ordinal: 0,
            operation: None,
            event: None,
            visible: true,
        };
        if let Some(row) = rows.into_iter().next() {
            draft.current.version = scalar(get(&row, 0)?)?;
            draft.current.selected = pair(get(&row, 1)?, get(&row, 2)?)?;
            draft.ordinal = scalar(get(&row, 3)?)?;
            let operation = get::<Option<i64>>(&row, 4)?.ok_or(NativeError::Integrity)?;
            if draft.ordinal == 0
                || get::<Option<String>>(&row, 5)?.as_deref()
                    != Some(draft.ordinal.to_string().as_str())
            {
                return Err(NativeError::Integrity);
            }
            let mut events = self
                .rows(include_str!("selection/read_event.sql"), &[&operation])
                .await?;
            if events.len() != 1 {
                return Err(NativeError::Integrity);
            }
            draft.operation = Some(operation);
            draft.event = Some(events.remove(0));
        } else {
            let row = self.run("SELECT EXISTS(SELECT FROM kehila.selection_events WHERE user_id=$1::text::kehila.ident)", &[&owner]).await?;
            if get::<bool>(&row, 0)? {
                return Err(NativeError::Integrity);
            }
        }
        Ok(draft)
    }
    pub(crate) async fn selection_read(&mut self) -> Result<ReadDraft> {
        let mut draft = self.selection_head().await?;
        if let Some(target) = &draft.current.selected {
            match self.authorize_item_scope(&target.project_id.0).await {
                Ok(()) => (),
                Err(NativeError::Database(DbError::Rejected(code))) if code == "42501" => {
                    // This error aborts the RR transaction. No more SQL is
                    // issued: the owner must confirm rollback before adoption.
                    draft.visible = false;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(draft)
    }
}
