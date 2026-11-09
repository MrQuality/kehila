//! One client owns each item's version chain. Recovery never changes its identity.
use super::super::*;
use super::plan::Cell;
use serde::Serialize;
use task_contract::work_item::{ProjectId, WorkItemId};

fn base64(bytes: &[u8]) -> String {
    let mut encoded = vec![0; data_encoding::BASE64.encode_len(bytes.len())];
    data_encoding::BASE64.encode_mut(bytes, &mut encoded);
    String::from_utf8(encoded).expect("base64 is ASCII")
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Warmup,
    Measured,
    Recovery,
    Control,
    Calibration,
}

pub(super) struct Slot {
    pub(super) index: usize,
    pub(super) project: String,
    pub(super) item: String,
    pub(super) fields: Vec<String>,
    pub(super) saved: SavedResult,
    pub(super) seed: Value,
    pub(super) pending: Option<Attempt>,
}

#[derive(Clone, Serialize)]
pub(super) struct Attempt {
    pub(super) phase: Phase,
    pub(super) ordinal: usize,
    pub(super) slot: usize,
    pub(super) session: usize,
    pub(super) edit: bool,
    pub(super) project: String,
    pub(super) item: String,
    pub(super) expected_item_version: u64,
    pub(super) request: Value,
    pub(super) canonical_base64: Option<String>,
    pub(super) request_json_bytes: usize,
    pub(super) response_body_bytes: usize,
    pub(super) response_base64: Option<String>,
    pub(super) status: Option<u16>,
    pub(super) latency_ns: u64,
    pub(super) outcome: &'static str,
    pub(super) transport_error: Option<String>,
    pub(super) integrity_error: Option<String>,
    pub(super) uncertain: bool,
}

pub(super) struct Command {
    phase: Phase,
    ordinal: usize,
    edit: bool,
    session: usize,
    body: Value,
    canonical: Option<String>,
}

impl Slot {
    pub(super) fn version(&self) -> u64 {
        match &self.saved {
            SavedResult::Create(value) => value.item.version,
            SavedResult::Edit(value) => value.item.version,
        }
    }

    pub(super) fn document(&self) -> Value {
        json!({"slot":self.index,"project_id":self.project,"item_id":self.item,
            "field_ids":self.fields,"seed":self.seed,"expected_final_version":self.version(),
            "final_result":saved_json(&self.saved),"unresolved":self.pending.is_some()})
    }

    pub(super) fn command(
        &self,
        cell: Cell,
        phase: Phase,
        ordinal: usize,
        texts: &[String],
    ) -> Command {
        let (edit, token) = match phase {
            Phase::Warmup => (cell.edit(ordinal), 1_000 + ordinal),
            Phase::Measured => (cell.edit(ordinal), 2_000 + ordinal),
            Phase::Control => (true, 12_000 + ordinal),
            Phase::Calibration => (true, 14_000 + ordinal),
            Phase::Recovery => panic!("reconciliation requires the original command identity"),
        };
        let body = if edit {
            assert_eq!(texts.len(), self.fields.len());
            json!({"project_id":self.project,"item_id":self.item,"operation_id":format!("{token:032x}"),
                "expected_item_version":self.version().to_string(),"expected_configuration_revision":"1",
                "estimate":{"action":"keep"},"fields":self.fields.iter().zip(texts).map(|(id,value)|
                    json!({"id":id,"action":"set","value":value})).collect::<Vec<_>>()})
        } else {
            json!({"project_id":self.project,"item_id":self.item})
        };
        let canonical = if edit {
            let input = crate::item_mutation_ingress::decode(
                crate::item_mutation_ingress::Route::Edit,
                &serde_json::to_vec(&body).unwrap(),
            )
            .unwrap();
            let crate::item_mutation_ingress::Request::Edit {
                project,
                item,
                command,
            } = input.request
            else {
                panic!("capacity edit identity");
            };
            Some(base64(
                work_item_codec::admit_request(&work_item_codec::Request::Edit {
                    project_id: ProjectId(project),
                    item_id: WorkItemId(item),
                    command,
                })
                .unwrap()
                .as_bytes(),
            ))
        } else {
            None
        };
        Command {
            phase,
            ordinal,
            edit,
            session: ordinal % 4,
            body,
            canonical,
        }
    }

    fn accept(&mut self, cell: Cell, command: &Command, value: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(&value["result"]).map_err(|error| error.to_string())?;
        let result = work_item_codec::decode_result(1, &bytes)
            .map_err(|error| format!("result codec: {error:?}"))?;
        if !command.edit {
            return if result == self.saved && value["readable_id"] == self.seed["readable_id"] {
                Ok(())
            } else {
                Err("read differs from exact known snapshot or readable identity".into())
            };
        }
        let SavedResult::Edit(edited) = &result else {
            return Err("edit returned another command family".into());
        };
        let (previous_project, previous_item, previous_data, previous_phase) = match &self.saved {
            SavedResult::Create(value) => (&value.project, &value.item, &value.data, value.phase),
            SavedResult::Edit(value) => (&value.project, &value.item, &value.data, value.phase),
        };
        let mut expected_item = previous_item.clone();
        expected_item.version = expected_item
            .version
            .checked_add(1)
            .ok_or("client version overflow")?;
        let mut expected_project = previous_project.clone();
        expected_project.next_sequence = (16 / cell.projects + 1) as u64;
        if edited.item != expected_item
            || edited.project != expected_project
            || edited.phase != previous_phase
            || edited.data.estimate != previous_data.estimate
            || edited.data.values.len() != self.fields.len()
        {
            return Err("edit changed identity, version, project, lifecycle or estimate".into());
        }
        let returned = edited
            .data
            .values
            .iter()
            .map(|entry| (entry.id.0.as_str(), &entry.value))
            .collect::<std::collections::HashMap<_, _>>();
        if returned.len() != self.fields.len() {
            return Err("edit contains duplicate field identities".into());
        }
        for field in command.body["fields"]
            .as_array()
            .ok_or("client fields absent")?
        {
            let id = field["id"].as_str().ok_or("client field identity absent")?;
            let text = field["value"].as_str().ok_or("client field text absent")?;
            if !matches!(returned.get(id).copied(), Some(FieldValue::Text(actual)) if actual == text)
            {
                return Err("edit differs from requested full-field values".into());
            }
        }
        self.saved = result;
        Ok(())
    }

