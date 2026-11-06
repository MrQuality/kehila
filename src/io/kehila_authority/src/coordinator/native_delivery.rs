//! Session eligibility and bounded delivery shared by native product commands.
use super::*;
use serde_json::Value;

pub(crate) struct NativeDelivery {
    status: u16,
    body: Arc<Zeroizing<Vec<u8>>>,
    entry: Arc<Session>,
    snapshot: Snapshot,
    deadline: Instant,
}
pub(crate) struct NativeMaterial {
    pub(crate) status: u16,
    pub(crate) body: Arc<Zeroizing<Vec<u8>>>,
}
impl NativeDelivery {
    pub(crate) fn check(&self) -> Result<(), Error> {
        if Instant::now() >= self.deadline || !self.entry.eligible(&self.snapshot, Instant::now()) {
            Err(Error::Unauthorized)
        } else {
            Ok(())
        }
    }
    pub(crate) fn material(&self) -> Result<NativeMaterial, Error> {
        self.check()?;
        Ok(NativeMaterial {
            status: self.status,
            body: self.body.clone(),
        })
    }
}
pub(super) struct Reply {
    pub(super) status: u16,
    pub(super) value: Value,
    pub(super) business: bool,
}
pub(super) fn reply(status: u16, value: Value) -> Reply {
    Reply {
        status,
        value,
        business: status < 400,
    }
}
// Sealed admitted envelope. Item attempt packets construct it before mutation.
#[derive(Clone)]
pub(super) struct EncodedReply {
    status: u16,
    body: Arc<Zeroizing<Vec<u8>>>,
    business: bool,
}
impl EncodedReply {
    pub(super) fn encoded_len(&self) -> usize {
        self.body.len()
    }
    #[cfg(test)]
    pub(super) fn test_parts(&self) -> (u16, &[u8]) {
        (self.status, self.body.as_slice())
    }
    pub(super) fn admit(reply: Reply) -> Result<Self, Error> {
        let body = Arc::new(crate::project::wire::encode(&reply.value)?);
        Ok(Self {
            status: reply.status,
            body,
            business: reply.business,
        })
    }
}
impl Engine {
    pub(super) fn project_eligibility(
        &self,
        entry: &Session,
        credential: &Credential,
        fresh: &Snapshot,
    ) -> Result<(), Error> {
        if credential.kind != Kind::Browser {
            return Err(Error::Unauthorized);
        }
        self.csrf(entry, credential, fresh)
            .map_err(|_| Error::Project(crate::project::Failure::Forbidden))?;
        if entry.eligible(fresh, Instant::now()) {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }
    pub(super) fn native_delivery(
        &self,
        entry: Arc<Session>,
        fresh: Snapshot,
        reply: Reply,
        deadline: Instant,
    ) -> Result<NativeDelivery, Error> {
        self.check(deadline)?;
        self.native_delivery_encoded(entry, fresh, EncodedReply::admit(reply)?, deadline)
    }
    pub(super) fn native_delivery_encoded(
        &self,
        entry: Arc<Session>,
        fresh: Snapshot,
        reply: EncodedReply,
        deadline: Instant,
    ) -> Result<NativeDelivery, Error> {
        self.check(deadline)?;
        if !entry.eligible(&fresh, Instant::now())
            || (reply.business && !entry.successful_business(&fresh, Instant::now()))
        {
            return Err(Error::Unauthorized);
        }
        self.check(deadline)?;
        Ok(NativeDelivery {
            status: reply.status,
            body: reply.body,
            entry,
            snapshot: fresh,
            deadline,
        })
    }
}
