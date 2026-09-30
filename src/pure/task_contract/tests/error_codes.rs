use task_contract::archive::ArchiveCommandError;
use task_contract::configuration_change::ConfigurationChangeError;
use task_contract::conversion::ConversionError;
use task_contract::current_work::SelectionError;
use task_contract::error_code::{Code, StableCode};
use task_contract::field::{FieldError, FieldId};
use task_contract::field_admin::OptionAdminError;
use task_contract::item_mutation::ItemMutationError;
use task_contract::project::ProjectError;
use task_contract::relationship::RelationshipError;
use task_contract::work_item::Error;

#[test]
fn shared_conflicts_have_the_same_stable_code_across_commands() {
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
        ConfigurationChangeError::FieldMigrationRequired {
            field_id: FieldId("summary".into())
        }
        .code(),
        Code::MigrationRequired
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
        ProjectError::EstimateUnitLocked.code(),
        Code::EstimateUnitLocked
    );
}

#[test]
fn selection_and_relationship_failures_are_distinguishable() {
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
