use crate::domain::{DomainResult, billing::Money, user::User};
use async_trait::async_trait;
use std::sync::Arc;

pub type DynPaymentGateway = Arc<dyn PaymentGateway + Send + Sync>;

/// Данные созданного счёта у платёжного провайдера.
#[derive(Debug, Clone)]
pub struct PaymentInvoice {
    /// URL для перехода к оплате счёта.
    pub payment_url: String,

    /// Идентификатор транзакции на стороне платёжного провайдера, если он доступен.
    pub provider_tx_id: Option<String>,
}

/// Актуальное состояние транзакции, полученное у провайдера авторизованным запросом.
#[derive(Debug, Clone)]
pub struct ProviderTransactionState {
    pub status: ProviderTxStatus,
    pub amount: Money,
    pub raw: serde_json::Value,
}

/// Статус транзакции, полученный напрямую от провайдера (не из тела вебхука).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderTxStatus {
    Pending,
    Confirmed,
    Canceled,
    /// Платёж подтверждён, а затем оспорен плательщиком постфактум. Требует ручного разбора — возможно, уже выданный доступ нужно отозвать.
    Chargebacked,
}

/// Порт для взаимодействия с платёжными провайдерами.
#[async_trait]
pub trait PaymentGateway: Send + Sync {
    /// Создаёт счёт на оплату у платёжного провайдера.
    ///
    /// `order_id` используется для идентификации заказа во внешней платёжной системе,
    /// а `payer_id` содержит идентификатор пользователя в системе.
    ///
    /// Возвращает данные созданного счёта, включая URL для оплаты
    /// и, если он предоставляется провайдером, идентификатор транзакции.
    async fn create_invoice(
        &self,
        order_id: &str,
        amount: Money,
        payer_id: &User,
    ) -> DomainResult<PaymentInvoice>;

    /// Запрашивает у провайдера актуальный статус транзакции.
    ///
    /// Используется вместо доверия телу вебхука.
    async fn get_status(&self, provider_tx_id: &str) -> DomainResult<ProviderTransactionState>;
}
