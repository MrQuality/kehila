//! Owner-bound selection preparation and retained historical reconstruction.
//!
//! These values establish representation and immutable-command agreement.
//! They confer no session, database freshness, grant or response-release proof.
use task_contract::{
    current_work::{
        decide_selection_command, CurrentWork, SelectionCommand, SelectionDecision, SelectionError,
    },
    relationship::Endpoint,
};

pub(crate) enum PreparationError {
    InvalidProof,
    StateExhausted,
    Domain(SelectionError),
    Representation(selection_codec::Error),
}
impl std::fmt::Debug for PreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProof => f.write_str("InvalidProof"),
            Self::StateExhausted => f.write_str("StateExhausted"),
            Self::Domain(error) => f.debug_tuple("Domain").field(error).finish(),
            Self::Representation(error) => f.debug_tuple("Representation").field(error).finish(),
        }
    }
}
impl From<selection_codec::Error> for PreparationError {
    fn from(error: selection_codec::Error) -> Self {
        Self::Representation(error)
    }
}
type Result<T> = std::result::Result<T, PreparationError>;

pub(crate) struct Canonical {
    installation: String,
    request: SelectionCommand,
    bytes: Vec<u8>,
    digest: [u8; 32],
}
impl Canonical {
    pub(crate) fn installation(&self) -> &str {
        &self.installation
    }
    pub(crate) fn request(&self) -> &SelectionCommand {
        &self.request
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn admit(
        installation: &str,
        actor: &str,
        request: SelectionCommand,
    ) -> Result<Self> {
        if !selection_codec::valid_identifier(installation)
            || !selection_codec::valid_identifier(actor)
            || request.user_id.0 != actor
        {
            return Err(PreparationError::InvalidProof);
        }
        let bytes = selection_codec::encode_request(&request, 1)?;
        let digest = selection_codec::fingerprint(&bytes);
        Ok(Self {
            installation: installation.into(),
            request,
            bytes,
            digest,
        })
    }

    pub(crate) fn prepare(
        &self,
        current: &CurrentWork,
        previous_ordinal: u64,
        target: Option<&Endpoint>,
    ) -> Result<Prepared> {
        if current.version > previous_ordinal
            || (current.version == 0 && current.selected.is_some())
            || (previous_ordinal == 0 && (current.version != 0 || current.selected.is_some()))
        {
            return Err(PreparationError::InvalidProof);
        }
        // The private database factory must establish current self/target
        // authority independently; a caller-supplied true is never such proof.
        let result = match decide_selection_command(current, target, None, true, &self.request) {
            SelectionDecision::Apply(result) => result,
            SelectionDecision::Reject(error) => return Err(PreparationError::Domain(error)),
            SelectionDecision::Replay(_) => return Err(PreparationError::InvalidProof),
        };
        let event = selection_codec::Event {
            ordinal: previous_ordinal
                .checked_add(1)
                .ok_or(PreparationError::StateExhausted)?,
            operation_id: self.request.operation_id.clone(),
            before: current.clone(),
            result,
        };
        let event_bytes = String::from_utf8(selection_codec::encode_event(&event, 1)?)
            .map_err(|_| PreparationError::InvalidProof)?;
        let result_bytes = selection_codec::encode_result(&event.result, 1)?;
        let accepted_reply = response("accepted", &result_bytes)?;
        Ok(Prepared {
            event,
            event_bytes,
            result_bytes,
            accepted_reply,
        })
    }

    // Reconstruct immutable version-one history without current target lookup,
    // current archive checks or current-domain acceptance of a new command.
    pub(crate) fn reconstruct(
        &self,
        history_codec: u32,
        history: &[u8],
        result_codec: u32,
        result: &[u8],
    ) -> Result<Historical> {
        let event = selection_codec::decode_event(history_codec, history)?;
        let result = selection_codec::decode_result(result_codec, result)?;
        if result != event.result {
            return Err(PreparationError::InvalidProof);
        }
        self.verify_event(&event)?;
        let reply = response("replayed", &selection_codec::encode_result(&result, 1)?)?;
        Ok(Historical { reply })
    }

    fn verify_event(&self, event: &selection_codec::Event) -> Result<()> {
        let after = &event.result.next;
        let before = &event.before;
        let expected_version = before
            .version
            .checked_add(u64::from(before.selected != after.selected));
        if event.ordinal == 0
            || (before.version == 0 && before.selected.is_some())
            || (after.version == 0 && after.selected.is_some())
            || before.version >= event.ordinal
            || (event.ordinal == 1 && (before.version != 0 || before.selected.is_some()))
            || event.operation_id != self.request.operation_id
            || before.user_id != self.request.user_id
            || before.version != self.request.expected_version
            || after.user_id != self.request.user_id
            || after.selected != self.request.requested
            || expected_version != Some(after.version)
        {
            return Err(PreparationError::InvalidProof);
        }
        Ok(())
    }
}

fn response(outcome: &str, result: &[u8]) -> Result<Vec<u8>> {
    let value = project_create_codec::parse_json(result)?;
    let selection = value.get("value").ok_or(PreparationError::InvalidProof)?;
    let bytes = serde_json::to_vec(&serde_json::json!({
        "codec_version":1,"outcome":outcome,"selection":selection,
    }))
    .map_err(|_| PreparationError::InvalidProof)?;
    if bytes.len() > selection_codec::MAX_WIRE_BYTES {
        return Err(PreparationError::InvalidProof);
    }
    Ok(bytes)
}

pub(crate) struct Prepared {
    event: selection_codec::Event,
    event_bytes: String,
    result_bytes: Vec<u8>,
    accepted_reply: Vec<u8>,
}
pub(crate) struct Historical {
    reply: Vec<u8>,
}
impl Prepared {
    pub(crate) fn event(&self) -> &selection_codec::Event {
        &self.event
    }
    pub(crate) fn history(&self) -> &str {
        &self.event_bytes
    }
    pub(crate) fn result(&self) -> &[u8] {
        &self.result_bytes
    }
    pub(crate) fn reply(&self) -> &[u8] {
        &self.accepted_reply
    }
}
impl Historical {
    pub(crate) fn reply(&self) -> &[u8] {
        &self.reply
    }
}

// Verify permanent attributed history without a current-domain decision or an
// expiring payload. Database/session/grant authority remains with the factory.
pub(crate) fn verify_retained_event(
    installation: &str,
    actor: &str,
    history_codec: u32,
    history: &[u8],
    digest: &[u8],
) -> Result<selection_codec::Event> {
    let event = selection_codec::decode_event(history_codec, history)?;
    let canonical = Canonical::admit(
        installation,
        actor,
        SelectionCommand {
            user_id: event.before.user_id.clone(),
            operation_id: event.operation_id.clone(),
            expected_version: event.before.version,
            requested: event.result.next.selected.clone(),
        },
    )?;
    if canonical.digest.as_slice() != digest {
        return Err(PreparationError::InvalidProof);
    }
    canonical.verify_event(&event)?;
    Ok(event)
}

#[cfg(test)]
mod tests;
