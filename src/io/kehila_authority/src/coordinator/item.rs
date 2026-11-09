use super::*;

impl Engine {
    pub(super) async fn item(
        &self,
        credential: Credential,
        input: crate::item_ingress::Input,
        deadline: Instant,
    ) -> Result<Output, Error> {
        self.check(deadline)?;
        let entry = self.candidate(&credential)?;
        #[cfg(test)]
        let wait = capacity::span(capacity::Stage::ReadMutex);
        let mut database = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        #[cfg(test)]
        if let Some(wait) = wait {
            wait.complete();
        }
        #[cfg(test)]
        let service = capacity::span(capacity::Stage::ReadService);
        let binding = &entry.binding;
        let mut transaction = database
            .read_snapshot(
                &binding.installation,
                binding.runtime_generation,
                binding.auth_generation,
                deadline,
            )
            .await
            .map_err(fence_error)?;
        let fresh = snapshot(transaction.authority());
        if let Err(error) = self.project_eligibility(&entry, &credential, &fresh) {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(error);
        }
        let outcome = transaction.read_items(input).await;
        transaction
            .rollback()
            .await
            .map_err(|_| Error::Unavailable)?;
        #[cfg(test)]
        if let Some(service) = service {
            service.complete();
        }
        drop(database);
        let value = outcome.map_err(|error| super::project::native_error(&self.live, error))?;
        Ok(Output::Item(self.native_delivery(
            entry,
            fresh,
            reply(200, value),
            deadline,
        )?))
    }
}
