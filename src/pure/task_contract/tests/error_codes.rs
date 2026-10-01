use task_contract::archive::ArchiveCommandError;
use task_contract::configuration_change::{ConfigurationChangeError, HistoricalReference};
use task_contract::conversion::ConversionError;
use task_contract::current_work::SelectionError;
use task_contract::error_code::{Code, StableCode};
use task_contract::field::{FieldError, FieldId};
use task_contract::field_admin::OptionAdminError;
use task_contract::item_mutation::ItemMutationError;
use task_contract::project::ProjectError;
use task_contract::project_admin::ProjectMetadataError;
use task_contract::relationship::RelationshipError;
use task_contract::relationship_command::RelationshipCreateError;
use task_contract::work_item::{Error, StatusId};

#[test]
fn shared_conflicts_have_the_same_stable_code_across_commands() {
    assert_eq!(
        ConfigurationChangeError::OperationIdReused.code(),
        Code::OperationIdReused
    );
    assert_eq!(
        SelectionError::OperationIdReused.code(),
        Code::OperationIdReused
    );
    assert_eq!(
        SelectionError::InvalidOperation.code(),
        Code::InvalidOperation
    );
    assert_eq!(
        Error::ConfigurationConflict {
            current_revision: 2
        }
        .code(),
        Code::ConfigurationConflict
    );
    assert_eq!(
        ItemMutationError::ConfigurationConflict {
            current_revision: 2
        }
        .code(),
        Code::ConfigurationConflict
    );
    assert_eq!(
        ArchiveCommandError::ConfigurationConflict {
            current_revision: 2
        }
        .code(),
        Code::ConfigurationConflict
    );
    assert_eq!(
        OptionAdminError::ConfigurationConflict {
            current_revision: 2
        }
        .code(),
        Code::ConfigurationConflict
    );
    assert_eq!(
        Code::ConfigurationConflict.as_str(),
        "configuration_conflict"
    );
}

#[test]
fn nested_field_and_project_errors_keep_specific_codes() {
    assert_eq!(
        ItemMutationError::PayloadLimitExceeded.code(),
        Code::PayloadLimitExceeded
    );
    assert_eq!(
        ConversionError::PayloadLimitExceeded.code(),
        Code::PayloadLimitExceeded
    );
    assert_eq!(
        Code::PayloadLimitExceeded.as_str(),
        "payload_limit_exceeded"
    );
    assert_eq!(
        ConfigurationChangeError::FieldMigrationRequired {
            field_id: FieldId("summary".into())
        }
        .code(),
        Code::MigrationRequired
    );
    assert_eq!(
        ConfigurationChangeError::HistoricalReferenceChange(HistoricalReference::Status(StatusId(
            "ready".into()
        )))
        .code(),
        Code::InvalidConfiguration
    );
    assert_eq!(
        ItemMutationError::Field(FieldError::Required).code(),
        Code::RequiredField
    );
    assert_eq!(
        ConversionError::Field(FieldError::ArchivedOption).code(),
        Code::ArchivedOption
    );
    assert_eq!(
        ItemMutationError::Project(ProjectError::SequenceExhausted).code(),
        Code::SequenceExhausted
    );
    assert_eq!(
        ProjectMetadataError::EstimateUnitLocked.code(),
        Code::EstimateUnitLocked
    );
}

#[test]
fn selection_and_relationship_failures_are_distinguishable() {
    assert_eq!(
        RelationshipCreateError::ConfigurationConflict {
            project_id: task_contract::work_item::ProjectId("project".into()),
            current_revision: 2,
        }
        .code(),
        Code::ConfigurationConflict
    );
    assert_eq!(
        SelectionError::VersionConflict { current_version: 1 }.code(),
        Code::SelectionVersionConflict
    );
    assert_eq!(RelationshipError::SelfLink.code(), Code::SelfLink);
    assert_eq!(
        RelationshipError::UnrelatedTypeOwner.code(),
        Code::UnrelatedTypeOwner
    );
    assert_eq!(Code::UnrelatedTypeOwner.as_str(), "unrelated_type_owner");
    assert_eq!(
        RelationshipError::Duplicate.code(),
        Code::DuplicateRelationship
    );
    assert_eq!(
        Code::DuplicateRelationship.as_str(),
        "duplicate_relationship"
    );
}

