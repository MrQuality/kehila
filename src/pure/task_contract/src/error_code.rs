//! Stable domain error codes shared by B-003 pure command decisions.
//!
//! The transport adapter may add detail, but must not infer a different code
//! from message text. HTTP mapping is outside this pure contract.

use crate::archive::ArchiveCommandError;
use crate::configuration_change::ConfigurationChangeError;
use crate::conversion::ConversionError;
use crate::current_work::SelectionError;
use crate::field::FieldError;
use crate::field_admin::OptionAdminError;
use crate::item_mutation::ItemMutationError;
use crate::project::ProjectError;
use crate::relationship::RelationshipError;
use crate::relationship_command::RelationshipCreateError;
use crate::work_item::Error as WorkItemError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Code {
    Unauthorized,
    MigrationUnauthorized,
    OperationIdReused,
    ConfigurationConflict,
    ItemVersionConflict,
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
    SelfLink,
    ProhibitedPhaseChange,
    MigrationRequired,
    EstimateUnitLocked,
    SequenceExhausted,
    VersionExhausted,
}

impl Code {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::MigrationUnauthorized => "migration_unauthorized",
            Self::OperationIdReused => "operation_id_reused",
            Self::ConfigurationConflict => "configuration_conflict",
            Self::ItemVersionConflict => "item_version_conflict",
            Self::SelectionVersionConflict => "selection_version_conflict",
            Self::InvalidConfiguration => "invalid_configuration",
            Self::InvalidReference => "invalid_reference",
            Self::UnrelatedTypeOwner => "unrelated_type_owner",
            Self::InvalidOperation => "invalid_operation",
            Self::ArchivedProject => "archived_project",
            Self::ArchivedItem => "archived_item",
            Self::ArchivedTarget => "archived_target",
            Self::ArchivedField => "archived_field",
            Self::ArchivedOption => "archived_option",
            Self::ArchivedRelationshipType => "archived_relationship_type",
            Self::HiddenField => "hidden_field",
            Self::RequiredField => "required_field",
            Self::InvalidFieldValue => "invalid_field_value",
            Self::InvalidEstimate => "invalid_estimate",
            Self::InvalidPrefix => "invalid_prefix",
            Self::InvalidName => "invalid_name",
            Self::WrongFieldType => "wrong_field_type",
            Self::NotChoiceField => "not_choice_field",
            Self::UnknownOption => "unknown_option",
            Self::DuplicateField => "duplicate_field",
            Self::PayloadLimitExceeded => "payload_limit_exceeded",
            Self::DuplicateOption => "duplicate_option",
            Self::DuplicateRelationship => "duplicate_relationship",
            Self::SelfLink => "self_link",
            Self::ProhibitedPhaseChange => "prohibited_phase_change",
            Self::MigrationRequired => "migration_required",
            Self::EstimateUnitLocked => "estimate_unit_locked",
            Self::SequenceExhausted => "sequence_exhausted",
            Self::VersionExhausted => "version_exhausted",
        }
    }
}

pub trait StableCode {
    fn code(&self) -> Code;
}

impl StableCode for WorkItemError {
    fn code(&self) -> Code {
        match self {
            Self::Unauthorized => Code::Unauthorized,
            Self::OperationIdReused => Code::OperationIdReused,
            Self::ConfigurationConflict { .. } => Code::ConfigurationConflict,
            Self::ItemVersionConflict { .. } => Code::ItemVersionConflict,
            Self::InvalidReference => Code::InvalidReference,
            Self::InvalidConfiguration => Code::InvalidConfiguration,
            Self::ArchivedTarget => Code::ArchivedTarget,
            Self::ArchivedProject => Code::ArchivedProject,
            Self::ProhibitedPhaseChange => Code::ProhibitedPhaseChange,
            Self::InvalidOperation => Code::InvalidOperation,
            Self::VersionOverflow => Code::VersionExhausted,
        }
    }
}

impl StableCode for FieldError {
    fn code(&self) -> Code {
        match self {
            Self::InvalidDefinition => Code::InvalidConfiguration,
            Self::WrongType => Code::WrongFieldType,
            Self::WrongValueKind | Self::InvalidValue | Self::InvalidDate => {
                Code::InvalidFieldValue
            }
            Self::Hidden => Code::HiddenField,
            Self::Archived => Code::ArchivedField,
            Self::Required | Self::ExistingValueMissing => Code::RequiredField,
            Self::UnknownOption => Code::UnknownOption,
            Self::ArchivedOption => Code::ArchivedOption,
        }
    }
}

impl StableCode for ProjectError {
    fn code(&self) -> Code {
        match self {
            Self::Archived => Code::ArchivedProject,
            Self::InvalidOperation => Code::InvalidOperation,
            Self::InvalidPrefix => Code::InvalidPrefix,
            Self::InvalidIdentity => Code::InvalidReference,
            Self::SequenceExhausted => Code::SequenceExhausted,
            Self::RevisionExhausted => Code::VersionExhausted,
            Self::InvalidEstimate => Code::InvalidEstimate,
            Self::EstimateUnitLocked => Code::EstimateUnitLocked,
            Self::FieldMigrationRequired => Code::MigrationRequired,
        }
    }
}

impl StableCode for RelationshipError {
    fn code(&self) -> Code {
        match self {
            Self::Unauthorized => Code::Unauthorized,
            Self::InvalidReference => Code::InvalidReference,
            Self::UnrelatedTypeOwner => Code::UnrelatedTypeOwner,
            Self::SelfLink => Code::SelfLink,
            Self::Duplicate => Code::DuplicateRelationship,
            Self::ArchivedTarget => Code::ArchivedTarget,
            Self::ArchivedType => Code::ArchivedRelationshipType,
        }
    }
}

