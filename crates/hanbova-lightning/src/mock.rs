use async_trait::async_trait;
use chrono::{Duration, Utc};
use hanbova_core::SatoshiAmount;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    error::{LightningError, Result},
    models::{
        CreateInvoiceRequest, Invoice, LightningBalance, LightningPaymentStatus, PayInvoiceRequest,
        PaymentDetails,
    },
    traits::LightningProvider,
};

/// Development mock provider implementing `LightningProvider`.
///
/// Simulates BOLT11 invoice generation, settlement, and balance accounting.
///
/// Integration Note for Live Breez SDK:
/// -------------------------------------
/// To connect Breez SDK (Liquid or Greenlight node service):
/// 1. Initialize Breez SDK with API key and working directory.
/// 2. Implement `LightningProvider` wrapping `breez_sdk::BreezServices`.
/// 3. Pass payment notifications through webhook/event stream.
#[derive(Debug, Clone)]
pub struct MockLightningProvider {
    invoices: Arc<RwLock<HashMap<String, Invoice>>>,
    payments: Arc<RwLock<HashMap<String, PaymentDetails>>>,
    balance_sats: Arc<RwLock<u64>>,
}

impl Default for MockLightningProvider {
    fn default() -> Self {
        Self::new(100_000)
    }
}

impl MockLightningProvider {
    pub fn new(initial_balance_sats: u64) -> Self {
        Self {
            invoices: Arc::new(RwLock::new(HashMap::new())),
            payments: Arc::new(RwLock::new(HashMap::new())),
            balance_sats: Arc::new(RwLock::new(initial_balance_sats)),
        }
    }
}

#[async_trait]
impl LightningProvider for MockLightningProvider {
    async fn create_invoice(&self, request: CreateInvoiceRequest) -> Result<Invoice> {
        let hash = format!("ln_hash_{}", Uuid::new_v4().simple());
        let expiry = request.expiry_seconds.unwrap_or(3600);
        let now = Utc::now();
        let expires_at = now + Duration::seconds(expiry as i64);

        let bolt11 = format!(
            "lnbc{}n1pjhanbova{}mockinvoice",
            request.amount_sats.as_u64(),
            &hash[..8]
        );

        let invoice = Invoice {
            payment_hash: hash.clone(),
            bolt11,
            amount_sats: request.amount_sats,
            description: request.description,
            expires_at,
            is_paid: false,
            created_at: now,
        };

        let mut lock = self.invoices.write().await;
        lock.insert(hash, invoice.clone());

        Ok(invoice)
    }

    async fn pay_invoice(&self, request: PayInvoiceRequest) -> Result<PaymentDetails> {
        if !request.bolt11.starts_with("lnbc") {
            return Err(LightningError::InvalidInvoice("Missing lnbc prefix".into()));
        }

        // Mock payment details
        let hash = format!("ln_pay_{}", Uuid::new_v4().simple());
        let preimage = format!("preimage_{}", Uuid::new_v4().simple());
        let amount_sats = SatoshiAmount::from_sats(1000);
        let fee_sats = SatoshiAmount::from_sats(2);

        let mut balance = self.balance_sats.write().await;
        let total_cost = amount_sats.as_u64() + fee_sats.as_u64();
        if *balance < total_cost {
            return Err(LightningError::InsufficientBalance {
                requested: total_cost,
                available: *balance,
            });
        }
        *balance -= total_cost;

        let details = PaymentDetails {
            payment_hash: hash.clone(),
            preimage: Some(preimage),
            amount_sats,
            fee_sats,
            status: LightningPaymentStatus::Succeeded,
            completed_at: Some(Utc::now()),
        };

        let mut lock = self.payments.write().await;
        lock.insert(hash, details.clone());

        Ok(details)
    }

    async fn get_payment(&self, payment_hash: &str) -> Result<PaymentDetails> {
        let lock = self.payments.read().await;
        lock.get(payment_hash)
            .cloned()
            .ok_or_else(|| LightningError::InvoiceNotFound(payment_hash.to_string()))
    }

    async fn get_balance(&self) -> Result<LightningBalance> {
        let balance = *self.balance_sats.read().await;
        Ok(LightningBalance {
            total_sats: SatoshiAmount::from_sats(balance),
            spendable_sats: SatoshiAmount::from_sats(balance),
            receiving_capacity_sats: SatoshiAmount::from_sats(10_000_000),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_invoice_and_balance() {
        let provider = MockLightningProvider::new(50_000);
        let invoice = provider
            .create_invoice(CreateInvoiceRequest {
                amount_sats: SatoshiAmount::from_sats(5_000),
                description: "Test Lightning Invoice".to_string(),
                expiry_seconds: Some(1800),
            })
            .await
            .unwrap();

        assert_eq!(invoice.amount_sats.as_u64(), 5_000);
        assert!(invoice.bolt11.starts_with("lnbc5000"));

        let balance = provider.get_balance().await.unwrap();
        assert_eq!(balance.total_sats.as_u64(), 50_000);
    }

    #[tokio::test]
    async fn test_pay_invoice_success() {
        let provider = MockLightningProvider::new(50_000);
        let payment = provider
            .pay_invoice(PayInvoiceRequest {
                bolt11: "lnbc1000n1...mock".to_string(),
                max_fee_sats: Some(10),
            })
            .await
            .unwrap();

        assert_eq!(payment.status, LightningPaymentStatus::Succeeded);
        assert!(payment.preimage.is_some());
    }
}
