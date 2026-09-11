use crate::domain::{
    DomainResult,
    billing::{Money, Transaction, TransactionId},
    user::UserId,
};
use async_trait::async_trait;
use std::sync::Arc;

pub type DynTransactionRepository = Arc<dyn TransactionRepository + Send + Sync>;

/// Репозиторий для хранения и получения транзакций.
#[async_trait]
pub trait TransactionRepository: Send + Sync {
    /// Сохраняет новую транзакцию.
    ///
    /// Возвращает идентификатор созданной транзакции.
    async fn create(&self, tx: &Transaction) -> DomainResult<TransactionId>;

    /// Обновляет существующую транзакцию.
    async fn update(&self, tx: &Transaction) -> DomainResult<()>;

    /// Находит транзакцию по внутреннему идентификатору заказа.
    ///
    /// Возвращает [`None`], если транзакция с указанным `order_id` не найдена.
    async fn find_by_order_id(&self, order_id: &str) -> DomainResult<Option<Transaction>>;

    /// Находит транзакцию по идентификатору у платёжного провайдера.
    ///
    /// Этот метод используется при обработке вебхука: провайдер присылает только свой `id`, а не наш `order_id`.
    async fn find_by_provider_tx_id(&self, provider_tx_id: &str) -> DomainResult<Option<Transaction>>;

    /// Возвращает текущий баланс пользователя.
    async fn balance_for_user(&self, user_id: UserId) -> DomainResult<Money>;
}
