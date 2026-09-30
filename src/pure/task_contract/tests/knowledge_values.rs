use task_contract::error_code::{Code, StableCode};
use task_contract::knowledge::{
    FileReference, KnowledgeEntry, KnowledgeEntryId, KnowledgeError, KnowledgeKind, KnowledgeValue,
};
use task_contract::relationship::EndpointIdentity;
use task_contract::work_item::{ProjectId, WorkItemId};

fn entry(kind: KnowledgeKind, value: KnowledgeValue) -> KnowledgeEntry {
    KnowledgeEntry {
        id: KnowledgeEntryId("knowledge-1".into()),
        item: EndpointIdentity {
            project_id: ProjectId("project".into()),
            item_id: WorkItemId("item".into()),
        },
        kind,
        value,
        version: 1,
    }
}

#[test]
fn typed_entries_keep_their_category_and_validate_content() {
    for kind in [
        KnowledgeKind::Decision,
        KnowledgeKind::Lesson,
        KnowledgeKind::Insight,
    ] {
        let valid = entry(kind, KnowledgeValue::Text("Recorded observation".into()));
        assert_eq!(valid.validate(), Ok(()));
        let blank = entry(kind, KnowledgeValue::Text("  \n".into()));
        assert_eq!(blank.validate(), Err(KnowledgeError::InvalidValue));
    }
    let wrong = entry(
        KnowledgeKind::CreatedFile,
        KnowledgeValue::Text("a path".into()),
    );
    assert_eq!(wrong.validate(), Err(KnowledgeError::WrongKind));
}

#[test]
fn labeled_file_references_are_data_and_have_utf8_bounds() {
    let valid = entry(
        KnowledgeKind::CreatedFile,
        KnowledgeValue::File(FileReference {
            label: "Design".into(),
            locator: "docs/design.md".into(),
        }),
    );
    assert_eq!(valid.validate(), Ok(()));
    let mut invalid = valid.clone();
    invalid.value = KnowledgeValue::File(FileReference {
        label: " ".into(),
        locator: "https://example.org/commit/abc".into(),
    });
    assert_eq!(invalid.validate(), Err(KnowledgeError::InvalidValue));
    invalid.value = KnowledgeValue::File(FileReference {
        label: "Design".into(),
        locator: "bad\u{0000}reference".into(),
    });
    assert_eq!(invalid.validate(), Err(KnowledgeError::InvalidValue));
    invalid.value = KnowledgeValue::File(FileReference {
        label: "é".repeat(129),
        locator: "docs/design.md".into(),
    });
    assert_eq!(invalid.validate(), Err(KnowledgeError::InvalidValue));
}

#[test]
fn entry_identity_version_and_text_limit_are_checked() {
    let mut invalid = entry(KnowledgeKind::Decision, KnowledgeValue::Text("x".into()));
    invalid.version = 0;
    assert_eq!(invalid.validate(), Err(KnowledgeError::InvalidReference));
    invalid.version = 1;
    invalid.value = KnowledgeValue::Text("é".repeat(8193));
    assert_eq!(invalid.validate(), Err(KnowledgeError::InvalidValue));
    invalid.value = KnowledgeValue::Text("é".repeat(8192));
    assert_eq!(invalid.validate(), Ok(()));
}

#[test]
fn knowledge_errors_have_stable_codes() {
    assert_eq!(
        KnowledgeError::InvalidReference.code(),
        Code::InvalidReference
    );
    assert_eq!(
        KnowledgeError::InvalidValue.code(),
        Code::InvalidKnowledgeValue
    );
    assert_eq!(KnowledgeError::WrongKind.code(), Code::WrongKnowledgeKind);
}
