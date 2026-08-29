use async_trait::async_trait;
use crate::infrastructure::database::{NodeRow, SharedTransaction, SqlxExecutor};
use sqlx::PgPool;
use tracing::instrument;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::vpn::{Node, NodeRepository};
use crate::exec_query;

pub struct SqlxNodeRepository {
    executor: SqlxExecutor,
}

impl SqlxNodeRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self {
            executor: SqlxExecutor::Pool(pool),
        }
    }

    pub const fn transaction(tx: SharedTransaction) -> Self {
        Self {
            executor: SqlxExecutor::Transaction(tx),
        }
    }
}

#[async_trait]
impl NodeRepository for SqlxNodeRepository {
    #[instrument(skip(self))]
    async fn find_active_nodes(&self) -> DomainResult<Vec<Node>> {
        let query = sqlx::query_as!(
            NodeRow,
            r#"
            SELECT *
            FROM nodes
            WHERE is_active = true
            "#
        );

        let rows: Vec<NodeRow> = exec_query!(self.executor, query, fetch_all)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;

        rows.into_iter().map(TryInto::try_into).collect()
    }
}