    pub(super) async fn send(
        &mut self,
        cell: Cell,
        command: Command,
        client: &Client,
        browser: &Browser,
    ) -> Attempt {
        let recovery = command.phase == Phase::Recovery;
        let path = if recovery {
            "/items/reconcile"
        } else if command.edit {
            "/items/edit"
        } else {
            "/items/read"
        };
        let request_json_bytes = serde_json::to_vec(&command.body).unwrap().len();
        let wire = wire_request(Some(browser), path, Some(command.body.clone()));
        let version = self.version();
        let started = Instant::now();
        let exchange = client.request(wire).await;
        let mut status = None;
        let mut response_body_bytes = 0;
        let mut raw_response = None;
        let mut transport_error = None;
        let mut integrity_error = None;
        let mut uncertain = false;
        let mut outcome = "transport_failure";
        match exchange {
            Err(error) => {
                transport_error = Some(error.to_string());
                uncertain = command.edit;
            }
            Ok((code, _, bytes)) => {
                let code = code.as_u16();
                status = Some(code);
                response_body_bytes = bytes.len();
                match serde_json::from_slice::<Value>(&bytes) {
                    Err(error) => {
                        integrity_error = Some(format!("response JSON: {error}"));
                        uncertain = command.edit;
                        outcome = "invalid_response";
                    }
                    Ok(value) if code != 200 && !value["error"].is_string() => {
                        integrity_error = Some("non-success response lacks an error code".into());
                        uncertain = command.edit;
                        outcome = "invalid_response";
                    }
                    Ok(value) => match code {
                        200 => match self.accept(cell, &command, &value) {
                            Ok(()) => outcome = "success",
                            Err(error) => {
                                integrity_error = Some(error);
                                uncertain = command.edit;
                                outcome = "invalid_response";
                            }
                        },
                        429 => outcome = "busy",
                        408 => outcome = "request_deadline",
                        503 if !command.edit => outcome = "unavailable",
                        503 if value["error"] == "attempt_exhausted" => {
                            outcome = "attempt_exhausted"
                        }
                        503 => {
                            outcome = "uncertain";
                            uncertain = true;
                        }
                        404 if recovery && value["error"] == "operation_not_found" => {
                            outcome = "reconciled_absent"
                        }
                        _ => {
                            integrity_error = Some(format!("unexpected HTTP status {code}"));
                            uncertain = command.edit;
                            outcome = "invalid_response";
                        }
                    },
                }
                raw_response = Some(bytes);
            }
        }
        if recovery && outcome != "success" && outcome != "reconciled_absent" {
            uncertain = true;
            integrity_error.get_or_insert_with(|| "original operation was not reconciled".into());
        }
        let latency_ns =
            u64::try_from(started.elapsed().as_nanos()).expect("capacity client duration overflow");
        let response_base64 = raw_response.map(|bytes| base64(&bytes));
        let attempt = Attempt {
            phase: command.phase,
            ordinal: command.ordinal,
            slot: self.index,
            session: command.session,
            edit: command.edit,
            project: self.project.clone(),
            item: self.item.clone(),
            expected_item_version: version,
            request: command.body,
            canonical_base64: command.canonical,
            request_json_bytes,
            response_body_bytes,
            response_base64,
            status,
            latency_ns,
            outcome,
            transport_error,
            integrity_error,
            uncertain,
        };
        if uncertain {
            self.pending = Some(attempt.clone());
        }
        attempt
    }

    pub(super) async fn recover(
        &mut self,
        cell: Cell,
        client: &Client,
        browsers: &[Browser],
    ) -> Option<Attempt> {
        let original = self.pending.take()?;
        let browser = &browsers[original.session];
        let command = Command {
            phase: Phase::Recovery,
            ordinal: original.ordinal,
            edit: true,
            session: original.session,
            body: original.request,
            canonical: original.canonical_base64,
        };
        Some(self.send(cell, command, client, browser).await)
    }
}

pub(super) fn saved_json(saved: &SavedResult) -> Value {
    serde_json::from_slice(&work_item_codec::encode_result(saved, 1).unwrap()).unwrap()
}

pub(super) fn text(cell: Cell, variant: usize, field: usize) -> String {
    let mut result = String::with_capacity(cell.text_bytes);
    for block in 0..cell.text_bytes.div_ceil(64) {
        let input = format!("capacity:{}:{variant}:{field}:{block}", cell.index);
        let digest = ring::digest::digest(&ring::digest::SHA256, input.as_bytes());
        use std::fmt::Write;
        for byte in digest.as_ref() {
            write!(result, "{byte:02x}").unwrap();
        }
    }
    result.truncate(cell.text_bytes);
    result
}
