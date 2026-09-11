use crate::application::AppResult;
use crate::domain::billing::{DynPaymentGateway, DynTransactionRepository, ProviderTransactionState, ProviderTxStatus};
use tracing::{error, info, warn};

pub struct ProcessPaymentWebhookCommand {
    transaction_repo: DynTransactionRepository,
    payment_gateway: DynPaymentGateway,
}
impl ProcessPaymentWebhookCommand {
    pub fn new(
        transaction_repo: DynTransactionRepository,
        payment_gateway: DynPaymentGateway,
    ) -> Self {
        Self {
            transaction_repo,
            payment_gateway,
        }
    }

    pub async fn execute(&self, provider_tx_id: &str) -> AppResult<()> {
        let state: ProviderTransactionState = self.payment_gateway.get_status(provider_tx_id).await?;

        let Some(mut tx) = self.transaction_repo.find_by_provider_tx_id(provider_tx_id).await? else {
            warn!(provider_tx_id, "Вебхук по неизвестной транзакции провайдера");
            return Ok(());
        };

        if !tx.is_pending() {
            info!(order_id = tx.order_id(), status = ?tx.status(), "Вебхук по уже обработанной транзакции");
            return Ok(());
        }

        if state.amount != tx.amount() {
            error!(
                order_id = tx.order_id(),
                expected = %tx.amount(),
                got = %state.amount,
                "Сумма от провайдера не совпадает с суммой транзакции. Требуется ручной разбор"
            );
            return Ok(());
        }

        match state.status {
            ProviderTxStatus::Pending => {
                info!(order_id = tx.order_id(), "Провайдер всё ещё считает платёж висящим");
                return Ok(());
            }
            ProviderTxStatus::Confirmed => {
                tx.mark_success(Some(state.raw));
                info!(order_id = tx.order_id(), "Пополнение зачислено");
            }
            ProviderTxStatus::Canceled => {
                tx.mark_failed(Some(state.raw));
                info!(order_id = tx.order_id(), "Платёж отменён провайдером");
            }
            ProviderTxStatus::Chargebacked => {
                tx.mark_failed(Some(state.raw));
                error!(order_id = tx.order_id(), "Chargeback - нужен ручной разбор, возможно отозвать доступ");
            }
        }

        self.transaction_repo.update(&tx).await?;
        Ok(())
    }
}
