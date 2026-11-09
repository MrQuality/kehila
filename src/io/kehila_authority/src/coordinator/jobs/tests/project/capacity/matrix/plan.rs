use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mix {
    Read,
    Edit,
    Mixed,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Cell {
    pub(super) index: usize,
    pub(super) fields: usize,
    pub(super) text_bytes: usize,
    pub(super) projects: usize,
    pub(super) mix: Mix,
    pub(super) concurrency: usize,
    pub(super) repetition: usize,
}

impl Cell {
    pub(super) fn new(index: usize) -> Option<Self> {
        if index >= 180 {
            return None;
        }
        Some(Self {
            index,
            fields: if index / 90 == 0 { 4 } else { 128 },
            text_bytes: if index / 90 == 0 { 64 } else { 1024 },
            projects: if (index / 45).is_multiple_of(2) { 1 } else { 4 },
            mix: [Mix::Read, Mix::Edit, Mix::Mixed][index / 15 % 3],
            concurrency: [1, 2, 4, 8, 16][index / 3 % 5],
            repetition: index % 3 + 1,
        })
    }

    pub(super) fn edit(self, ordinal: usize) -> bool {
        match self.mix {
            Mix::Read => false,
            Mix::Edit => true,
            Mix::Mixed => ordinal % 2 == 1,
        }
    }

    pub(super) fn document(self) -> Value {
        json!({
            "schema":1,"index":self.index,
            "profile":if self.fields == 4 { "compact" } else { "large" },
            "synthetic_configuration":true,"fields_per_item":self.fields,
            "text_bytes_per_field":self.text_bytes,
            "layout":if self.projects == 1 { "shared" } else { "independent" },
            "projects":self.projects,"mix":match self.mix {
                Mix::Read=>"read",Mix::Edit=>"edit",Mix::Mixed=>"mixed"
            },
            "concurrency":self.concurrency,"repetition":self.repetition,
            "seeded_items":16,"authenticated_sessions":4,"warmup_requests":10,"measured_requests":100
        })
    }
}

#[test]
fn capacity_cell_plan_has_exact_finite_dimensions_and_mix_counts() {
    let mut combinations = std::collections::BTreeSet::new();
    for index in 0..180 {
        let cell = Cell::new(index).unwrap();
        assert_eq!(cell.document()["index"], index);
        assert!(combinations.insert((
            cell.fields,
            cell.projects,
            cell.document()["mix"].as_str().unwrap().to_owned(),
            cell.concurrency,
            cell.repetition
        )));
        let edits = (0..100).filter(|ordinal| cell.edit(*ordinal)).count();
        assert_eq!(
            edits,
            match cell.mix {
                Mix::Read => 0,
                Mix::Edit => 100,
                Mix::Mixed => 50,
            }
        );
    }
    assert_eq!(combinations.len(), 180);
    assert!(Cell::new(180).is_none());
    assert!(Cell::new(usize::MAX).is_none());
    let last = Cell::new(179).unwrap();
    assert_eq!(
        (
            last.fields,
            last.projects,
            last.mix,
            last.concurrency,
            last.repetition
        ),
        (128, 4, Mix::Mixed, 16, 3)
    );
}
