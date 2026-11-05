//! Permanent attributed history shared by current item and project reads.
use super::*;
use project_create_codec::MAX_RECORD_BYTES;
use task_contract::{
    project::{Project, ReadableId},
    work_item::WorkItem,
};
use work_item_codec::{Revision, SavedResult};

pub(crate) struct Item {
    pub(crate) result: SavedResult,
    pub(crate) readable: ReadableId,
}

impl Fenced<'_, ReadProof> {
    pub(crate) async fn read_items(
        &mut self,
        input: crate::item_ingress::Input,
    ) -> Result<serde_json::Value> {
        let project = input.project();
        let a = &self.authority;
        let proof = [
            a.installation.clone(),
            a.owner.clone(),
            a.runtime.to_string(),
            a.authentication.to_string(),
            a.revision.to_string(),
        ];
        let authority = self
            .rows(
                "SELECT (kehila_auth.item_read_authority( \
            ROW($1::text::kehila.ident,$2::text::kehila.ident,$3::text::kehila.u64, \
            $4::text::kehila.u64,$5::text::kehila.u64)::kehila_auth.authority, \
            $6::text::kehila.ident)).owner_actor_id::text",
                &[
                    &proof[0], &proof[1], &proof[2], &proof[3], &proof[4], &project,
                ],
            )
            .await?;
        if authority.len() != 1 || get::<String>(&authority[0], 0)? != proof[1] {
            return Err(NativeError::Integrity);
        }
        drop(authority);
        let current = self.read_project(project).await?;
        match input {
            crate::item_ingress::Input::Read { item, .. } => {
                crate::item::read(self.current_item(&current.project, &item).await?)
            }
            crate::item_ingress::Input::List { project, before } => {
                let upper = before.unwrap_or(u64::MAX);
                // Inclusive first-page maximum must include u64::MAX. On later
                // pages subtract only a validated positive cursor.
                let bound = if before.is_some() { upper - 1 } else { upper }.to_string();
                // Qualify the numeric base column. Its unqualified output
                // label denotes the projected text, which sorts 9 before 20.
                let rows = self.rows("SELECT i.item_id::text,i.issued_sequence::numeric(20,0)::text \
                  FROM kehila.items i WHERE i.project_id=$1::text::kehila.ident AND i.issued_sequence<=$2::text::kehila.u64 \
                  ORDER BY i.issued_sequence DESC LIMIT 17", &[&project,&bound]).await?;
                let more = rows.len() > crate::item::PAGE_SIZE;
                let mut page = crate::item::Page::new();
                let mut previous = before;
                for row in rows.into_iter().take(crate::item::PAGE_SIZE) {
                    let item: String = get(&row, 0)?;
                    let sequence = scalar(get(&row, 1)?)?;
                    if sequence == 0 || previous.is_some_and(|v| sequence >= v) {
                        return Err(NativeError::Integrity);
                    }
                    previous = Some(sequence);
                    drop(row);
                    let item = self.current_item(&current.project, &item).await?;
                    if item.readable.sequence != sequence {
                        return Err(NativeError::Integrity);
                    }
                    if !page.push(item, &current.configuration)? {
                        return Ok(page.finish(true));
                    }
                }
                Ok(page.finish(more))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_counters_may_lag_but_stable_metadata_must_agree() {
        let prepared = crate::project::Prepared::new(
            crate::native_ingress::Form {
                name: "Project".into(),
                prefix: "PR".into(),
                estimate_unit: EstimateUnit::Hours,
            },
            "p".into(),
            "0".repeat(32),
        )
        .unwrap();
        let saved = &prepared.result().project;
        let mut current = saved.clone();
        current.next_sequence = 4;
        current.ever_estimated = true;
        assert!(historical_project(saved, &current).is_ok());
        let mut changed = saved.clone();
        changed.name.push('x');
        assert!(historical_project(&changed, &current).is_err());
        let mut changed = saved.clone();
        changed.prefix = "OTHER".into();
        assert!(historical_project(&changed, &current).is_err());
        let mut changed = saved.clone();
        changed.archived = true;
        assert!(historical_project(&changed, &current).is_err());
        let mut changed = saved.clone();
        changed.next_sequence = 5;
        assert!(historical_project(&changed, &current).is_err());
        let mut changed = saved.clone();
        changed.ever_estimated = true;
        assert!(historical_project(&changed, saved).is_err());
    }
}

struct Stored {
    operation: i64,
    revision: Revision,
}
impl Stored {
    fn bind_project(&self, current: &Project) -> Result<()> {
        historical_project(self.project(), current)
    }
    fn project(&self) -> &Project {
        match &self.revision.result {
            SavedResult::Create(value) => &value.project,
            SavedResult::Edit(value) => &value.project,
        }
    }
    fn item(&self) -> &WorkItem {
        match &self.revision.result {
            SavedResult::Create(value) => &value.item,
            SavedResult::Edit(value) => &value.item,
        }
    }
}

fn historical_project(saved: &Project, current: &Project) -> Result<()> {
    // This exact fresh profile has no metadata/configuration writer. Before
    // exposing one, compare each revision with its retained historical snapshot
    // instead. Only allocation and the monotonic estimate flag can differ here.
    let mut stable = saved.clone();
    stable.next_sequence = current.next_sequence;
    stable.ever_estimated = current.ever_estimated;
    if &stable != current
        || saved.next_sequence > current.next_sequence
        || (saved.ever_estimated && !current.ever_estimated)
    {
        return Err(NativeError::Integrity);
    }
    Ok(())
}

fn stored(row: Row, project: &str, item: &str, version: u64) -> Result<Stored> {
    let codec: i64 = get(&row, 15)?;
    let bytes: Option<String> = get(&row, 16)?;
    let bytes = bytes.ok_or(NativeError::Integrity)?;
    if bytes.len() > MAX_RECORD_BYTES || !get::<bool>(&row, 21)? || !get::<bool>(&row, 22)? {
        return Err(NativeError::Integrity);
    }
    let revision = work_item_codec::decode_revision(
        u32::try_from(codec).map_err(|_| NativeError::Integrity)?,
        bytes.as_bytes(),
    )
    .map_err(|_| NativeError::Integrity)?;
    let (saved_project, saved_item, data, phase, family) = match &revision.result {
        SavedResult::Create(value) => (
            &value.project,
            &value.item,
            &value.data,
            value.phase,
            "item_create",
        ),
        SavedResult::Edit(value) => (
            &value.project,
            &value.item,
            &value.data,
            value.phase,
            "item_edit",
        ),
    };
    let recorded_phase = match get::<String>(&row, 11)?.as_str() {
        "new" => task_contract::work_item::Phase::New,
        "active" => task_contract::work_item::Phase::Active,
        "done" => task_contract::work_item::Phase::Done,
        _ => return Err(NativeError::Integrity),
    };
    let estimate: Option<i32> = get(&row, 13)?;
    let operation: i64 = get(&row, 7)?;
    let previous: i64 = get(&row, 18)?;
    let token: String = get(&row, 6)?;
    if get::<String>(&row, 0)? != project
        || get::<String>(&row, 1)? != item
        || scalar(get(&row, 2)?)? != version
        || version == 0
        || revision.configuration_revision != scalar(get(&row, 3)?)?
        || revision.configuration_revision == 0
        || revision.actor_id.0 != get::<String>(&row, 4)?
        || get::<String>(&row, 5)? != family
        || operation <= 0
        || previous <= 0
        || token.is_empty()
        || token.len() > 128
        || !token.bytes().all(|b| (b'!'..=b'~').contains(&b))
        || saved_item.id.0 != item
        || saved_item.project_id.0 != project
        || saved_item.version != version
        || saved_item.type_id.0 != get::<String>(&row, 8)?
        || saved_item.workflow_id.0 != get::<String>(&row, 9)?
        || saved_item.status_id.0 != get::<String>(&row, 10)?
        || phase != recorded_phase
        || saved_item.archived != get::<bool>(&row, 12)?
        || data.estimate.map(|v| v.thousandths()) != estimate.and_then(|v| u64::try_from(v).ok())
        || estimate.is_some_and(|v| v < 0)
        || project::unit_name(saved_project.estimate_unit) != get::<String>(&row, 14)?
        || revision.project_state_version != scalar(get(&row, 17)?)?
        || revision.project_state_version == 0
        || revision.previous_project_operation
            != u64::try_from(previous).map_err(|_| NativeError::Integrity)?
        || saved_project.id.0 != project
        || saved_project.configuration_revision != revision.configuration_revision
        || saved_project.next_sequence != scalar(get(&row, 19)?)?
        || saved_project.next_sequence < 2
        || saved_project.ever_estimated != get::<bool>(&row, 20)?
        || (family == "item_create") != (version == 1)
    {
        return Err(NativeError::Integrity);
    }
    Ok(Stored {
        operation,
        revision,
    })
}

impl<P> Fenced<'_, P> {
    async fn item_revision(&mut self, project: &str, item: &str, version: u64) -> Result<Stored> {
        let rows = self
            .rows(
                include_str!("item_revision.sql"),
                &[&project, &item, &version.to_string()],
            )
            .await?;
        let mut rows = rows.into_iter();
        let row = rows.next().ok_or(NativeError::Integrity)?;
        if rows.next().is_some() {
            return Err(NativeError::Integrity);
        }
        stored(row, project, item, version)
    }

    pub(super) async fn verify_operational_head(
        &mut self,
        project: &Project,
        creation: i64,
    ) -> Result<()> {
        let id = &project.id.0;
        let rows = self.rows("SELECT h.state_version::numeric(20,0)::text,h.operation_row_id, \
          r.item_id::text,r.item_version::numeric(20,0)::text, \
          COALESCE(o.actor_id=p.created_by AND o.command_family='project_create' \
            AND o.target_kind='project' AND o.target_project_id=p.project_id AND o.target_item_id IS NULL \
            AND o.target_user_id IS NULL AND o.target_relationship_id IS NULL \
            AND o.required_grant='project_create' AND o.grant_policy_version=1,false), \
          latest.project_state_version::numeric(20,0)::text,latest.operation_row_id \
          FROM kehila.project_operational_heads h JOIN kehila.projects p USING(project_id) \
          LEFT JOIN kehila.operations o ON o.operation_row_id=h.operation_row_id \
          LEFT JOIN kehila.item_revisions r ON r.project_id=h.project_id \
            AND r.project_state_version=h.state_version AND r.operation_row_id=h.operation_row_id \
          LEFT JOIN LATERAL (SELECT project_state_version,operation_row_id FROM kehila.item_revisions \
            WHERE project_id=h.project_id ORDER BY project_state_version DESC LIMIT 1) latest ON true \
          WHERE h.project_id=$1::text::kehila.ident LIMIT 1", &[&id]).await?;
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let row = &rows[0];
        let state = scalar(get(row, 0)?)?;
        let operation: i64 = get(row, 1)?;
        let item: Option<String> = get(row, 2)?;
        let version: Option<String> = get(row, 3)?;
        let latest_state: Option<String> = get(row, 5)?;
        let latest_operation: Option<i64> = get(row, 6)?;
        if state == 0 {
            if operation != creation
                || !get::<bool>(row, 4)?
                || item.is_some()
                || version.is_some()
                || latest_state.is_some()
                || latest_operation.is_some()
                || project.next_sequence != 1
                || project.ever_estimated
            {
                return Err(NativeError::Integrity);
            }
            return Ok(());
        }
        let item = item.ok_or(NativeError::Integrity)?;
        let version = scalar(version.ok_or(NativeError::Integrity)?)?;
        if latest_state.as_deref() != Some(state.to_string().as_str())
            || latest_operation != Some(operation)
        {
            return Err(NativeError::Integrity);
        }
        drop(rows);
        let head = self.item_revision(id, &item, version).await?;
        head.bind_project(project)?;
        if head.operation != operation
            || head.revision.project_state_version != state
            || head.project().next_sequence != project.next_sequence
            || head.project().ever_estimated != project.ever_estimated
        {
            return Err(NativeError::Integrity);
        }
        Ok(())
    }

    pub(super) async fn current_item(
        &mut self,
        current_project: &Project,
        item: &str,
    ) -> Result<Item> {
        let project = current_project.id.0.as_str();
        let rows = self.rows("SELECT current_version::numeric(20,0)::text,issued_prefix::text, \
          issued_sequence::numeric(20,0)::text,creation_operation_row_id, \
          (SELECT r.item_version::numeric(20,0)::text FROM kehila.item_revisions r \
            WHERE r.project_id=i.project_id AND r.item_id=i.item_id ORDER BY r.item_version DESC LIMIT 1) \
          FROM kehila.items i WHERE project_id=$1::text::kehila.ident AND item_id=$2::text::kehila.ident LIMIT 1", &[&project, &item]).await?;
        if rows.is_empty() {
            return Err(NativeError::Rejected(Failure::Absent));
        }
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let version = scalar(get(&rows[0], 0)?)?;
        let prefix: String = get(&rows[0], 1)?;
        let sequence = scalar(get(&rows[0], 2)?)?;
        let creation: i64 = get(&rows[0], 3)?;
        if version == 0
            || get::<Option<String>>(&rows[0], 4)?.as_deref() != Some(version.to_string().as_str())
        {
            return Err(NativeError::Integrity);
        }
        drop(rows);
        let first = self.item_revision(project, item, 1).await?;
        first.bind_project(current_project)?;
        if first.operation != creation {
            return Err(NativeError::Integrity);
        }
        let SavedResult::Create(first) = first.revision.result else {
            return Err(NativeError::Integrity);
        };
        let readable = first.readable_id.clone();
        if readable.project_id.0 != project
            || readable.item_id.0 != item
            || readable.prefix != prefix
            || readable.sequence != sequence
            || sequence == 0
        {
            return Err(NativeError::Integrity);
        }
        if first.project.prefix != readable.prefix
            || Some(first.project.next_sequence) != readable.sequence.checked_add(1)
        {
            return Err(NativeError::Integrity);
        }
        if version == 1 {
            return Ok(Item {
                result: SavedResult::Create(first),
                readable,
            });
        }
        let original_item = first.item.clone();
        drop(first);
        let current = self.item_revision(project, item, version).await?;
        current.bind_project(current_project)?;
        if current.item().version != version {
            return Err(NativeError::Integrity);
        }
        let SavedResult::Edit(current) = current.revision.result else {
            return Err(NativeError::Integrity);
        };
        if current.item.type_id != original_item.type_id
            || current.item.workflow_id != original_item.workflow_id
            || current.item.status_id != original_item.status_id
            || current.item.archived != original_item.archived
        {
            return Err(NativeError::Integrity);
        }
        Ok(Item {
            result: SavedResult::Edit(current),
            readable,
        })
    }
}
