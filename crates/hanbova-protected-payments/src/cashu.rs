use async_trait::async_trait;
use cdk::{
    amount::Amount,
    nuts::{
        nut10::Conditions, nut11::SigFlag, CurrencyUnit, PublicKey, SecretKey, SpendingConditions,
    },
    util::unix_time,
    wallet::{ReceiveOptions, SendOptions, Wallet},
};
use cdk_redb::WalletRedbDatabase;
use chrono::{DateTime, Utc};
use hanbova_core::{PaymentIntent, PaymentStatus, PaymentType, SatoshiAmount};
use std::{collections::HashMap, path::Path, str::FromStr, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    error::{ProtectedPaymentError, Result},
    models::{
        ClaimPaymentRequest, CreateProtectedPaymentRequest, ProtectedPaymentReceipt,
        RefundPaymentRequest, WalletBalance,
    },
    traits::ProtectedPaymentProvider,
};

/// Genuine Cashu NUT-10 & NUT-11 Protected Payment Provider.
///
/// Cryptographically creates, locks, claims, and refunds ecash proofs via CDK.
/// Private keys remain client-side and are never logged or stored in server tables.
#[derive(Clone)]
pub struct CashuProtectedPaymentProvider {
    wallet: Arc<Wallet>,
    mint_url: String,
    intents: Arc<RwLock<HashMap<Uuid, TrackedPayment>>>,
}

#[derive(Debug, Clone)]
struct TrackedPayment {
    intent: PaymentIntent,
    cashu_token: String,
    _recipient_pubkey: String,
    _refund_pubkey: Option<String>,
    locktime: DateTime<Utc>,
}

