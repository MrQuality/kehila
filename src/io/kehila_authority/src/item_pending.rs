//! Immutable creation intent admission; this module never executes an item.
pub(crate) mod ingress;
use crate::coordinator::Error;
use std::collections::{HashSet, VecDeque};
use task_contract::{
    field::FieldEntry,
    item_mutation::CreateCommand,
    project::Estimate,
    work_item::{ProjectId, WorkItemId, WorkItemTypeId, WorkflowId},
};
use work_item_codec::{NativeRecord, Request};

// Provisional collision containment, not a measured capacity or retry target.
pub(crate) const REGISTRATION_ATTEMPTS: usize = 3;
pub(crate) type Identities = [[String; 3]; REGISTRATION_ATTEMPTS];

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Form {
    project: ProjectId,
    kind: WorkItemTypeId,
    workflow: Option<WorkflowId>,
    configuration: u64,
    estimate: Option<Estimate>,
    values: Vec<FieldEntry>,
}

/// Only complete native admission constructs this write capability. Alternative
/// identities are small; there is never an eager batch of large command copies.
pub(crate) struct Registration {
    project: ProjectId,
    command: CreateCommand,
    handle: String,
    bytes: NativeRecord,
    alternatives: VecDeque<[String; 3]>,
}

pub(crate) enum Command {
    Write(Write),
    Inspect(Inspection),
}
pub(crate) enum Write {
    Register(Box<Registration>),
    Cancel { project: String, handle: String },
}
pub(crate) enum Inspection {
    List {
        project: String,
        before: Option<i64>,
    },
    Open {
        project: String,
        handle: String,
    },
}

impl Form {
    pub(crate) fn prepare(self, identities: Identities) -> Result<Registration, Error> {
        let mut distinct = HashSet::new();
        for key in identities.iter().flatten() {
            if !ingress::key(key) || !distinct.insert(key.as_str()) {
                return Err(Error::Unavailable);
            }
        }
        drop(distinct);
        let mut alternatives = VecDeque::from(Vec::from(identities));
        let [handle, token, target] = alternatives.pop_front().ok_or(Error::Unavailable)?;
        Registration::admit(
            self.project,
            CreateCommand {
                operation_id: token,
                item_id: WorkItemId(target),
                type_id: self.kind,
                workflow_id: self.workflow,
                expected_configuration_revision: self.configuration,
                estimate: self.estimate,
                values: self.values,
            },
            handle,
            alternatives,
        )
    }
}

impl Registration {
    fn admit(
        project: ProjectId,
        command: CreateCommand,
        handle: String,
        alternatives: VecDeque<[String; 3]>,
    ) -> Result<Self, Error> {
        let request = Request::Create {
            project_id: project,
            command,
        };
        let bytes = work_item_codec::admit_request(&request).map_err(|_| Error::Invalid)?;
        let Request::Create {
            project_id: project,
            command,
        } = request
        else {
            return Err(Error::Unavailable);
        };
        Ok(Self {
            project,
            command,
            handle,
            bytes,
            alternatives,
        })
    }

    pub(crate) fn project(&self) -> &str {
        &self.project.0
    }
    pub(crate) fn handle(&self) -> &str {
        &self.handle
    }
    pub(crate) fn token(&self) -> &str {
        &self.command.operation_id
    }
    pub(crate) fn target(&self) -> &str {
        &self.command.item_id.0
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        self.bytes.as_bytes()
    }

    // The execution owner calls this only after confirmed full rollback and
    // database-guard release. A lost COMMIT never returns this capability here.
    pub(crate) fn next(self) -> Result<Option<Self>, Error> {
        let Self {
            project,
            mut command,
            handle: _,
            bytes,
            mut alternatives,
        } = self;
        drop(bytes);
        let Some([handle, token, target]) = alternatives.pop_front() else {
            return Ok(None);
        };
        command.operation_id = token;
        command.item_id = WorkItemId(target);
        Self::admit(project, command, handle, alternatives).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn identities() -> Identities {
        std::array::from_fn(|attempt| {
            std::array::from_fn(|part| format!("{:032x}", attempt * 3 + part + 1))
        })
    }
    fn form(value: Value) -> Form {
        let ingress::Input::Register(form) = ingress::decode(
            ingress::Route::Register,
            &serde_json::to_vec(&value).unwrap(),
        )
        .unwrap() else {
            panic!()
        };
        form
    }
    fn value() -> Value {
        json!({"project_id":"p","type_id":"task","workflow_id":null,
            "expected_configuration_revision":"1","estimate":"0",
            "values":[{"id":"title","value":"exact title"}]})
    }
    #[test]
    fn collision_candidates_change_only_uncommitted_server_identity() {
        let first = form(value()).prepare(identities()).unwrap();
        let request = work_item_codec::verify_request(
            1,
            first.bytes(),
            &work_item_codec::fingerprint(first.bytes()),
        )
        .unwrap();
        let Request::Create {
            project_id,
            command,
        } = request
        else {
            panic!()
        };
        let first_handle = first.handle().to_owned();
        let first_target = first.target().to_owned();
        let second = first.next().unwrap().unwrap();
        assert_ne!(second.handle(), first_handle);
        assert_ne!(second.target(), first_target);
        let Request::Create {
            project_id: other,
            command: second_command,
        } = work_item_codec::decode_request(1, second.bytes()).unwrap()
        else {
            panic!()
        };
        assert_eq!(project_id, other);
        assert_eq!(command.values, second_command.values);
        assert_eq!(command.estimate, second_command.estimate);
        assert_eq!(command.type_id, second_command.type_id);
        assert_eq!(command.workflow_id, second_command.workflow_id);
        assert_eq!(
            command.expected_configuration_revision,
            second_command.expected_configuration_revision
        );
        assert_ne!(command.operation_id, second_command.operation_id);
        let third = second.next().unwrap().unwrap();
        assert!(third.next().unwrap().is_none());
    }
    #[test]
    fn repeated_generated_keys_fail_before_an_admitted_registration_exists() {
        let keys = std::array::from_fn(|_| std::array::from_fn(|_| "0".repeat(32)));
        assert!(matches!(
            form(value()).prepare(keys),
            Err(Error::Unavailable)
        ));
    }
    #[test]
    fn generated_request_expansion_can_reject_an_original_admitted_form() {
        let mut value = value();
        value["values"] = Value::Array(
            (0..16)
                .map(|i| {
                    json!({"id":format!("field-{i}"),
            "value":"x".repeat(project_create_codec::MAX_STRING_BYTES)})
                })
                .collect(),
        );
        let excess = serde_json::to_vec(&value).unwrap().len()
            - (project_create_codec::MAX_RECORD_BYTES - 100);
        value["values"][15]["value"] =
            Value::String("x".repeat(project_create_codec::MAX_STRING_BYTES - excess));
        assert_eq!(
            serde_json::to_vec(&value).unwrap().len(),
            project_create_codec::MAX_RECORD_BYTES - 100
        );
        assert!(matches!(
            form(value).prepare(identities()),
            Err(Error::Invalid)
        ));
    }
}
