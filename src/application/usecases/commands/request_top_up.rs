use crate::application::{AppError, AppResult};
use crate::domain::{
    DomainError, UserError,
    billing::{
        DynPaymentGateway, DynTransactionRepository, Money, PaymentProvider, Transaction,
        TransactionKind,
    },
    user::User,
};
use tracing::{error, info};
use uuid::Uuid;

pub struct RequestTopUpCommand {
    transaction_repo: DynTransactionRepository,
    payment_gateway: DynPaymentGateway,
}

impl RequestTopUpCommand {
    pub fn new(
        transaction_repo: DynTransactionRepository,
        payment_gateway: DynPaymentGateway,
    ) -> Self {
        Self {
            transaction_repo,
            payment_gateway,
        }
    }

    pub async fn execute(&self, user: &User, amount: Money) -> AppResult<String> {
        let user_id = user.id().ok_or(DomainError::User(UserError::EntityNotSaved))?;
        let kind = TransactionKind::TopUp;
        let order_id = format!("{}_{user_id}_{}", kind.as_str(), Uuid::new_v4().simple());

        let mut tx = Transaction::new(
            user_id,
            amount,
            kind,
            PaymentProvider::Platega,
            order_id.clone(),
        );
        self.transaction_repo.create(&tx).await?;

        let invoice = match self.payment_gateway.create_invoice(&order_id, amount, user).await {
            Ok(invoice) => invoice,
            Err(e) => {
                tx.mark_failed(None);
                if let Err(update_err) = self.transaction_repo.update(&tx).await {
                    error!(
                        error = %update_err,
                        order_id,
                        "Счёт не создан, и не удалось пометить транзакцию Failed - почистить вручную"
                    );
                }
                error!(
                    error = %e,
                    order_id,
                    user_id = %user_id,
                    amount = %amount,
                    "Platega не создал счёт - транзакция помечена Failed, пополнение не состоится"
                );
                return Err(e.into());
            }
        };

        let Some(provider_tx_id) = invoice.provider_tx_id else {
            tx.mark_failed(None);
            if let Err(update_err) = self.transaction_repo.update(&tx).await {
                error!(error = %update_err, order_id, "Не удалось пометить Failed транзакцию без provider_tx_id");
            }
            error!(order_id, user_id = %user_id, "Шлюз не вернул provider_tx_id - транзакция помечена Failed");
            return Err(AppError::Internal(
                "Платёжный шлюз не вернул идентификатор транзакции".into(),
            ));
        };

        tx.attach_provider_tx_id(provider_tx_id);
        if let Err(e) = self.transaction_repo.update(&tx).await {
            error!(error = %e, order_id, "Не удалось сохранить provider_tx_id, нужна ручная сверка");
        }

        info!(user_id = %user_id, order_id, "Счёт на пополнение создан");
        Ok(invoice.payment_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainResult,
        billing::{
            PaymentGateway, PaymentInvoice, ProviderTransactionState, TransactionId,
            TransactionRepository, TransactionStatus,
        },
        user::{ReferralCode, SubscriptionToken, TelegramId, UserId},
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // ==========================================
    // Моки
    // ==========================================

    struct MockPaymentGateway {
        /// None = create_invoice падает.
        invoice: Option<PaymentInvoice>,
    }

    #[async_trait]
    impl PaymentGateway for MockPaymentGateway {
        async fn create_invoice(
            &self,
            _order_id: &str,
            _amount: Money,
            _payer_id: &User,
        ) -> DomainResult<PaymentInvoice> {
            match &self.invoice {
                Some(invoice) => Ok(invoice.clone()),
                None => Err(DomainError::SystemFailure(
                    "Platega недоступен (тест)".into(),
                )),
            }
        }

        async fn get_status(
            &self,
            _provider_tx_id: &str,
        ) -> DomainResult<ProviderTransactionState> {
            unreachable!("get_status() не используется в этом юзкейсе")
        }
    }

    struct MockTransactionRepository {
        updated: Arc<Mutex<Vec<Transaction>>>,
    }

    #[async_trait]
    impl TransactionRepository for MockTransactionRepository {
        async fn create(&self, _tx: &Transaction) -> DomainResult<TransactionId> {
            Ok(TransactionId::new(1))
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
            unreachable!("find_by_provider_tx_id() не используется в этом юзкейсе")
        }
        async fn balance_for_user(&self, _user_id: UserId) -> DomainResult<Money> {
            unreachable!("balance_for_user() не используется в этом юзкейсе")
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    fn make_user() -> User {
        let mut user = User::new(
            TelegramId::new(555),
            Uuid::new_v4(),
            Some("tester".into()),
            "Test User".into(),
            Money::new(1000).unwrap(),
            ReferralCode::new("REF".into()),
            SubscriptionToken::new("TOK".into()),
        );
        user.assign_id(UserId::new(7));
        user
    }

    fn setup(
        invoice: Option<PaymentInvoice>,
    ) -> (RequestTopUpCommand, Arc<Mutex<Vec<Transaction>>>) {
        let updated = Arc::new(Mutex::new(Vec::new()));
        let repo: DynTransactionRepository = Arc::new(MockTransactionRepository {
            updated: updated.clone(),
        });
        let gateway: DynPaymentGateway = Arc::new(MockPaymentGateway { invoice });
        (RequestTopUpCommand::new(repo, gateway), updated)
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_happy_path_returns_url_and_saves_provider_tx_id() {
        let invoice = PaymentInvoice {
            payment_url: "https://pay.example/abc".into(),
            provider_tx_id: Some("platega-777".into()),
        };
        let (cmd, updated) = setup(Some(invoice));

        let url = cmd.execute(&make_user(), Money::new(50_000).unwrap()).await.unwrap();

        assert_eq!(url, "https://pay.example/abc");
        let updated = updated.lock().unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(
            updated[0].provider_tx_id(),
            Some("platega-777"),
            "id транзакции провайдера должен сохраниться"
        );
        assert_eq!(
            updated[0].status(),
            TransactionStatus::Pending,
            "до вебхука транзакция ожидает подтверждения"
        );
    }

    #[tokio::test]
    async fn test_invoice_failure_marks_transaction_failed() {
        let (cmd, updated) = setup(None); // шлюз падает

        let result = cmd.execute(&make_user(), Money::new(50_000).unwrap()).await;

        assert!(result.is_err(), "юзер должен увидеть сбой");
        let updated = updated.lock().unwrap();
        assert_eq!(
            updated.len(),
            1,
            "транзакция не должна остаться сиротой в Pending"
        );
        assert_eq!(updated[0].status(), TransactionStatus::Failed);
        assert_eq!(updated[0].provider_tx_id(), None);
    }

    #[tokio::test]
    async fn test_missing_provider_tx_id_marks_failed() {
        let invoice = PaymentInvoice {
            payment_url: "https://pay.example/abc".into(),
            provider_tx_id: None,
        };
        let (cmd, updated) = setup(Some(invoice));

        let result = cmd.execute(&make_user(), Money::new(50_000).unwrap()).await;

        assert!(result.is_err());
        let updated = updated.lock().unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].status(), TransactionStatus::Failed);
    }
}
