use super::*;

impl Engine {
    pub(super) async fn admit_management(
        &self,
        credential: &Credential,
        deadline: Instant,
    ) -> Result<(Arc<Session>, Authority, Verifier), Error> {
        let entry = self.candidate(credential)?;
        let credentials = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?
        .credentials(deadline)
        .await
        .map_err(|_| Error::Unavailable)?;
        let (proof, verifier) = verifier(credentials)?;
        let fresh = snapshot(&proof);
        self.csrf(&entry, credential, &fresh)?;
        if !entry.eligible(&fresh, Instant::now()) {
            return Err(Error::Unauthorized);
        }
        charge(&self.management_allowance)?;
        Ok((entry, proof, verifier))
    }

    pub(super) async fn revoke_others(
        &self,
        credential: Credential,
        password: NormalizedPassword,
        deadline: Instant,
    ) -> Result<Output, Error> {
        let (entry, proof, verifier) = self.admit_management(&credential, deadline).await?;
        if !self
            .hash(deadline, |worker| {
                worker.verify(deadline, password, verifier)
            })
            .await?
        {
            return Err(Error::Unauthorized);
        }
        let mut database = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        let mut transaction = database
            .fenced_password(&proof, deadline)
            .await
            .map_err(fence_error)?;
        let fresh = snapshot(transaction.authority());
        if !entry.eligible(&fresh, Instant::now()) {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(Error::Unauthorized);
        }
        let selection = self
            .sessions
            .lock()
            .map_err(|_| Error::Unavailable)
            .and_then(|mut registry| {
                registry
                    .revoke_others(&entry, &fresh, Instant::now())
                    .map_err(|error| match error {
                        crate::session::SessionError::Unknown => Error::Unauthorized,
                        _ => Error::Unavailable,
                    })
            });
        let targets = match selection {
            Ok(targets) => targets,
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| Error::Unavailable)?;
                return Err(error);
            }
        };
        for target in &targets {
            if transaction
                .revoke(&target.public_id, target.kind, &entry.public_id)
                .await
                .is_err()
            {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| Error::OtherAuditUnconfirmed)?;
                return Err(Error::OtherAuditUnconfirmed);
            }
        }
        let committed = transaction
            .commit()
            .await
            .map_err(|_| Error::OtherAuditUnconfirmed)?;
        self.sessions
            .lock()
            .map_err(|_| Error::OtherAuditUnconfirmed)?
            .confirm_revoke_others(&entry, &snapshot(&committed), &targets, Instant::now())
            .map_err(|_| Error::OtherAuditUnconfirmed)?;
        Ok(Output::OthersRevoked {
            count: targets.len(),
        })
    }
}
