use crate::domain::{
    billing::{Money, PaymentProvider, TransactionId, TransactionKind, TransactionStatus},
    user::UserId,
};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Transaction {
    id: Option<TransactionId>,
    user_id: UserId,
    amount: Money,
    kind: TransactionKind,
    status: TransactionStatus,
    provider: PaymentProvider,
    order_id: String,
    provider_tx_id: Option<String>,
    payload: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
}

impl Transaction {
    /// Создаёт новую транзакцию в статусе [`TransactionStatus::Pending`].
    ///
    /// Транзакция считается ожидающей подтверждения от платёжного провайдера.
    /// Идентификатор транзакции, идентификатор операции у провайдера и payload изначально отсутствуют.
    #[must_use]
    pub fn new(
        user_id: UserId,
        amount: Money,
        kind: TransactionKind,
        provider: PaymentProvider,
        order_id: String,
    ) -> Self {
        Self {
            id: None,
            user_id,
            amount,
            kind,
            status: TransactionStatus::Pending,
            provider,
            order_id,
            provider_tx_id: None,
            payload: None,
            created_at: Utc::now(),
        }
    }

    /// Создаёт успешно завершённую внутреннюю транзакцию.
    ///
    /// Такая транзакция не требует подтверждения от внешнего платёжного провайдера.
    /// В качестве провайдера автоматически используется [`PaymentProvider::Internal`],
    /// а статус устанавливается в [`TransactionStatus::Success`].
    #[must_use]
    pub fn new_completed(
        user_id: UserId,
        amount: Money,
        kind: TransactionKind,
        order_id: String,
    ) -> Self {
        let mut tx = Self::new(user_id, amount, kind, PaymentProvider::Internal, order_id);
        tx.status = TransactionStatus::Success;
        tx
    }

    /// Восстанавливает транзакцию из сохранённых данных.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn restore_from_db(
        id: TransactionId,
        user_id: UserId,
        amount: Money,
        kind: TransactionKind,
        status: TransactionStatus,
        provider: PaymentProvider,
        order_id: String,
        provider_tx_id: Option<String>,
        payload: Option<serde_json::Value>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Some(id),
            user_id,
            amount,
            kind,
            status,
            provider,
            order_id,
            provider_tx_id,
            payload,
            created_at,
        }
    }

    #[must_use]
    pub const fn id(&self) -> Option<TransactionId> {
        self.id
    }
    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }
    #[must_use]
    pub const fn amount(&self) -> Money {
        self.amount
    }
    #[must_use]
    pub const fn kind(&self) -> TransactionKind {
        self.kind
    }
    #[must_use]
    pub const fn status(&self) -> TransactionStatus {
        self.status
    }
    #[must_use]
    pub const fn provider(&self) -> PaymentProvider {
        self.provider
    }
    #[must_use]
    pub fn order_id(&self) -> &str {
        &self.order_id
    }
    #[must_use]
    pub fn provider_tx_id(&self) -> Option<&str> {
        self.provider_tx_id.as_deref()
    }
    #[must_use]
    pub const fn payload(&self) -> Option<&serde_json::Value> {
        self.payload.as_ref()
    }
    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self.status, TransactionStatus::Pending)
    }

    /// Назначает идентификатор транзакции.
    pub const fn assign_id(&mut self, id: TransactionId) {
        self.id = Some(id);
    }

    /// Сохраняет идентификатор транзакции у внешнего провайдера.
    pub fn attach_provider_tx_id(&mut self, id: String) {
        self.provider_tx_id = Some(id);
    }

    /// Переводит транзакцию в статус [`TransactionStatus::Success`].
    ///
    /// Если передан `payload`, он сохраняется в транзакции.
    /// `None` не очищает ранее сохранённый payload.
    pub fn mark_success(&mut self, payload: Option<serde_json::Value>) {
        self.status = TransactionStatus::Success;
        if payload.is_some() {
            self.payload = payload;
        }
    }

    /// Переводит транзакцию в статус [`TransactionStatus::Failed`].
    ///
    /// Если передан `payload`, он сохраняется в транзакции.
    /// `None` не очищает ранее сохранённый payload.
    pub fn mark_failed(&mut self, payload: Option<serde_json::Value>) {
        self.status = TransactionStatus::Failed;
        if payload.is_some() {
            self.payload = payload;
        }
    }
}
