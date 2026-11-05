//! Bounded read representations; no domain mutation or authority selection.
use crate::postgres::transaction::{Item, NativeError};
use serde_json::{json, Value};
use task_contract::{item_mutation::display_name, work_item::Configuration};
use work_item_codec::SavedResult;

pub(crate) const PAGE_SIZE: usize = 16;
// Leave the outer envelope/cursor allowance of the existing native wire codec.
const PAGE_BYTES: usize = project_create_codec::MAX_RECORD_BYTES;

pub(crate) fn readable(item: &Item) -> Value {
    json!({"project_id":item.readable.project_id.0,"item_id":item.readable.item_id.0,
        "prefix":item.readable.prefix,"sequence":item.readable.sequence.to_string()})
}

pub(crate) fn read(item: Item) -> Result<Value, NativeError> {
    let result =
        work_item_codec::encode_result(&item.result, 1).map_err(|_| NativeError::Integrity)?;
    Ok(
        json!({"outcome":"read","result":project_create_codec::parse_json(&result)
        .map_err(|_| NativeError::Integrity)?,"readable_id":readable(&item)}),
    )
}

pub(crate) struct Page {
    entries: Vec<Value>,
    encoded_bytes: usize,
    before: Option<u64>,
}
impl Page {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            encoded_bytes: 2,
            before: None,
        }
    }
    pub(crate) fn push(
        &mut self,
        item: Item,
        configuration: &Configuration,
    ) -> Result<bool, NativeError> {
        let (current, data) = match &item.result {
            SavedResult::Create(value) => (&value.item, &value.data),
            SavedResult::Edit(value) => (&value.item, &value.data),
        };
        let kind = configuration
            .types
            .iter()
            .find(|kind| kind.id == current.type_id)
            .ok_or(NativeError::Integrity)?;
        let name = display_name(kind, &item.readable, data);
        if name.len() > project_create_codec::MAX_STRING_BYTES {
            return Err(NativeError::Integrity);
        }
        let value = json!({"item_id":current.id.0,"version":current.version.to_string(),
            "type_id":current.type_id.0,"workflow_id":current.workflow_id.0,"status_id":current.status_id.0,
            "archived":current.archived,"estimate":data.estimate.map(|v|v.thousandths().to_string()),
            "name":name,"readable_id":readable(&item)});
        // The single summary contains one <=64KiB title plus bounded scalar
        // identifiers. Its temporary encoding is bounded even under JSON escape
        // expansion. Retain no full item history after this iteration.
        let size = serde_json::to_vec(&value)
            .map_err(|_| NativeError::Integrity)?
            .len();
        if size > PAGE_BYTES {
            return Err(NativeError::Integrity);
        }
        let total = self
            .encoded_bytes
            .checked_add(size)
            .and_then(|v| v.checked_add(1))
            .ok_or(NativeError::Integrity)?;
        if self.entries.len() == PAGE_SIZE || total > PAGE_BYTES {
            return Ok(false);
        }
        self.encoded_bytes = total;
        self.before = Some(item.readable.sequence);
        self.entries.push(value);
        Ok(true)
    }
    pub(crate) fn finish(self, more: bool) -> Value {
        json!({"outcome":"listed","entries":self.entries,
            "before":if more { self.before.map(|v|v.to_string()) } else { None }})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use task_contract::{
        field::{FieldEntry, FieldValue},
        item_mutation::{decide_create, CreateCommand, CreateDecision},
        work_item::WorkItemId,
    };

    fn fixture(sequence: u64, title: Option<&str>) -> (Item, Configuration) {
        let prepared = crate::project::Prepared::new(
            crate::native_ingress::Form {
                name: "Project".into(),
                prefix: "PR".into(),
                estimate_unit: task_contract::project::EstimateUnit::Hours,
            },
            "p".into(),
            "0".repeat(32),
        )
        .unwrap();
        let configuration = prepared.result().configuration.clone();
        let mut project = prepared.result().project.clone();
        project.next_sequence = sequence;
        let kind = &configuration.types[0];
        let command = CreateCommand {
            operation_id: "1".repeat(32),
            expected_configuration_revision: 1,
            item_id: WorkItemId(format!("i{sequence}")),
            type_id: kind.id.clone(),
            workflow_id: None,
            estimate: None,
            values: title
                .map(|text| {
                    vec![FieldEntry {
                        id: kind.title_field_id.clone().unwrap(),
                        value: FieldValue::Text(text.into()),
                    }]
                })
                .unwrap_or_default(),
        };
        let CreateDecision::Apply(result) =
            decide_create(&project, &configuration, None, true, &command)
        else {
            panic!("valid fixture rejected")
        };
        let readable = result.readable_id.clone();
        (
            Item {
                result: SavedResult::Create(result),
                readable,
            },
            configuration,
        )
    }

    #[test]
    fn list_preserves_exact_title_and_uses_identity_for_blank_or_absent_title() {
        for (title, expected) in [
            (Some("  exact title  "), "  exact title  "),
            (Some(" \t"), "PR-1"),
            (None, "PR-1"),
        ] {
            let (item, configuration) = fixture(1, title);
            let mut page = Page::new();
            assert!(page
                .push(item, &configuration)
                .unwrap_or_else(|_| panic!("summary rejected")));
            let value = page.finish(false);
            assert_eq!(value["entries"][0]["name"], expected);
            assert!(value["before"].is_null());
        }
    }

    #[test]
    fn page_byte_stop_keeps_last_delivered_cursor_and_never_truncates_a_title() {
        // Valid accepted field text, with six-byte JSON escape expansion.
        let title = "\u{1}".repeat(16 * 1024);
        let mut page = Page::new();
        for sequence in (11..=20).rev() {
            let (item, configuration) = fixture(sequence, Some(&title));
            assert!(page
                .push(item, &configuration)
                .unwrap_or_else(|_| panic!("summary rejected")));
        }
        let (item, configuration) = fixture(10, Some(&title));
        assert!(!page
            .push(item, &configuration)
            .unwrap_or_else(|_| panic!("summary rejected")));
        let value = page.finish(true);
        assert_eq!(value["before"], "11");
        assert_eq!(value["entries"].as_array().unwrap().len(), 10);
        assert_eq!(value["entries"][1]["name"], title);
        assert!(crate::project::wire::encode(&value).is_ok());
    }

    #[test]
    fn count_stop_does_not_adopt_the_undelivered_sequence() {
        let mut page = Page::new();
        for sequence in (5..=20).rev() {
            let (item, configuration) = fixture(sequence, None);
            assert!(page
                .push(item, &configuration)
                .unwrap_or_else(|_| panic!("summary rejected")));
        }
        let (item, configuration) = fixture(4, None);
        assert!(!page
            .push(item, &configuration)
            .unwrap_or_else(|_| panic!("summary rejected")));
        let value = page.finish(true);
        assert_eq!(value["before"], "5");
        assert_eq!(value["entries"].as_array().unwrap().len(), PAGE_SIZE);
    }
}