impl StableCode for RelationshipCreateError {
    fn code(&self) -> Code {
        match self {
            Self::Unauthorized => Code::Unauthorized,
            Self::OperationIdReused => Code::OperationIdReused,
            Self::ConfigurationConflict { .. } => Code::ConfigurationConflict,
            Self::ItemVersionConflict { .. } => Code::ItemVersionConflict,
            Self::InvalidReference => Code::InvalidReference,
            Self::InvalidOperation => Code::InvalidOperation,
            Self::Relationship(error) => error.code(),
        }
    }
}

impl StableCode for SelectionError {
    fn code(&self) -> Code {
        match self {
            Self::Unauthorized => Code::Unauthorized,
            Self::VersionConflict { .. } => Code::SelectionVersionConflict,
            Self::InvalidReference => Code::InvalidReference,
            Self::ArchivedTarget => Code::ArchivedTarget,
            Self::VersionOverflow => Code::VersionExhausted,
        }
    }
}

impl StableCode for ConfigurationChangeError {
    fn code(&self) -> Code {
        match self {
            Self::Unauthorized => Code::Unauthorized,
            Self::OperationIdReused => Code::OperationIdReused,
            Self::RevisionConflict { .. } => Code::ConfigurationConflict,
            Self::ArchivedProject => Code::ArchivedProject,
            Self::InvalidConfiguration => Code::InvalidConfiguration,
            Self::InvalidReference => Code::InvalidReference,
            Self::InvalidOperation => Code::InvalidOperation,
            Self::RevisionOverflow => Code::VersionExhausted,
            Self::MigrationRequired { .. } => Code::MigrationRequired,
            Self::FieldMigrationRequired { .. } => Code::MigrationRequired,
        }
    }
}

impl StableCode for ConversionError {
    fn code(&self) -> Code {
        match self {
            Self::Unauthorized => Code::Unauthorized,
            Self::MigrationUnauthorized => Code::MigrationUnauthorized,
            Self::OperationIdReused => Code::OperationIdReused,
            Self::ConfigurationConflict { .. } => Code::ConfigurationConflict,
            Self::ItemVersionConflict { .. } => Code::ItemVersionConflict,
            Self::InvalidReference => Code::InvalidReference,
            Self::InvalidConfiguration => Code::InvalidConfiguration,
            Self::ArchivedProject => Code::ArchivedProject,
            Self::ArchivedItem => Code::ArchivedItem,
            Self::ArchivedTarget => Code::ArchivedTarget,
            Self::InvalidOperation => Code::InvalidOperation,
            Self::DuplicateField => Code::DuplicateField,
            Self::PayloadLimitExceeded => Code::PayloadLimitExceeded,
            Self::Field(error) => error.code(),
            Self::ProhibitedPhaseChange => Code::ProhibitedPhaseChange,
            Self::VersionOverflow => Code::VersionExhausted,
        }
    }
}

impl StableCode for ItemMutationError {
    fn code(&self) -> Code {
        match self {
            Self::Unauthorized => Code::Unauthorized,
            Self::OperationIdReused => Code::OperationIdReused,
            Self::ConfigurationConflict { .. } => Code::ConfigurationConflict,
            Self::ItemVersionConflict { .. } => Code::ItemVersionConflict,
            Self::InvalidConfiguration => Code::InvalidConfiguration,
            Self::InvalidReference => Code::InvalidReference,
            Self::InvalidOperation => Code::InvalidOperation,
            Self::ArchivedProject => Code::ArchivedProject,
            Self::ArchivedItem => Code::ArchivedItem,
            Self::ArchivedTarget => Code::ArchivedTarget,
            Self::DuplicateField => Code::DuplicateField,
            Self::PayloadLimitExceeded => Code::PayloadLimitExceeded,
            Self::Field(error) => error.code(),
            Self::Project(error) => error.code(),
            Self::VersionOverflow => Code::VersionExhausted,
        }
    }
}

impl StableCode for ArchiveCommandError {
    fn code(&self) -> Code {
        match self {
            Self::Unauthorized => Code::Unauthorized,
            Self::OperationIdReused => Code::OperationIdReused,
            Self::ConfigurationConflict { .. } => Code::ConfigurationConflict,
            Self::ItemVersionConflict { .. } => Code::ItemVersionConflict,
            Self::InvalidConfiguration => Code::InvalidConfiguration,
            Self::InvalidReference => Code::InvalidReference,
            Self::InvalidOperation => Code::InvalidOperation,
            Self::ArchivedProject => Code::ArchivedProject,
            Self::VersionOverflow => Code::VersionExhausted,
        }
    }
}

impl StableCode for OptionAdminError {
    fn code(&self) -> Code {
        match self {
            Self::Unauthorized => Code::Unauthorized,
            Self::OperationIdReused => Code::OperationIdReused,
            Self::ConfigurationConflict { .. } => Code::ConfigurationConflict,
            Self::InvalidConfiguration => Code::InvalidConfiguration,
            Self::InvalidReference => Code::InvalidReference,
            Self::InvalidOperation => Code::InvalidOperation,
            Self::ArchivedProject => Code::ArchivedProject,
            Self::ArchivedField => Code::ArchivedField,
            Self::NotChoiceField => Code::NotChoiceField,
            Self::DuplicateOption => Code::DuplicateOption,
            Self::UnknownOption => Code::UnknownOption,
            Self::InvalidName => Code::InvalidName,
            Self::RevisionOverflow => Code::VersionExhausted,
        }
    }
}
