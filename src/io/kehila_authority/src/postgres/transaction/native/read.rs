use super::*;
use project_create_codec::{
    verify_revision_snapshots, MAX_COLLECTION_ENTRIES, MAX_NODES, MAX_RECORD_BYTES,
};
use std::collections::BTreeMap;
use task_contract::{
    field::{FieldDefinition, FieldId, FieldKind, FieldOrigin, FieldUsage},
    project::Project,
    work_item::{
        Configuration, Phase, PhaseChange, ProjectId, Status, StatusId, WorkItemType,
        WorkItemTypeId, Workflow, WorkflowId,
    },
};

// SQL carries exact pre-materialization sentinels. A codec ceiling change must
// update those queries deliberately rather than silently truncating a reload.
const _: () =
    assert!(MAX_COLLECTION_ENTRIES == 1024 && MAX_NODES == 16384 && MAX_RECORD_BYTES == 1048576);

pub(crate) struct Current {
    pub(crate) project: Project,
    pub(crate) configuration: Configuration,
    pub(crate) creation_operation: i64,
}

fn phase(value: String) -> Result<Phase> {
    match value.as_str() {
        "new" => Ok(Phase::New),
        "active" => Ok(Phase::Active),
        "done" => Ok(Phase::Done),
        _ => Err(NativeError::Integrity),
    }
}
fn position(row: &Row, index: usize, column: usize) -> Result<()> {
    let value: i16 = get(row, column)?;
    if usize::try_from(value).ok() != Some(index) {
        Err(NativeError::Integrity)
    } else {
        Ok(())
    }
}
fn text(row: &Row, index: usize) -> Result<Vec<u8>> {
    let value: Option<String> = get(row, index)?;
    let value = value.ok_or(NativeError::Integrity)?;
    if value.len() > MAX_RECORD_BYTES {
        return Err(NativeError::Integrity);
    }
    Ok(value.into_bytes())
}