#[test]
fn every_domain_error_code_has_a_pinned_wire_string() {
    use Code::*;
    let codes = [
        ProjectAlreadyExists,
        Unauthorized,
        MigrationUnauthorized,
        OperationIdReused,
        ConfigurationConflict,
        ItemVersionConflict,
        KnowledgeVersionConflict,
        SelectionVersionConflict,
        InvalidConfiguration,
        InvalidReference,
        UnrelatedTypeOwner,
        InvalidOperation,
        ArchivedProject,
        ArchivedItem,
        ArchivedTarget,
        ArchivedField,
        ArchivedOption,
        ArchivedRelationshipType,
        HiddenField,
        RequiredField,
        InvalidFieldValue,
        InvalidSourceSnapshot,
        InvalidKnowledgeValue,
        WrongKnowledgeKind,
        InvalidEstimate,
        InvalidPrefix,
        InvalidName,
        WrongFieldType,
        NotChoiceField,
        UnknownOption,
        DuplicateField,
        PayloadLimitExceeded,
        DuplicateOption,
        DuplicateRelationship,
        DuplicateKnowledgeEntry,
        FollowUpOriginExists,
        FollowUpCycle,
        SelfLink,
        ProhibitedPhaseChange,
        MigrationRequired,
        EstimateUnitLocked,
        SequenceExhausted,
        VersionExhausted,
    ];
    assert_eq!(codes.len(), 43);
    let distinct: std::collections::HashSet<_> = codes.iter().map(|code| code.as_str()).collect();
    assert_eq!(distinct.len(), codes.len());
    for code in codes {
        let expected = match code {
            ProjectAlreadyExists => "project_already_exists",
            Unauthorized => "unauthorized",
            MigrationUnauthorized => "migration_unauthorized",
            OperationIdReused => "operation_id_reused",
            ConfigurationConflict => "configuration_conflict",
            ItemVersionConflict => "item_version_conflict",
            KnowledgeVersionConflict => "knowledge_version_conflict",
            SelectionVersionConflict => "selection_version_conflict",
            InvalidConfiguration => "invalid_configuration",
            InvalidReference => "invalid_reference",
            UnrelatedTypeOwner => "unrelated_type_owner",
            InvalidOperation => "invalid_operation",
            ArchivedProject => "archived_project",
            ArchivedItem => "archived_item",
            ArchivedTarget => "archived_target",
            ArchivedField => "archived_field",
            ArchivedOption => "archived_option",
            ArchivedRelationshipType => "archived_relationship_type",
            HiddenField => "hidden_field",
            RequiredField => "required_field",
            InvalidFieldValue => "invalid_field_value",
            InvalidSourceSnapshot => "invalid_source_snapshot",
            InvalidKnowledgeValue => "invalid_knowledge_value",
            WrongKnowledgeKind => "wrong_knowledge_kind",
            InvalidEstimate => "invalid_estimate",
            InvalidPrefix => "invalid_prefix",
            InvalidName => "invalid_name",
            WrongFieldType => "wrong_field_type",
            NotChoiceField => "not_choice_field",
            UnknownOption => "unknown_option",
            DuplicateField => "duplicate_field",
            PayloadLimitExceeded => "payload_limit_exceeded",
            DuplicateOption => "duplicate_option",
            DuplicateRelationship => "duplicate_relationship",
            DuplicateKnowledgeEntry => "duplicate_knowledge_entry",
            FollowUpOriginExists => "follow_up_origin_exists",
            FollowUpCycle => "follow_up_cycle",
            SelfLink => "self_link",
            ProhibitedPhaseChange => "prohibited_phase_change",
            MigrationRequired => "migration_required",
            EstimateUnitLocked => "estimate_unit_locked",
            SequenceExhausted => "sequence_exhausted",
            VersionExhausted => "version_exhausted",
        };
        assert_eq!(code.as_str(), expected, "{code:?}");
    }
}
