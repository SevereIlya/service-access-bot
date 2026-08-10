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
