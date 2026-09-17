use crate::application::AppResult;
use crate::domain::billing::{
    DynPaymentGateway, DynTransactionRepository, ProviderTransactionState, ProviderTxStatus,
    TransactionStatus,
};
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

        if tx.status() == TransactionStatus::Success
            && state.status == ProviderTxStatus::Chargebacked
        {
            tx.mark_success(Some(state.raw));
            if let Err(e) = self.transaction_repo.update(&tx).await {
                warn!(error = %e, order_id = tx.order_id(), "Не удалось сохранить payload chargeback'а");
            }
            error!(
                order_id = tx.order_id(),
                user_id = %tx.user_id(),
                amount = %tx.amount(),
                "CHARGEBACK по подтверждённому платежу - нужен ручной разбор (доступ/компенсация)"
            );
            return Ok(());
        }

        if !tx.is_pending() {
            info!(order_id = tx.order_id(), status = ?tx.status(), "Вебхук по уже обработанной транзакции");
            return Ok(());
        }

        if state.amount < tx.amount() {
            error!(
                order_id = tx.order_id(),
                expected = %tx.amount(),
                got = %state.amount,
                "Провайдер подтвердил сумму МЕНЬШЕ выставленного счёта."
            );
            return Ok(());
        }

        if state.amount != tx.amount() {
            info!(
                order_id = tx.order_id(),
                invoiced = %tx.amount(),
                confirmed = %state.amount,
                "Провайдер подтвердил сумму БОЛЬШЕ выставленного счёта."
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainResult,
        billing::{
            Money, PaymentGateway, PaymentInvoice, PaymentProvider, Transaction, TransactionId,
            TransactionKind, TransactionRepository,
        },
        user::{User, UserId},
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // ==========================================
    // Моки
    // ==========================================

    struct MockPaymentGateway {
        state: ProviderTransactionState,
    }

    #[async_trait]
    impl PaymentGateway for MockPaymentGateway {
        async fn create_invoice(
            &self,
            _order_id: &str,
            _amount: Money,
            _payer_id: &User,
        ) -> DomainResult<PaymentInvoice> {
            unreachable!("create_invoice() не используется в этом юзкейсе")
        }

        async fn get_status(
            &self,
            _provider_tx_id: &str,
        ) -> DomainResult<ProviderTransactionState> {
            Ok(self.state.clone())
        }
    }

    struct MockTransactionRepository {
        tx: Option<Transaction>,
        updated: Arc<Mutex<Vec<Transaction>>>,
    }

    #[async_trait]
    impl TransactionRepository for MockTransactionRepository {
        async fn create(&self, _tx: &Transaction) -> DomainResult<TransactionId> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn update(&self, tx: &Transaction) -> DomainResult<()> {
            self.updated.lock().unwrap().push(tx.clone());
            Ok(())
        }
        async fn find_by_order_id(&self, _order_id: &str) -> DomainResult<Option<Transaction>> {
            unreachable!("find_by_order_id() не используется в этом юзкейсе")
        }
        async fn find_by_provider_tx_id(
            &self,
            _provider_tx_id: &str,
        ) -> DomainResult<Option<Transaction>> {
            Ok(self.tx.clone())
        }
        async fn balance_for_user(&self, _user_id: UserId) -> DomainResult<Money> {
            unreachable!("balance_for_user() не используется в этом юзкейсе")
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    fn make_tx(status: TransactionStatus) -> Transaction {
        let mut tx = Transaction::new(
            UserId::new(7),
            Money::new(40_000).unwrap(),
            TransactionKind::TopUp,
            PaymentProvider::Platega,
            "top_up_7_test".into(),
        );
        tx.attach_provider_tx_id("platega-123".into());
        if status == TransactionStatus::Success {
            tx.mark_success(None);
        } else if status == TransactionStatus::Failed {
            tx.mark_failed(None);
        }
        tx
    }

    fn make_state(status: ProviderTxStatus) -> ProviderTransactionState {
        ProviderTransactionState {
            status,
            amount: Money::new(40_000).unwrap(),
            raw: serde_json::json!({ "id": "platega-123", "status": "TEST" }),
        }
    }

    fn setup(
        tx: Option<Transaction>,
        state: ProviderTransactionState,
    ) -> (ProcessPaymentWebhookCommand, Arc<Mutex<Vec<Transaction>>>) {
        let updated = Arc::new(Mutex::new(Vec::new()));
        let repo = Arc::new(MockTransactionRepository {
            tx,
            updated: updated.clone(),
        });
        let gateway: DynPaymentGateway = Arc::new(MockPaymentGateway { state });
        (ProcessPaymentWebhookCommand::new(repo, gateway), updated)
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_chargeback_after_success_keeps_status_and_saves_payload() {
        let (cmd, updated) = setup(
            Some(make_tx(TransactionStatus::Success)),
            make_state(ProviderTxStatus::Chargebacked),
        );

        cmd.execute("platega-123").await.unwrap();

        let updated = updated.lock().unwrap();
        assert_eq!(
            updated.len(),
            1,
            "факт chargeback-а должен сохраниться в payload"
        );
        assert_eq!(
            updated[0].status(),
            TransactionStatus::Success,
            "статус менять нельзя - ручной разбор, а не автоматическое списание"
        );
        assert!(
            updated[0].payload().is_some(),
            "сырой ответ провайдера - улика для разбора"
        );
    }

    #[tokio::test]
    async fn test_chargeback_while_pending_marks_failed() {
        let (cmd, updated) = setup(
            Some(make_tx(TransactionStatus::Pending)),
            make_state(ProviderTxStatus::Chargebacked),
        );

        cmd.execute("platega-123").await.unwrap();

        let updated = updated.lock().unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(
            updated[0].status(),
            TransactionStatus::Failed,
            "неоплаченный chargeback = отмена, как и раньше"
        );
    }

    #[tokio::test]
    async fn test_confirmed_marks_pending_success() {
        let (cmd, updated) = setup(
            Some(make_tx(TransactionStatus::Pending)),
            make_state(ProviderTxStatus::Confirmed),
        );

        cmd.execute("platega-123").await.unwrap();

        let updated = updated.lock().unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].status(), TransactionStatus::Success);
    }
}
