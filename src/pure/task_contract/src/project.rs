//! Pure project, readable-ID, and estimate rules for B-003.
//!
//! Callers must serialize durable sequence allocation and project archival
//! with affected current-work selections. These functions only decide results.

pub use crate::field::FieldKind;
use crate::work_item::{ProjectId, WorkItemId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EstimateUnit {
    Hours,
    Points,
}

/// Exact nonnegative thousandths of the selected project unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Estimate(u64);

impl Estimate {
    /// Fits a decimal(9,3) representation without rounding.
    pub const MAX_THOUSANDTHS: u64 = 999_999_999;

    pub fn try_from_thousandths(value: u64) -> Result<Self, ProjectError> {
        if value > Self::MAX_THOUSANDTHS {
            return Err(ProjectError::InvalidEstimate);
        }
        Ok(Self(value))
    }

    pub const fn thousandths(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub prefix: String,
    pub configuration_revision: u64,
    /// Next project-local number; issued numbers never change or recycle.
    pub next_sequence: u64,
    pub archived: bool,
    /// Existing estimates retain the unit active when they were recorded.
    pub estimate_unit: EstimateUnit,
    /// Monotonic flag: clearing the last estimate never unlocks unit changes.
    pub ever_estimated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadableId {
    pub item_id: WorkItemId,
    pub project_id: ProjectId,
    pub prefix: String,
    pub sequence: u64,
}

impl ReadableId {
    pub fn display(&self) -> String {
        format!("{}-{}", self.prefix, self.sequence)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveAction {
    Archive,
    Restore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveEffect {
    None,
    ClearCurrentWorkInProject(ProjectId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveDecision {
    pub project: Project,
    pub effect: ArchiveEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemAccess {
    pub readable: bool,
    pub writable: bool,
    pub selectable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectError {
    Archived,
    InvalidOperation,
    InvalidPrefix,
    InvalidIdentity,
    SequenceExhausted,
    RevisionExhausted,
    InvalidEstimate,
    EstimateUnitLocked,
    FieldMigrationRequired,
}

/// Project archival has no WorkItem lifecycle or resource effect.
/// The returned selection-clearing instruction must be committed coherently.
pub fn decide_project_archive(
    project: &Project,
    action: ArchiveAction,
) -> Result<ArchiveDecision, ProjectError> {
    let archive = action == ArchiveAction::Archive;
    if project.archived == archive {
        return Err(ProjectError::InvalidOperation);
    }
    let mut next = project.clone();
    next.archived = archive;
    next.configuration_revision = project
        .configuration_revision
        .checked_add(1)
        .ok_or(ProjectError::RevisionExhausted)?;
    Ok(ArchiveDecision {
        project: next,
        effect: if archive {
            ArchiveEffect::ClearCurrentWorkInProject(project.id.clone())
        } else {
            ArchiveEffect::None
        },
    })
}

/// This expresses domain eligibility; authorization remains a separate check.
pub fn item_access(project: &Project, item_archived: bool) -> ItemAccess {
    ItemAccess {
        readable: true,
        writable: !project.archived && !item_archived,
        selectable: !project.archived && !item_archived,
    }
}

/// Readable IDs are scoped to the project. B-005 owns atomic sequence allocation.
pub fn allocate_readable_id(
    project: &Project,
    item_id: WorkItemId,
) -> Result<(Project, ReadableId), ProjectError> {
    if project.archived {
        return Err(ProjectError::Archived);
    }
    if item_id.0.is_empty() || project.id.0.is_empty() {
        return Err(ProjectError::InvalidIdentity);
    }
    if !valid_prefix(&project.prefix) {
        return Err(ProjectError::InvalidPrefix);
    }
    if project.next_sequence == 0 {
        return Err(ProjectError::SequenceExhausted);
    }
    let next_sequence = project
        .next_sequence
        .checked_add(1)
        .ok_or(ProjectError::SequenceExhausted)?;
    let issued = ReadableId {
        item_id,
        project_id: project.id.clone(),
        prefix: project.prefix.clone(),
        sequence: project.next_sequence,
    };
    let mut next = project.clone();
    next.next_sequence = next_sequence;
    Ok((next, issued))
}

/// Prefix syntax is an initial bounded contract; the display value is frozen
/// into every issued ID so changing this prefix cannot rename old items.
pub fn change_prefix(project: &Project, prefix: &str) -> Result<Project, ProjectError> {
    if project.archived {
        return Err(ProjectError::Archived);
    }
    if !valid_prefix(prefix) {
        return Err(ProjectError::InvalidPrefix);
    }
    let mut next = project.clone();
    if project.prefix == prefix {
        return Ok(next);
    }
    next.prefix = prefix.to_owned();
    next.configuration_revision = project
        .configuration_revision
        .checked_add(1)
        .ok_or(ProjectError::RevisionExhausted)?;
    Ok(next)
}

pub fn valid_prefix(prefix: &str) -> bool {
    (2..=12).contains(&prefix.len())
        && prefix.as_bytes()[0].is_ascii_uppercase()
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

/// Parse decimal text without floating point or implicit rounding.
/// At most three fractional digits are accepted.
pub fn estimate_from_decimal(value: &str) -> Result<Estimate, ProjectError> {
    let (whole, fraction) = match value.split_once('.') {
        Some(parts) => parts,
        None => (value, ""),
    };
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
        || fraction.len() > 3
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ProjectError::InvalidEstimate);
    }
    let fractional_digits = fraction.len();
    let whole: u64 = whole.parse().map_err(|_| ProjectError::InvalidEstimate)?;
    let fraction: u64 = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse()
            .map_err(|_| ProjectError::InvalidEstimate)?
    };
    let factor = match fractional_digits {
        0 | 3 => 1,
        1 => 100,
        2 => 10,
        _ => return Err(ProjectError::InvalidEstimate),
    };
    let scaled = whole
        .checked_mul(1000)
        .and_then(|base| {
            fraction
                .checked_mul(factor)
                .and_then(|part| base.checked_add(part))
        })
        .ok_or(ProjectError::InvalidEstimate)?;
    Estimate::try_from_thousandths(scaled)
}

pub fn change_estimate_unit(
    project: &Project,
    unit: EstimateUnit,
) -> Result<Project, ProjectError> {
    if project.archived {
        return Err(ProjectError::Archived);
    }
    if project.estimate_unit != unit && project.ever_estimated {
        return Err(ProjectError::EstimateUnitLocked);
    }
    let mut next = project.clone();
    if project.estimate_unit == unit {
        return Ok(next);
    }
    next.estimate_unit = unit;
    next.configuration_revision = project
        .configuration_revision
        .checked_add(1)
        .ok_or(ProjectError::RevisionExhausted)?;
    Ok(next)
}

/// Record the monotonic unit lock when any estimate value is first accepted.
/// Clearing an estimate (`None`) does not clear that lock.
pub fn record_estimate(
    project: &Project,
    estimate: Option<Estimate>,
) -> Result<Project, ProjectError> {
    if project.archived {
        return Err(ProjectError::Archived);
    }
    let mut next = project.clone();
    next.ever_estimated |= estimate.is_some();
    Ok(next)
}

/// `ever_valued` must reflect both current and historical values. A new kind
/// needs a new field and explicit migration once the old field held a value.
pub fn change_field_kind(
    project: &Project,
    current: FieldKind,
    requested: FieldKind,
    ever_valued: bool,
) -> Result<(), ProjectError> {
    if project.archived {
        return Err(ProjectError::Archived);
    }
    if current != requested && ever_valued {
        Err(ProjectError::FieldMigrationRequired)
    } else {
        Ok(())
    }
}
