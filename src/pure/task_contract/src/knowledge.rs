//! Typed, manually entered knowledge attached to a WorkItem.

use crate::relationship::EndpointIdentity;

pub const MAX_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_FILE_LABEL_BYTES: usize = 256;
pub const MAX_FILE_LOCATOR_BYTES: usize = 2048;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct KnowledgeEntryId(pub String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeKind {
    CreatedFile,
    Decision,
    Lesson,
    Insight,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileReference {
    pub label: String,
    /// An opaque external reference; validation does not fetch or resolve it.
    pub locator: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KnowledgeValue {
    File(FileReference),
    Text(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeEntry {
    pub id: KnowledgeEntryId,
    pub item: EndpointIdentity,
    pub kind: KnowledgeKind,
    pub value: KnowledgeValue,
    pub version: i64,
    pub archived: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeError {
    InvalidReference,
    InvalidValue,
    WrongKind,
}

impl KnowledgeEntry {
    pub fn validate(&self) -> Result<(), KnowledgeError> {
        if self.id.0.is_empty()
            || self.item.project_id.0.is_empty()
            || self.item.item_id.0.is_empty()
            || self.version <= 0
        {
            return Err(KnowledgeError::InvalidReference);
        }
        match (self.kind, &self.value) {
            (KnowledgeKind::CreatedFile, KnowledgeValue::File(reference)) => {
                if invalid_line(&reference.label, MAX_FILE_LABEL_BYTES)
                    || invalid_line(&reference.locator, MAX_FILE_LOCATOR_BYTES)
                {
                    return Err(KnowledgeError::InvalidValue);
                }
            }
            (
                KnowledgeKind::Decision | KnowledgeKind::Lesson | KnowledgeKind::Insight,
                KnowledgeValue::Text(text),
            ) => {
                if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
                    return Err(KnowledgeError::InvalidValue);
                }
            }
            _ => return Err(KnowledgeError::WrongKind),
        }
        Ok(())
    }
}

fn invalid_line(value: &str, max_bytes: usize) -> bool {
    value.trim().is_empty() || value.len() > max_bytes || value.chars().any(char::is_control)
}