impl<P> Fenced<'_, P> {
    pub(super) async fn revision_pair(
        &mut self,
        id: &str,
        revision: u64,
        operation: i64,
    ) -> Result<(Project, Configuration)> {
        let revision_text = revision.to_string();
        let rows=self.rows("SELECT snapshot_codec_version, \
          CASE WHEN octet_length(project_snapshot::text)<=1048576 THEN project_snapshot::text END, \
          CASE WHEN octet_length(configuration_snapshot::text)<=1048576 THEN configuration_snapshot::text END, \
          actor_id::text,operation_row_id,command_family,target_kind \
          FROM kehila.configuration_revisions WHERE project_id=$1::text::kehila.ident \
          AND revision=$2::text::kehila.u64 LIMIT 1",&[&id,&revision_text]).await?;
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let row = &rows[0];
        let version: i32 = get(row, 0)?;
        let actor: String = get(row, 3)?;
        let recorded_operation: i64 = get(row, 4)?;
        let family: String = get(row, 5)?;
        let kind: String = get(row, 6)?;
        if actor != self.authority.owner
            || recorded_operation != operation
            || family != "project_create"
            || kind != "project"
        {
            return Err(NativeError::Integrity);
        }
        let version = u32::try_from(version).map_err(|_| NativeError::Integrity)?;
        verify_revision_snapshots(
            &ProjectId(id.to_owned()),
            revision,
            version,
            &text(row, 1)?,
            &text(row, 2)?,
        )
        .map_err(|_| NativeError::Integrity)
    }

    async fn current_project(&mut self, id: &str) -> Result<Current> {
        let actor = self.authority.owner.clone();
        let grants=self.rows("SELECT g.access_profile,g.active,g.version::text, \
          e.access_profile,e.active,e.version::text,e.granted_by::text,e.granting_operation_row_id,e.command_family,e.target_kind \
          FROM kehila.project_grants g LEFT JOIN LATERAL ( \
            SELECT * FROM kehila.project_grant_events event WHERE event.project_id=g.project_id AND event.actor_id=g.actor_id \
            ORDER BY event.version DESC LIMIT 1) e ON true \
          WHERE g.project_id=$1::text::kehila.ident AND g.actor_id=$2::text::kehila.ident LIMIT 1",&[&id,&actor]).await?;
        if grants.is_empty() {
            return Err(NativeError::Rejected(Failure::Absent));
        }
        if grants.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let grant = &grants[0];
        let active: bool = get(grant, 1)?;
        if !active {
            return Err(NativeError::Rejected(Failure::Forbidden));
        }
        let profile: String = get(grant, 0)?;
        let version = scalar(get(grant, 2)?)?;
        if profile != "initial_owner"
            || version == 0
            || get::<Option<String>>(grant, 3)?.as_deref() != Some(profile.as_str())
            || get::<Option<bool>>(grant, 4)? != Some(true)
            || get::<Option<String>>(grant, 5)?.as_deref() != Some(version.to_string().as_str())
            || get::<Option<String>>(grant, 6)?.as_deref() != Some(actor.as_str())
            || get::<Option<String>>(grant, 8)?.as_deref() != Some("project_create")
            || get::<Option<String>>(grant, 9)?.as_deref() != Some("project")
        {
            return Err(NativeError::Integrity);
        }
        let granting_operation: i64 =
            get::<Option<i64>>(grant, 7)?.ok_or(NativeError::Integrity)?;
        let projects=self.rows("SELECT project_id::text,name,current_prefix::text,estimate_unit,configuration_revision::text,\
          next_sequence::text,ever_estimated,archived,created_by::text,creation_operation_row_id,\
          creation_command_family,creation_target_kind,seed_profile \
          FROM kehila.projects WHERE project_id=$1::text::kehila.ident LIMIT 1",&[&id]).await?;
        if projects.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let row = &projects[0];
        let actual_id: String = get(row, 0)?;
        let unit: String = get(row, 3)?;
        let project = Project {
            id: ProjectId(actual_id),
            name: get(row, 1)?,
            prefix: get(row, 2)?,
            estimate_unit: match unit.as_str() {
                "hours" => EstimateUnit::Hours,
                "points" => EstimateUnit::Points,
                _ => return Err(NativeError::Integrity),
            },
            configuration_revision: scalar(get(row, 4)?)?,
            next_sequence: scalar(get(row, 5)?)?,
            ever_estimated: get(row, 6)?,
            archived: get(row, 7)?,
        };
        let created_by: String = get(row, 8)?;
        let creation_operation: i64 = get(row, 9)?;
        if project.id.0 != id
            || project.configuration_revision == 0
            || project.next_sequence == 0
            || created_by != actor
            || creation_operation != granting_operation
            || creation_operation <= 0
            || get::<String>(row, 10)? != "project_create"
            || get::<String>(row, 11)? != "project"
            || get::<String>(row, 12)? != "m1_v1"
        {
            return Err(NativeError::Integrity);
        }
        let (recorded_project, recorded_configuration) = self
            .revision_pair(id, project.configuration_revision, creation_operation)
            .await?;
        let status_rows=self.rows("SELECT status_id::text,name,phase::text,archived,position \
          FROM kehila.statuses WHERE project_id=$1::text::kehila.ident ORDER BY position LIMIT 1025",&[&id]).await?;
        if status_rows.len() > MAX_COLLECTION_ENTRIES {
            return Err(NativeError::Integrity);
        }
        let statuses = status_rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                position(row, i, 4)?;
                Ok(Status {
                    id: StatusId(get(row, 0)?),
                    name: get(row, 1)?,
                    phase: phase(get(row, 2)?)?,
                    archived: get(row, 3)?,
                    group_id: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        drop(status_rows);
        let member_rows = self
            .rows(
                "SELECT workflow_id::text,status_id::text,position \
          FROM kehila.workflow_statuses WHERE project_id=$1::text::kehila.ident \
          ORDER BY workflow_id COLLATE \"C\",position LIMIT 16385",
                &[&id],
            )
            .await?;
        if member_rows.len() > MAX_NODES {
            return Err(NativeError::Integrity);
        }
        let mut members: BTreeMap<String, Vec<StatusId>> = BTreeMap::new();
        for row in member_rows {
            let parent: String = get(&row, 0)?;
            let list = members.entry(parent).or_default();
            position(&row, list.len(), 2)?;
            list.push(StatusId(get(&row, 1)?));
            if list.len() > MAX_COLLECTION_ENTRIES {
                return Err(NativeError::Integrity);
            }
        }
        let changes = self
            .rows(
                "SELECT workflow_id::text,from_phase::text,to_phase::text,position \
          FROM kehila.workflow_phase_changes WHERE project_id=$1::text::kehila.ident \
          ORDER BY workflow_id COLLATE \"C\",position LIMIT 16385",
                &[&id],
            )
            .await?;
        if changes.len() > MAX_NODES {
            return Err(NativeError::Integrity);
        }
        let mut changes_by_workflow: BTreeMap<String, Vec<PhaseChange>> = BTreeMap::new();
        for row in changes {
            let parent: String = get(&row, 0)?;
            let list = changes_by_workflow.entry(parent).or_default();
            position(&row, list.len(), 3)?;
            list.push(PhaseChange::new(
                phase(get(&row, 1)?)?,
                phase(get(&row, 2)?)?,
            ));
            if list.len() > MAX_COLLECTION_ENTRIES {
                return Err(NativeError::Integrity);
            }
        }
        let workflows=self.rows("SELECT workflow_id::text,initial_status_id::text,initial_phase::text,archived,position \
          FROM kehila.workflows WHERE project_id=$1::text::kehila.ident ORDER BY position LIMIT 1025",&[&id]).await?;
        if workflows.len() > MAX_COLLECTION_ENTRIES {
            return Err(NativeError::Integrity);
        }
        let mut typed_workflows = Vec::with_capacity(workflows.len());
        for (i, row) in workflows.iter().enumerate() {
            position(row, i, 4)?;
            if get::<String>(row, 2)? != "new" {
                return Err(NativeError::Integrity);
            }
            let workflow: String = get(row, 0)?;
            typed_workflows.push(Workflow {
                id: WorkflowId(workflow.clone()),
                initial_status_id: StatusId(get(row, 1)?),
                archived: get(row, 3)?,
                status_ids: members.remove(&workflow).unwrap_or_default(),
                permitted_phase_changes: changes_by_workflow.remove(&workflow).unwrap_or_default(),
            });
        }
        if !members.is_empty() || !changes_by_workflow.is_empty() {
            return Err(NativeError::Integrity);
        }
        let permitted = self
            .rows(
                "SELECT type_id::text,workflow_id::text,position \
          FROM kehila.type_workflows WHERE project_id=$1::text::kehila.ident \
          ORDER BY type_id COLLATE \"C\",position LIMIT 16385",
                &[&id],
            )
            .await?;
        if permitted.len() > MAX_NODES {
            return Err(NativeError::Integrity);
        }
        let mut by_type: BTreeMap<String, Vec<WorkflowId>> = BTreeMap::new();
        for row in permitted {
            let parent: String = get(&row, 0)?;
            let list = by_type.entry(parent).or_default();
            position(&row, list.len(), 2)?;
            list.push(WorkflowId(get(&row, 1)?));
            if list.len() > MAX_COLLECTION_ENTRIES {
                return Err(NativeError::Integrity);
            }
        }
        let types=self.rows("SELECT type_id::text,default_workflow_id::text,title_field_id::text,archived,position \
          FROM kehila.work_item_types WHERE project_id=$1::text::kehila.ident ORDER BY position LIMIT 1025",&[&id]).await?;
        if types.len() > MAX_COLLECTION_ENTRIES {
            return Err(NativeError::Integrity);
        }
        let mut typed_types = Vec::with_capacity(types.len());
        for (i, row) in types.iter().enumerate() {
            position(row, i, 4)?;
            let kind: String = get(row, 0)?;
            typed_types.push(WorkItemType {
                id: WorkItemTypeId(kind.clone()),
                default_workflow_id: WorkflowId(get(row, 1)?),
                title_field_id: get::<Option<String>>(row, 2)?.map(FieldId),
                archived: get(row, 3)?,
                permitted_workflows: by_type.remove(&kind).unwrap_or_default(),
            });
        }
        if !by_type.is_empty() {
            return Err(NativeError::Integrity);
        }
        let field_rows=self.rows("SELECT field_id::text,owner_type_id::text,name,value_kind,origin,usage,archived,position \
          FROM kehila.fields WHERE project_id=$1::text::kehila.ident ORDER BY position LIMIT 1025",&[&id]).await?;
        if field_rows.len() > MAX_COLLECTION_ENTRIES {
            return Err(NativeError::Integrity);
        }
        let fields = field_rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                position(row, i, 7)?;
                if get::<String>(row, 3)? != "text"
                    || get::<String>(row, 4)? != "application"
                    || get::<String>(row, 5)? != "optional"
                    || get::<bool>(row, 6)?
                {
                    return Err(NativeError::Integrity);
                }
                Ok(FieldDefinition {
                    id: FieldId(get(row, 0)?),
                    owner_type: WorkItemTypeId(get(row, 1)?),
                    name: get(row, 2)?,
                    kind: FieldKind::Text,
                    origin: FieldOrigin::Application,
                    usage: FieldUsage::Optional,
                    archived: false,
                    options: vec![],
                })
            })
            .collect::<Result<Vec<_>>>()?;
        drop(field_rows);
        let configuration = Configuration {
            project_id: project.id.clone(),
            revision: project.configuration_revision,
            project_archived: project.archived,
            statuses,
            status_groups: vec![],
            workflows: typed_workflows,
            types: typed_types,
            fields,
            relationship_types: vec![],
        };
        let mut stable = project.clone();
        stable.next_sequence = recorded_project.next_sequence;
        stable.ever_estimated = recorded_project.ever_estimated;
        if stable != recorded_project || configuration != recorded_configuration {
            return Err(NativeError::Integrity);
        }
        // Free the historical pair and relational driver buffers before loading
        // the one indexed operational history. Its mutable counters are proved
        // independently; creation snapshots and historical replay stay frozen.
        drop(recorded_configuration);
        drop(recorded_project);
        drop(stable);
        drop(types);
        drop(workflows);
        drop(projects);
        drop(grants);
        self.verify_operational_head(&project, creation_operation)
            .await?;
        Ok(Current {
            project,
            configuration,
            creation_operation,
        })
    }
}

impl Fenced<'_, SessionProof> {
    pub(crate) async fn reload_created(&mut self, id: &str) -> Result<Current> {
        self.current_project(id).await
    }
}
impl Fenced<'_, ReadProof> {
    pub(crate) async fn read_project(&mut self, id: &str) -> Result<Current> {
        self.current_project(id).await
    }
}