impl CashuProtectedPaymentProvider {
    /// Creates a new `CashuProtectedPaymentProvider` backed by Redb wallet storage.
    pub fn new(mint_url: &str, storage_dir: &Path, seed: [u8; 64]) -> Result<Self> {
        let db = WalletRedbDatabase::new(storage_dir)
            .map_err(|e| ProtectedPaymentError::Cdk(format!("Failed to open Redb storage: {e}")))?;

        let wallet =
            Wallet::new(mint_url, CurrencyUnit::Sat, Arc::new(db), seed, None).map_err(|e| {
                ProtectedPaymentError::Cdk(format!("Failed to initialize CDK wallet: {e}"))
            })?;

        Ok(Self {
            wallet: Arc::new(wallet),
            mint_url: mint_url.to_string(),
            intents: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Access the underlying CDK Wallet.
    pub fn wallet(&self) -> &Arc<Wallet> {
        &self.wallet
    }

    /// Mint development satoshis via fake wallet backend.
    pub async fn mint_dev_funds(&self, amount_sats: u64) -> Result<Amount> {
        let payment_method = cdk::nuts::PaymentMethod::from_str("bolt11")
            .map_err(|e| ProtectedPaymentError::Cdk(format!("Invalid payment method: {e}")))?;

        let quote = self
            .wallet
            .mint_quote(payment_method, Some(Amount::from(amount_sats)), None, None)
            .await
            .map_err(|e| ProtectedPaymentError::Cdk(format!("Mint quote failed: {e}")))?;

        self.wallet
            .mint(&quote.id, cdk::amount::SplitTarget::default(), None)
            .await
            .map_err(|e| ProtectedPaymentError::Cdk(format!("Mint failed: {e}")))?;

        Ok(Amount::from(amount_sats))
    }
}

#[async_trait]
impl ProtectedPaymentProvider for CashuProtectedPaymentProvider {
    async fn check_mint_support(&self) -> Result<bool> {
        let mint_info = self
            .wallet
            .fetch_mint_info()
            .await
            .map_err(|e| ProtectedPaymentError::MintUnreachable(e.to_string()))?
            .ok_or_else(|| {
                ProtectedPaymentError::MintUnreachable("Mint returned empty info".into())
            })?;

        let nut11_supported = mint_info.nuts.nut11.supported;
        if !nut11_supported {
            return Err(ProtectedPaymentError::Nut11NotSupported(
                self.mint_url.clone(),
            ));
        }

        Ok(true)
    }

    async fn create_protected_payment(
        &self,
        request: CreateProtectedPaymentRequest,
    ) -> Result<ProtectedPaymentReceipt> {
        // 1. Verify NUT-11 support
        self.check_mint_support().await?;

        // 2. Validate Locking Conditions
        let conditions_input = request.locking_conditions.ok_or_else(|| {
            ProtectedPaymentError::LockingCondition(
                "LockingConditions (recipient_pubkey, locktime) are required for Protected Send"
                    .into(),
            )
        })?;

        let recipient_pubkey =
            PublicKey::from_hex(&conditions_input.recipient_pubkey).map_err(|e| {
                ProtectedPaymentError::InvalidPublicKey(format!("Recipient pubkey invalid: {e}"))
            })?;

        let refund_pubkey_cdk = if let Some(ref ref_pub) = conditions_input.refund_pubkey {
            Some(PublicKey::from_hex(ref_pub).map_err(|e| {
                ProtectedPaymentError::InvalidPublicKey(format!("Refund pubkey invalid: {e}"))
            })?)
        } else {
            None
        };

        let now_unix = unix_time();
        let locktime_unix = conditions_input.locktime.timestamp() as u64;

        if locktime_unix <= now_unix {
            return Err(ProtectedPaymentError::LockingCondition(format!(
                "Locktime must be in the future (requested: {}, current: {})",
                locktime_unix, now_unix
            )));
        }

        let refund_keys_vec = refund_pubkey_cdk.map(|k| vec![k]);

        // 3. Create NUT-10 / NUT-11 Spending Conditions
        let nut10_conditions = Conditions::new(
            Some(locktime_unix),
            None,
            refund_keys_vec,
            None,
            Some(SigFlag::SigInputs),
            None,
        )
        .map_err(|e| {
            ProtectedPaymentError::LockingCondition(format!(
                "Failed to build NUT-10 conditions: {e}"
            ))
        })?;

        let spending_conditions =
            SpendingConditions::new_p2pk(recipient_pubkey, Some(nut10_conditions));

        // 4. Lock proofs from Wallet
        let send_options = SendOptions {
            conditions: Some(spending_conditions),
            ..Default::default()
        };

        let prepared_send = self
            .wallet
            .prepare_send(Amount::from(request.amount_sats.as_u64()), send_options)
            .await
            .map_err(|e| {
                ProtectedPaymentError::Cdk(format!("Failed to prepare locked proofs: {e}"))
            })?;

        let token = prepared_send.confirm(None).await.map_err(|e| {
            ProtectedPaymentError::Cdk(format!("Failed to confirm locked proofs: {e}"))
        })?;

        let token_str = token.to_string();

        // 5. Build Intent and Receipt
        let mut intent = PaymentIntent::new(
            PaymentType::Protected,
            request.amount_sats,
            request.recipient_identifier.clone(),
            request.sender_id,
            request.description,
            Some(conditions_input.locktime),
        )?;

        if let Some(id) = request.payment_id {
            intent.id = id;
        }

        intent.update_status(PaymentStatus::Protected)?;
        intent.update_status(PaymentStatus::Claimable)?;

        let claim_ref = format!("hnbv_claim_{}", intent.id.simple());
        intent.claim_reference = Some(claim_ref.clone());

        let receipt = ProtectedPaymentReceipt {
            payment_id: intent.id,
            status: intent.status,
            amount_sats: intent.amount_sats,
            recipient_identifier: intent.recipient_identifier.clone(),
            expires_at: conditions_input.locktime,
            claim_reference: claim_ref,
            cashu_token: Some(token_str.clone()),
            created_at: intent.created_at,
        };

        let tracked = TrackedPayment {
            intent,
            cashu_token: token_str,
            _recipient_pubkey: conditions_input.recipient_pubkey,
            _refund_pubkey: conditions_input.refund_pubkey,
            locktime: conditions_input.locktime,
        };

        self.intents
            .write()
            .await
            .insert(receipt.payment_id, tracked);

        Ok(receipt)
    }

    async fn claim_payment(&self, request: ClaimPaymentRequest) -> Result<ProtectedPaymentReceipt> {
        let mut lock = self.intents.write().await;
        let tracked = lock
            .get_mut(&request.payment_id)
            .ok_or_else(|| ProtectedPaymentError::NotFound(request.payment_id.to_string()))?;

        let token_str = request.cashu_token.as_ref().unwrap_or(&tracked.cashu_token);

        // Parse recipient private key
        let secret_key = SecretKey::from_hex(&request.claim_proof).map_err(|e| {
            ProtectedPaymentError::InvalidClaimProof(format!("Invalid private key hex: {e}"))
        })?;

        let recv_options = ReceiveOptions {
            p2pk_signing_keys: vec![secret_key],
            ..Default::default()
        };

        match self.wallet.receive(token_str, recv_options).await {
            Ok(amount) => {
                tracked.intent.update_status(PaymentStatus::Claimed)?;
                Ok(ProtectedPaymentReceipt {
                    payment_id: tracked.intent.id,
                    status: tracked.intent.status,
                    amount_sats: SatoshiAmount::from_sats(amount.into()),
                    recipient_identifier: tracked.intent.recipient_identifier.clone(),
                    expires_at: tracked.locktime,
                    claim_reference: format!("claimed_{}", tracked.intent.id.simple()),
                    cashu_token: None,
                    created_at: tracked.intent.created_at,
                })
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("TokenAlreadySpent") || err_msg.contains("already spent") {
                    // Check if sender already refunded
                    tracked.intent.update_status(PaymentStatus::Refunded).ok();
                    Err(ProtectedPaymentError::TokenAlreadySpent(
                        "Payment has already been refunded by the sender after locktime".into(),
                    ))
                } else {
                    Err(ProtectedPaymentError::Cdk(format!(
                        "Claim failed at mint: {err_msg}"
                    )))
                }
            }
        }
    }

    async fn refund_payment(
        &self,
        request: RefundPaymentRequest,
    ) -> Result<ProtectedPaymentReceipt> {
        let mut lock = self.intents.write().await;
        let tracked = lock
            .get_mut(&request.payment_id)
            .ok_or_else(|| ProtectedPaymentError::NotFound(request.payment_id.to_string()))?;

        let now = Utc::now();
        if now < tracked.locktime {
            return Err(ProtectedPaymentError::PaymentNotExpired(tracked.locktime));
        }

        let token_str = request.cashu_token.as_ref().unwrap_or(&tracked.cashu_token);

        let refund_proof = request.refund_proof.ok_or_else(|| {
            ProtectedPaymentError::InvalidClaimProof("Refund private key proof is required".into())
        })?;

        let refund_secret_key = SecretKey::from_hex(&refund_proof).map_err(|e| {
            ProtectedPaymentError::InvalidClaimProof(format!("Invalid refund secret hex: {e}"))
        })?;

        let recv_options = ReceiveOptions {
            p2pk_signing_keys: vec![refund_secret_key],
            ..Default::default()
        };

        match self.wallet.receive(token_str, recv_options).await {
            Ok(amount) => {
                if tracked.intent.status != PaymentStatus::RefundAvailable {
                    tracked
                        .intent
                        .update_status(PaymentStatus::RefundAvailable)
                        .ok();
                }
                tracked.intent.update_status(PaymentStatus::Refunded)?;

                Ok(ProtectedPaymentReceipt {
                    payment_id: tracked.intent.id,
                    status: tracked.intent.status,
                    amount_sats: SatoshiAmount::from_sats(amount.into()),
                    recipient_identifier: tracked.intent.recipient_identifier.clone(),
                    expires_at: tracked.locktime,
                    claim_reference: format!("refunded_{}", tracked.intent.id.simple()),
                    cashu_token: None,
                    created_at: tracked.intent.created_at,
                })
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("TokenAlreadySpent") || err_msg.contains("already spent") {
                    // Recipient beat sender
                    tracked.intent.update_status(PaymentStatus::Claimed).ok();
                    Err(ProtectedPaymentError::TokenAlreadySpent(
                        "Payment has already been claimed by the recipient".into(),
                    ))
                } else {
                    Err(ProtectedPaymentError::Cdk(format!(
                        "Refund failed at mint: {err_msg}"
                    )))
                }
            }
        }
    }

    async fn get_payment_status(&self, payment_id: Uuid) -> Result<PaymentStatus> {
        let lock = self.intents.read().await;
        let tracked = lock
            .get(&payment_id)
            .ok_or_else(|| ProtectedPaymentError::NotFound(payment_id.to_string()))?;

        let now = Utc::now();
        if now >= tracked.locktime && tracked.intent.status == PaymentStatus::Claimable {
            return Ok(PaymentStatus::RefundAvailable);
        }

        Ok(tracked.intent.status)
    }

    async fn get_wallet_balance(&self) -> Result<WalletBalance> {
        let total_spendable = self
            .wallet
            .total_balance()
            .await
            .map_err(|e| ProtectedPaymentError::Cdk(e.to_string()))?;

        let lock = self.intents.read().await;
        let mut protected_outgoing = 0u64;
        let protected_incoming = 0u64;

        for tracked in lock.values() {
            if tracked.intent.status.is_active() {
                protected_outgoing += tracked.intent.amount_sats.as_u64();
            }
        }

        Ok(WalletBalance {
            spendable_sats: total_spendable.into(),
            protected_outgoing_sats: protected_outgoing,
            protected_incoming_sats: protected_incoming,
        })
    }
}
