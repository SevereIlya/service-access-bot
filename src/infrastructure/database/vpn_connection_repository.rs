use crate::domain::error::{DomainError, DomainResult};
use crate::domain::user::UserId;
use crate::domain::vpn::{NodeId, VpnConnection, VpnConnectionRepository};
use crate::exec_query;
use crate::infrastructure::database::{
    SharedTransaction, SqlxExecutor, VpnConnectionRow,
};
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::instrument;

pub struct SqlxVpnConnectionRepository {
    executor: SqlxExecutor,
}

impl SqlxVpnConnectionRepository {
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
impl VpnConnectionRepository for SqlxVpnConnectionRepository {
    #[instrument(skip(self, connection), fields(user_id = %connection.user_id(), node_id = %connection.node_id()))]
    async fn upsert(&self, connection: &VpnConnection) -> DomainResult<()> {
        let query = sqlx::query!(
            r#"
            INSERT INTO vpn_connections (user_id, node_id, is_synced, created_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id, node_id)
            DO UPDATE SET is_synced = EXCLUDED.is_synced
            "#,
            connection.user_id().inner(),
            connection.node_id().inner(),
            connection.is_synced(),
            connection.created_at()
        );

        exec_query!(self.executor, query, execute)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn find_by_user_and_node(
        &self,
        user_id: UserId,
        node_id: NodeId,
    ) -> DomainResult<Option<VpnConnection>> {
        let query = sqlx::query_as!(
            VpnConnectionRow,
            r#"
            SELECT *
            FROM vpn_connections
            WHERE user_id = $1
              AND node_id = $2
            "#,
            user_id.inner(),
            node_id.inner(),
        );

        let row: Option<VpnConnectionRow> =
            exec_query!(self.executor, query, fetch_optional)
                .map_err(|e| DomainError::SystemFailure(e.to_string()))?;

        row.map(TryInto::try_into).transpose()
    }
}
