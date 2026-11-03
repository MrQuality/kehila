//! Prepared native commands. Preparation performs no I/O or authorization.
pub(crate) mod wire;
use crate::{coordinator::Error, native_ingress::Form};
use project_create_codec::{
    encode_native_result, encode_native_revision_snapshots, fingerprint, NativeRequest,
    NativeRevisionSnapshots,
};
use std::collections::VecDeque;
use task_contract::{
    field::{FieldKind, FieldOrigin, FieldUsage},
    work_item::Phase,
};
use task_contract::{
    project::EstimateUnit,
    project_create::{
        decide_project_create, validate_project_create_form, ProjectCreateCommand,
        ProjectCreateDecision, ProjectCreateResult,
    },
    work_item::ProjectId,
};

pub(crate) struct Prepared {
    request: NativeRequest,
    request_bytes: Vec<u8>,
    digest: [u8; 32],
    result: ProjectCreateResult,
    result_bytes: Vec<u8>,
    snapshots: NativeRevisionSnapshots,
}

pub(crate) struct Candidates(VecDeque<Prepared>);

impl Candidates {
    pub(crate) fn new(form: Form, token: String, targets: [String; 3]) -> Result<Self, Error> {
        Self::build(form, token, targets, Prepared::new)
    }
    #[cfg(test)]
    pub(crate) fn seed_overflow_for_test(
        form: Form,
        token: String,
        targets: [String; 3],
    ) -> Result<Self, Error> {
        Self::build(form, token, targets, Prepared::seed_overflow_for_test)
    }
    fn build(
        form: Form,
        token: String,
        targets: [String; 3],
        mut make: impl FnMut(Form, String, String) -> Result<Prepared, Error>,
    ) -> Result<Self, Error> {
        if targets[0] == targets[1] || targets[0] == targets[2] || targets[1] == targets[2] {
            return Err(Error::Unavailable); // Bounded outcome; no generate-until-distinct loop.
        }
        let mut admitted = VecDeque::with_capacity(3);
        for target in targets {
            admitted.push_back(make(form.clone(), target, token.clone())?);
        }
        Ok(Self(admitted))
    }

    // This consumes an immutable complete admission proof. The execution owner
    // must first confirm rollback/release, then check live/deadline, and only
    // afterwards acquire new DB admission/fences for this distinct candidate.
    pub(crate) fn next_admitted(&mut self) -> Option<Prepared> {
        self.0.pop_front()
    }
}

