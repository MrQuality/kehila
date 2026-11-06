//! Sealed pure acceptance for one unseen item-write attempt.
//! Database proof construction/save/reload and replay delivery are separate.
use std::sync::Arc;
use task_contract::{
    current_work::UserId,
    item_mutation::{decide_create, decide_edit, CreateDecision, EditDecision, ItemMutationError},
    project::{Project, ReadableId},
    work_item::Configuration,
};
use work_item_codec::{NativeRecord, Request, Revision, SavedResult};
#[derive(Clone, Debug, Eq, PartialEq)]
struct Scope {
    installation: String,
    actor: String,
}
#[derive(Debug)]
pub(crate) enum PreparationError {
    InvalidProof,
    StateExhausted,
    Expired,
    Unavailable,
    Domain(ItemMutationError),
    Representation(project_create_codec::Error),
}
impl From<project_create_codec::Error> for PreparationError {
    fn from(error: project_create_codec::Error) -> Self {
        Self::Representation(error)
    }
}
type Result<T> = std::result::Result<T, PreparationError>;
#[derive(Debug)]
pub(crate) struct Canonical {
    scope: Scope,
    request: Request,
    bytes: NativeRecord,
    digest: [u8; 32],
}
impl Canonical {
    pub(crate) fn project_id(&self) -> &str {
        match &self.request {
            Request::Create { project_id, .. } | Request::Edit { project_id, .. } => &project_id.0,
        }
    }
    // Identity binding only; this constructor grants no database authority.
    pub(crate) fn admit(installation: &str, actor: &str, request: Request) -> Result<Self> {
        Self::new(
            Scope {
                installation: installation.into(),
                actor: actor.into(),
            },
            request,
        )
    }
    fn new(scope: Scope, request: Request) -> Result<Self> {
        if !valid_id(&scope.installation) || !valid_id(&scope.actor) {
            return Err(PreparationError::InvalidProof);
        }
        let bytes = work_item_codec::admit_request(&request)?;
        let digest = work_item_codec::fingerprint(bytes.as_bytes());
        Ok(Self {
            scope,
            request,
            bytes,
            digest,
        })
    }
    fn bytes(&self) -> &[u8] {
        self.bytes.as_bytes()
    }
}
// Only the owning database adapter will construct these fields in production.
// A typed value alone is not evidence of current grants or database coherence.
#[derive(Debug)]
struct Previous {
    operation: i64,
    revision: Revision,
    readable: ReadableId,
}
#[derive(Debug)]
struct Observed {
    scope: Scope,
    project: Project,
    configuration: Configuration,
    configuration_project: Project,
    configuration_operation: i64,
    state_version: u64,
    state_operation: i64,
    existing: Option<Previous>,
}
fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.contains('\0')
}
fn project_from(result: &SavedResult) -> &Project {
    match result {
        SavedResult::Create(value) => &value.project,
        SavedResult::Edit(value) => &value.project,
    }
}

