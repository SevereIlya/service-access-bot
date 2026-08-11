#[macro_export]
macro_rules! exec_query {
    ($executor:expr, $query:expr, $method:ident) => {
        match &$executor {
            SqlxExecutor::Pool(pool) => $query.$method(pool).await,
            SqlxExecutor::Transaction(tx_mutex) => {
                let mut lock = tx_mutex.lock().await;
                match lock.as_mut() {
                    Some(tx) => $query.$method(&mut **tx).await,
                    None => {
                        return Err(DomainError::SystemFailure(
                            "Транзакция закрыта".into(),
                        ));
                    }
                }
            }
        }
    };
}

#[macro_export]
macro_rules! in_transaction {
    ($uow:expr, |$tx:ident| $body:block) => {{
        let mut $tx = $uow.begin().await?;
        match (async { $body }).await {
            Ok(value) => {
                $tx.commit().await?;
                Ok(value)
            }
            Err(e) => {
                if let Err(rollback_error) = $tx.rollback().await {
                    tracing::error!(error = ?rollback_error, "rollback упал тоже");
                }
                Err(e)
            }
        }
    }};
}