pub(crate) enum Command {
    Binding,
    Register {
        form: Form,
        handle: String,
        token: String,
    },
    List(Option<i64>),
    Open(i64),
    Execute {
        request_id: i64,
        candidates: Candidates,
    },
    Reconcile(i64),
    Cancel(i64),
    Read(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Failure {
    Forbidden,
    Absent,
    Conflict,
    Expired,
    Cancelled,
    OutcomeUnknown,
}

pub(crate) fn validate_form(form: &Form) -> Result<(), Error> {
    validate_project_create_form(&form.name, &form.prefix).map_err(|_| Error::Invalid)
}

pub(crate) fn unit_name(unit: EstimateUnit) -> &'static str {
    match unit {
        EstimateUnit::Hours => "hours",
        EstimateUnit::Points => "points",
    }
}

impl Prepared {
    pub(crate) fn new(form: Form, project_id: String, token: String) -> Result<Self, Error> {
        Generated::new(form, project_id, token)?.admit()
    }
    #[cfg(test)]
    fn seed_overflow_for_test(
        form: Form,
        project_id: String,
        token: String,
    ) -> Result<Self, Error> {
        let mut generated = Generated::new(form, project_id, token)?;
        let field = generated
            .result
            .configuration
            .fields
            .first()
            .cloned()
            .ok_or(Error::Unavailable)?;
        while generated.result.configuration.fields.len()
            <= project_create_codec::MAX_COLLECTION_ENTRIES
        {
            let index = generated.result.configuration.fields.len();
            let mut extra = field.clone();
            extra.id = task_contract::field::FieldId(format!("overflow-field-{index}"));
            extra.name = format!("Overflow field {index}");
            generated.result.configuration.fields.push(extra);
        }
        assert!(
            relational_shape(&generated.result).is_ok(),
            "overflow fixture must preserve relational shape"
        );
        assert!(
            matches!(
                encode_native_result(&generated.result),
                Err(project_create_codec::Error::LimitExceeded)
            ),
            "overflow fixture must reach the codec collection ceiling"
        );
        generated.admit()
    }
    pub(crate) fn command(&self) -> &ProjectCreateCommand {
        self.request.command()
    }
    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
    pub(crate) fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub(crate) fn result(&self) -> &ProjectCreateResult {
        &self.result
    }
    pub(crate) fn result_bytes(&self) -> &[u8] {
        &self.result_bytes
    }
    pub(crate) fn snapshots(&self) -> &NativeRevisionSnapshots {
        &self.snapshots
    }
}
// This private stage cannot enter a serving queue. Only complete admission
// constructs Prepared, including when a test injects a trusted-seed failure.
struct Generated {
    request: NativeRequest,
    result: ProjectCreateResult,
}
impl Generated {
    fn new(form: Form, project_id: String, token: String) -> Result<Self, Error> {
        let request = NativeRequest::new(ProjectCreateCommand {
            operation_id: token,
            project_id: ProjectId(project_id),
            name: form.name,
            prefix: form.prefix,
            estimate_unit: form.estimate_unit,
        })
        .map_err(|_| Error::Invalid)?;
        let ProjectCreateDecision::Apply(result) =
            decide_project_create(false, None, true, request.command())
        else {
            return Err(Error::Unavailable);
        };
        Ok(Self { request, result })
    }
    fn admit(self) -> Result<Prepared, Error> {
        let Self { request, result } = self;
        relational_shape(&result)?;
        // Every generated envelope is admitted before this sealed value can
        // enter the execution queue, serving mutex or database connection.
        let request_bytes = request.encode().map_err(|_| Error::Invalid)?;
        let digest = fingerprint(&request_bytes);
        let result_bytes = encode_native_result(&result).map_err(|_| Error::Unavailable)?;
        let snapshots =
            encode_native_revision_snapshots(&result).map_err(|_| Error::Unavailable)?;
        Ok(Prepared {
            request,
            request_bytes,
            digest,
            result,
            result_bytes,
            snapshots,
        })
    }
}

fn relational_shape(result: &ProjectCreateResult) -> Result<(), Error> {
    let c = &result.configuration;
    let p = &result.project;
    let id = |value: &str| !value.is_empty() && value.len() <= 128 && !value.contains('\0');
    let name = |value: &str| !value.is_empty() && value.len() <= 256 && !value.contains('\0');
    let mut valid = p.configuration_revision == 1
        && p.next_sequence == 1
        && !p.archived
        && !p.ever_estimated
        && c.revision == 1
        && !c.project_archived
        && c.project_id == p.id
        && id(&p.id.0)
        && c.status_groups.is_empty()
        && c.relationship_types.is_empty();
    valid &= c
        .statuses
        .iter()
        .all(|s| id(&s.id.0) && name(&s.name) && s.group_id.is_none());
    valid &= c.workflows.iter().all(|w| {
        id(&w.id.0)
            && id(&w.initial_status_id.0)
            && w.status_ids.iter().all(|s| id(&s.0))
            && w.permitted_phase_changes
                .iter()
                .all(|change| change.is_system_allowed())
            && c.statuses
                .iter()
                .any(|s| s.id == w.initial_status_id && s.phase == Phase::New)
    });
    valid &= c.types.iter().all(|t| {
        id(&t.id.0)
            && id(&t.default_workflow_id.0)
            && t.title_field_id.as_ref().is_none_or(|f| id(&f.0))
            && t.permitted_workflows.iter().all(|w| id(&w.0))
    });
    valid &= c.fields.iter().all(|f| {
        id(&f.id.0)
            && id(&f.owner_type.0)
            && name(&f.name)
            && f.kind == FieldKind::Text
            && f.origin == FieldOrigin::Application
            && f.usage == FieldUsage::Optional
            && !f.archived
            && f.options.is_empty()
    });
    if valid {
        Ok(())
    } else {
        Err(Error::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use project_create_codec::{decode_result, verify_request, verify_revision_snapshots};
    fn form() -> Form {
        Form {
            name: "Owner project".into(),
            prefix: "OP".into(),
            estimate_unit: EstimateUnit::Points,
        }
    }

    #[test]
    fn entire_candidate_batch_is_sealed_to_each_distinct_target() {
        let targets = ["1".repeat(32), "2".repeat(32), "3".repeat(32)];
        let mut candidates = Candidates::new(form(), "a".repeat(32), targets.clone()).unwrap();
        for target in targets {
            let p = candidates.next_admitted().unwrap();
            assert_eq!(p.command().project_id.0, target);
            assert_eq!(
                verify_request(1, p.request_bytes(), p.digest()).unwrap(),
                *p.command()
            );
            assert_eq!(decode_result(1, p.result_bytes()).unwrap(), *p.result());
            assert_eq!(
                verify_revision_snapshots(
                    &p.command().project_id,
                    1,
                    1,
                    p.snapshots().project(),
                    p.snapshots().configuration()
                )
                .unwrap(),
                (p.result().project.clone(), p.result().configuration.clone())
            );
        }
        assert!(candidates.next_admitted().is_none());
    }

    #[test]
    fn duplicated_random_targets_and_invalid_form_are_bounded_preparation_failures() {
        assert!(matches!(
            Candidates::new(
                form(),
                "a".repeat(32),
                ["1".repeat(32), "1".repeat(32), "2".repeat(32)]
            ),
            Err(Error::Unavailable)
        ));
        let mut bad = form();
        bad.name = " ".into();
        assert!(matches!(validate_form(&bad), Err(Error::Invalid)));
        assert!(matches!(
            Prepared::new(bad, "1".repeat(32), "a".repeat(32)),
            Err(Error::Invalid)
        ));
    }
}
