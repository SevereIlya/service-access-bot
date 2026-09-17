use crate::domain::{
    DomainError, DomainResult,
    billing::{Money, PaymentGateway, PaymentInvoice, ProviderTransactionState, ProviderTxStatus},
    user::User,
};
use crate::infrastructure::PlategaConfig;
use async_trait::async_trait;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct PlategaGateway {
    client: Client,
    config: PlategaConfig,
}

impl PlategaGateway {
    pub fn new(config: PlategaConfig) -> anyhow::Result<Self> {
        Ok(Self {
            client: Client::builder().timeout(Duration::from_secs(10)).build()?,
            config,
        })
    }
}

// === DTO ===

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateInvoiceRequest {
    payment_details: PaymentDetailsDto,
    description: String,
    r#return: String,
    failed_url: String,
    order_id: String,
    metadata: MetadataDto,
}

#[derive(Debug, Serialize, Deserialize)]
struct PaymentDetailsDto {
    amount: f64,
    currency: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MetadataDto {
    user_id: String,
    user_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInvoiceResponseV2 {
    transaction_id: String,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransactionStatusDto {
    status: PlategaStatus,
    payment_details: PaymentDetailsDto,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum PlategaStatus {
    Pending,
    Canceled,
    Confirmed,
    Chargebacked,
}

impl From<PlategaStatus> for ProviderTxStatus {
    fn from(status: PlategaStatus) -> Self {
        match status {
            PlategaStatus::Pending => Self::Pending,
            PlategaStatus::Canceled => Self::Canceled,
            PlategaStatus::Confirmed => Self::Confirmed,
            PlategaStatus::Chargebacked => Self::Chargebacked,
        }
    }
}

#[async_trait]
impl PaymentGateway for PlategaGateway {
    async fn create_invoice(
        &self,
        order_id: &str,
        amount: Money,
        payer: &User,
    ) -> DomainResult<PaymentInvoice> {
        let body = CreateInvoiceRequest {
            payment_details: PaymentDetailsDto {
                amount: amount.as_rubles_f64(),
                currency: "RUB".to_string(),
            },
            description: "Пополнение баланса Paralink VPN".to_string(),
            r#return: self.config.success_url.clone(),
            failed_url: self.config.failed_url.clone(),
            order_id: order_id.to_string(),
            metadata: MetadataDto {
                user_id: payer.telegram_id().inner().to_string(),
                user_name: payer.username().unwrap_or_else(|| payer.full_name().to_string()),
            },
        };

        let url = format!("{}/v2/transaction/process", self.config.base_url);
        let response: Response = self
            .client
            .post(&url)
            .header("X-MerchantId", &self.config.merchant_id)
            .header("X-Secret", &self.config.secret)
            .json(&body)
            .send()
            .await
            .map_err(|e| DomainError::SystemFailure(format!("Platega: HTTP-ошибка создания счёта: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(DomainError::SystemFailure(format!(
                "Platega вернул {status} при создании счёта: {text}"
            )));
        }

        let parsed: CreateInvoiceResponseV2 = response
            .json()
            .await
            .map_err(|e| DomainError::SystemFailure(format!("Platega: не удалось распарсить ответ: {e}")))?;

        Ok(PaymentInvoice {
            payment_url: parsed.url,
            provider_tx_id: Some(parsed.transaction_id),
        })
    }

    async fn get_status(&self, provider_tx_id: &str) -> DomainResult<ProviderTransactionState> {
        let url = format!("{}/transaction/{provider_tx_id}", self.config.base_url);
        let response: Response = self
            .client
            .get(&url)
            .header("X-MerchantId", &self.config.merchant_id)
            .header("X-Secret", &self.config.secret)
            .send()
            .await
            .map_err(|e| DomainError::SystemFailure(format!("Platega: HTTP-ошибка запроса статуса: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(DomainError::SystemFailure(format!(
                "Platega вернул {status} по статусу {provider_tx_id}: {text}"
            )));
        }

        let raw: serde_json::Value = response
            .json()
            .await
            .map_err(|e| DomainError::SystemFailure(format!("Platega: статус - не JSON: {e}")))?;

        let parsed: TransactionStatusDto = serde_json::from_value(raw.clone())
            .map_err(|e| DomainError::SystemFailure(format!("Platega: неожиданная форма ответа статуса: {e}")))?;

        Ok(ProviderTransactionState {
            status: parsed.status.into(),
            amount: Money::try_from_rubles_f64(parsed.payment_details.amount)?,
            raw,
        })
    }
}