impl Observed {
    fn check(&self, command: &Canonical) -> Result<()> {
        self.verify(&command.scope, &command.request)
    }
    fn verify(&self, scope: &Scope, request: &Request) -> Result<()> {
        let project = match request {
            Request::Create { project_id, .. } | Request::Edit { project_id, .. } => project_id,
        };
        if self.scope != *scope
            || self.project.id != *project
            || self.configuration.project_id != *project
            || self.configuration.revision != self.project.configuration_revision
            || super::item_read::historical_project(&self.configuration_project, &self.project)
                .is_err()
            || self.configuration_operation <= 0
            || self.state_operation <= 0
            || self.project.next_sequence == 0
            || self.configuration.project_archived != self.project.archived
        {
            return Err(PreparationError::InvalidProof);
        }
        if let Some(previous) = &self.existing {
            let revision = &previous.revision;
            if previous.operation <= 0
                || !task_contract::project::valid_prefix(&previous.readable.prefix)
                || previous.readable.sequence == 0
                || previous.readable.sequence >= self.project.next_sequence
                || revision.actor_id.0 != self.scope.actor
                || revision.project_state_version == 0
                || revision.project_state_version > self.state_version
                || revision.previous_project_operation == 0
                || revision.configuration_revision
                    != project_from(&revision.result).configuration_revision
                || super::item_read::historical_project(
                    project_from(&revision.result),
                    &self.project,
                )
                .is_err()
            {
                return Err(PreparationError::InvalidProof);
            }
            if let SavedResult::Create(created) = &revision.result {
                if created.readable_id != previous.readable {
                    return Err(PreparationError::InvalidProof);
                }
            }
        }
        match (request, &self.existing) {
            (Request::Create { .. }, None) => (),
            (Request::Edit { item_id, .. }, Some(Previous { revision, .. })) => {
                let item = match &revision.result {
                    SavedResult::Create(value) => &value.item,
                    SavedResult::Edit(value) => &value.item,
                };
                if item.id != *item_id || item.project_id != *project {
                    return Err(PreparationError::InvalidProof);
                }
            }
            _ => return Err(PreparationError::InvalidProof),
        }
        Ok(())
    }
}
fn native_project(project: &Project) -> Result<Vec<u8>> {
    let value = serde_json::json!({"codec_version":1,"kind":"project_snapshot","value":project_create_codec::composition::project_value(project)?});
    Ok(project_create_codec::composition::encode_native_record(
        &value,
    )?)
}
fn native_configuration(configuration: &Configuration) -> Result<Vec<u8>> {
    let bytes = project_create_codec::encode_snapshot(configuration)?;
    let value = project_create_codec::parse_json(&bytes)?;
    drop(bytes);
    Ok(project_create_codec::composition::encode_native_record(
        &value,
    )?)
}
// Not Clone: consumed by exactly one future protected save attempt. Reprepare
// must drop this proof/result/history while retaining only Canonical identity.
#[derive(Debug)]
struct Accepted {
    command: Arc<Canonical>,
    observed: Observed,
    revision: Revision,
    result_bytes: NativeRecord,
    history: NativeRecord,
    project_bytes: Vec<u8>,
    configuration_bytes: Vec<u8>,
    configuration_project_bytes: Vec<u8>,
    existing_bytes: Option<NativeRecord>,
}
impl Accepted {
    fn new(command: Arc<Canonical>, observed: Observed) -> Result<Self> {
        observed.check(&command)?;
        let state = observed
            .state_version
            .checked_add(1)
            .ok_or(PreparationError::StateExhausted)?;
        let result = match &command.request {
            Request::Create { command, .. } => match decide_create(
                &observed.project,
                &observed.configuration,
                None,
                true,
                command,
            ) {
                CreateDecision::Apply(result) => SavedResult::Create(result),
                CreateDecision::Reject(error) => return Err(PreparationError::Domain(error)),
                CreateDecision::Replay(_) => return Err(PreparationError::InvalidProof),
            },
            Request::Edit { command, .. } => {
                let previous = &observed
                    .existing
                    .as_ref()
                    .ok_or(PreparationError::InvalidProof)?
                    .revision;
                let (item, data) = match &previous.result {
                    SavedResult::Create(value) => (&value.item, &value.data),
                    SavedResult::Edit(value) => (&value.item, &value.data),
                };
                match decide_edit(
                    &observed.project,
                    &observed.configuration,
                    item,
                    data,
                    None,
                    true,
                    command,
                ) {
                    EditDecision::Apply(result) => SavedResult::Edit(result),
                    EditDecision::Reject(error) => return Err(PreparationError::Domain(error)),
                    EditDecision::Replay(_) => return Err(PreparationError::InvalidProof),
                }
            }
        };
        // Retain the already admitted native result. The write path must never
        // regenerate the accepted domain result after persistence begins. This does
        // not admit an outer transport envelope or authorize historical delivery.
        let result_bytes = work_item_codec::admit_result(&result)?;
        let revision = Revision {
            actor_id: UserId(observed.scope.actor.clone()),
            configuration_revision: observed.configuration.revision,
            project_state_version: state,
            previous_project_operation: u64::try_from(observed.state_operation)
                .map_err(|_| PreparationError::InvalidProof)?,
            result,
        };
        let history = work_item_codec::admit_revision(&revision)?;
        let project_bytes = native_project(&observed.project)?;
        let configuration_bytes = native_configuration(&observed.configuration)?;
        let configuration_project_bytes = native_project(&observed.configuration_project)?;
        let existing_bytes = observed
            .existing
            .as_ref()
            .map(|previous| work_item_codec::admit_revision(&previous.revision))
            .transpose()?;
        let accepted = Self {
            command,
            observed,
            revision,
            result_bytes,
            history,
            project_bytes,
            configuration_bytes,
            configuration_project_bytes,
            existing_bytes,
        };
        // Seven individually admitted records at most. This checked retained-byte
        // bound excludes typed values, encoding temporaries and driver/framing copies.
        let total = accepted
            .retained_record_bytes()
            .ok_or(PreparationError::Representation(
                project_create_codec::Error::LimitExceeded,
            ))?;
        if total > 7 * project_create_codec::MAX_RECORD_BYTES {
            return Err(PreparationError::Representation(
                project_create_codec::Error::LimitExceeded,
            ));
        }
        Ok(accepted)
    }
    fn revision(&self) -> &Revision {
        &self.revision
    }
    fn request_bytes(&self) -> &[u8] {
        self.command.bytes()
    }
    fn result_bytes(&self) -> &[u8] {
        self.result_bytes.as_bytes()
    }
    fn into_result(self) -> NativeRecord {
        let Self { result_bytes, .. } = self;
        result_bytes
    }
    fn history_bytes(&self) -> &[u8] {
        self.history.as_bytes()
    }
    fn retained_record_bytes(&self) -> Option<usize> {
        [
            self.request_bytes().len(),
            self.result_bytes().len(),
            self.history_bytes().len(),
            self.project_bytes.len(),
            self.configuration_bytes.len(),
            self.configuration_project_bytes.len(),
            self.existing_bytes
                .as_ref()
                .map_or(0, |value| value.as_bytes().len()),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
    }
}
mod factory;
pub(crate) use factory::{
    Admitted, Disposition, Draft, HistoricalAccepted, HistoricalDraft, HistoryAuthorization,
    HistoryPreparation, HistoryRelease, HistoryReleaseFailure, Preparation, ReplayRequest,
    Resolved, RetryIdentity, SaveOutcome, WriteConflict, WriteFailure,
};
#[cfg(test)]
mod tests;
