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

        let invoice = self.payment_gateway.create_invoice(&order_id, amount, user).await?;

        let Some(provider_tx_id) = invoice.provider_tx_id else {
            error!(order_id, "Шлюз не вернул provider_tx_id");
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
