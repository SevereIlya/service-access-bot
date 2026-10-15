use crate::application::AppResult;
use crate::domain::billing::{
    DynPaymentGateway, DynTransactionRepository, ProviderTransactionState, ProviderTxStatus,
    Transaction, TransactionStatus,
};
use crate::domain::notification::DynNotifier;
use crate::domain::user::DynUserRepository;
use tracing::{error, info, warn};

pub struct ProcessPaymentWebhookCommand {
    transaction_repo: DynTransactionRepository,
    payment_gateway: DynPaymentGateway,
    user_repo: DynUserRepository,
    notifier: DynNotifier,
}

impl ProcessPaymentWebhookCommand {
    pub fn new(
        transaction_repo: DynTransactionRepository,
        payment_gateway: DynPaymentGateway,
        user_repo: DynUserRepository,
        notifier: DynNotifier,
    ) -> Self {
        Self {
            transaction_repo,
            payment_gateway,
            user_repo,
            notifier,
        }
    }

    pub async fn execute(&self, provider_tx_id: &str) -> AppResult<()> {
        let state: ProviderTransactionState =
            self.payment_gateway.get_status(provider_tx_id).await?;

        let Some(mut tx) = self.transaction_repo.find_by_provider_tx_id(provider_tx_id).await?
        else {
            warn!(
                provider_tx_id,
                "Вебхук по неизвестной транзакции провайдера"
            );
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
            tx.mark_failed(Some(state.raw));
            if let Err(e) = self.transaction_repo.update(&tx).await {
                error!(error = %e, order_id = tx.order_id(), "Не удалось сохранить факт недоплаты в БД");
            }
            error!(
                order_id = tx.order_id(),
                user_id = %tx.user_id(),
                expected = %tx.amount(),
                got = %state.amount,
                "Недоплата: провайдер подтвердил МЕНЬШЕ счёта - не зачислено, нужен ручной разбор"
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

        let should_notify_topup = matches!(state.status, ProviderTxStatus::Confirmed);

        match state.status {
            ProviderTxStatus::Pending => {
                info!(
                    order_id = tx.order_id(),
                    "Провайдер всё ещё считает платёж висящим"
                );
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
                error!(
                    order_id = tx.order_id(),
                    user_id = %tx.user_id(),
                    amount = %tx.amount(),
                    "Chargeback по висящему платежу - нужен ручной разбор, возможно отозвать доступ"
                );
            }
        }

        self.transaction_repo.update(&tx).await?;

        if should_notify_topup {
            self.notify_balance_topped_up(&tx).await;
        }

        Ok(())
    }

    async fn notify_balance_topped_up(&self, tx: &Transaction) {
        let user = match self.user_repo.find_by_user_id(tx.user_id()).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                warn!(user_id = %tx.user_id(), order_id = tx.order_id(), "Юзер для уведомления о пополнении не найден");
                return;
            }
            Err(e) => {
                warn!(error = %e, user_id = %tx.user_id(), "Не удалось загрузить юзера для уведомления");
                return;
            }
        };

        let new_balance = match self.transaction_repo.balance_for_user(tx.user_id()).await {
            Ok(balance) => balance,
            Err(e) => {
                warn!(error = %e, user_id = %tx.user_id(), "Не удалось посчитать баланс для уведомления");
                return;
            }
        };

        if let Err(e) =
            self.notifier.notify_balance_topped_up(&user, tx.amount(), new_balance).await
        {
            warn!(error = %e, order_id = tx.order_id(), "Не удалось отправить уведомление о пополнении");
        }
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
        device::{DeviceName, DeviceSetupCode},
        notification::Notifier,
        user::{ReferralCode, SubscriptionToken, TelegramId, User, UserId, UserRepository},
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

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
        balance: Money,
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
            Ok(self.balance)
        }
    }

    struct MockUserRepository {
        user: Option<User>,
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, _user: &User) -> DomainResult<UserId> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn update(&self, _user: &User) -> DomainResult<()> {
            unreachable!("update() не используется в этом юзкейсе")
        }
        async fn find_by_user_id(&self, _user_id: UserId) -> DomainResult<Option<User>> {
            Ok(self.user.clone())
        }
        async fn find_by_telegram_id(
            &self,
            _telegram_id: TelegramId,
        ) -> DomainResult<Option<User>> {
            unreachable!("find_by_telegram_id() не используется в этом юзкейсе")
        }
        async fn find_by_subscription_token(
            &self,
            _token: &SubscriptionToken,
        ) -> DomainResult<Option<User>> {
            unreachable!("find_by_subscription_token() не используется в этом юзкейсе")
        }
    }

    struct MockNotifier {
        notified: Arc<Mutex<Vec<(Money, Money)>>>,
    }

    #[async_trait]
    impl Notifier for MockNotifier {
        async fn notify_subscription_expired(&self, _user: &User) -> DomainResult<()> {
            unreachable!("notify_subscription_expired() не используется в этом юзкейсе")
        }
        async fn notify_subscription_expiring(
            &self,
            _user: &User,
            _expires_at: DateTime<Utc>,
        ) -> DomainResult<()> {
            unreachable!("notify_subscription_expiring() не используется в этом юзкейсе")
        }
        async fn notify_balance_topped_up(
            &self,
            _user: &User,
            amount: Money,
            new_balance: Money,
        ) -> DomainResult<()> {
            self.notified.lock().unwrap().push((amount, new_balance));
            Ok(())
        }
        async fn notify_new_device_detected(
            &self,
            _user: &User,
            _device_name: &DeviceName,
        ) -> DomainResult<()> {
            unreachable!("notify_new_device_detected() не используется в этом юзкейсе")
        }
        async fn notify_device_setup_code(
            &self,
            _user: &User,
            _device_name: &DeviceName,
            _code: &DeviceSetupCode,
        ) -> DomainResult<()> {
            unreachable!("notify_device_setup_code() не используется в этом юзкейсе")
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

    fn make_test_user() -> User {
        let mut user = User::new(
            TelegramId::new(7),
            Uuid::new_v4(),
            Some("test".into()),
            "Test User".into(),
            Money::new(1000).unwrap(),
            ReferralCode::new("REF".into()),
            SubscriptionToken::new("TOK".into()),
        );
        user.assign_id(UserId::new(7));
        user
    }

    #[allow(clippy::type_complexity)]
    fn setup(
        tx: Option<Transaction>,
        state: ProviderTransactionState,
    ) -> (
        ProcessPaymentWebhookCommand,
        Arc<Mutex<Vec<Transaction>>>,
        Arc<Mutex<Vec<(Money, Money)>>>,
    ) {
        let updated = Arc::new(Mutex::new(Vec::new()));
        let repo = Arc::new(MockTransactionRepository {
            tx,
            updated: updated.clone(),
            balance: Money::new(40_000).unwrap(),
        });
        let gateway: DynPaymentGateway = Arc::new(MockPaymentGateway { state });
        let user_repo: DynUserRepository = Arc::new(MockUserRepository {
            user: Some(make_test_user()),
        });
        let notified = Arc::new(Mutex::new(Vec::new()));
        let notifier: DynNotifier = Arc::new(MockNotifier {
            notified: notified.clone(),
        });
        (
            ProcessPaymentWebhookCommand::new(repo, gateway, user_repo, notifier),
            updated,
            notified,
        )
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_chargeback_after_success_keeps_status_and_saves_payload() {
        let (cmd, updated, notified) = setup(
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
        assert!(
            notified.lock().unwrap().is_empty(),
            "chargeback - не пополнение, уведомлять не о чем"
        );
    }

    #[tokio::test]
    async fn test_chargeback_while_pending_marks_failed() {
        let (cmd, updated, notified) = setup(
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
        assert!(notified.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_confirmed_marks_pending_success_and_notifies() {
        let (cmd, updated, notified) = setup(
            Some(make_tx(TransactionStatus::Pending)),
            make_state(ProviderTxStatus::Confirmed),
        );

        cmd.execute("platega-123").await.unwrap();

        let updated = updated.lock().unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].status(), TransactionStatus::Success);

        let notified = notified.lock().unwrap();
        assert_eq!(
            notified.len(),
            1,
            "подтверждённое пополнение обязано уведомить юзера"
        );
        assert_eq!(
            notified[0].0,
            Money::new(40_000).unwrap(),
            "уведомление должно нести именно сумму пополнения"
        );
    }

    #[tokio::test]
    async fn test_underpayment_marks_failed_and_saves_evidence() {
        let mut state = make_state(ProviderTxStatus::Confirmed);
        state.amount = Money::new(39_999).unwrap(); // меньше счёта в 40 000
        let (cmd, updated, notified) = setup(Some(make_tx(TransactionStatus::Pending)), state);

        cmd.execute("platega-123").await.unwrap();

        let updated = updated.lock().unwrap();
        assert_eq!(
            updated.len(),
            1,
            "недоплата обязана менять статус, а не висеть Pending вечно"
        );
        assert_eq!(
            updated[0].status(),
            TransactionStatus::Failed,
            "недоплаченное - не зачислено"
        );
        assert!(
            updated[0].payload().is_some(),
            "сырой ответ с реальной суммой - улика"
        );
        assert!(
            notified.lock().unwrap().is_empty(),
            "недоплата - не зачисление, уведомлять не о чем"
        );
    }
}
