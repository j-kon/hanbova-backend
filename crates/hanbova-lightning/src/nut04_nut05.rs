//! Cashu NUT-04 (Mint via Lightning) & NUT-05 (Melt via Lightning) Client Bridge

use serde::{Deserialize, Serialize};
use crate::error::{LightningError, Result};

/// Request to request a Lightning invoice from a Cashu mint to deposit/mint sats (NUT-04)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintQuoteRequest {
    pub amount: u64,
    pub unit: String,
}

/// Response containing the mint quote and BOLT11 payment request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintQuoteResponse {
    pub quote: String,
    pub request: String,
    pub state: String,
    pub expiry: Option<u64>,
}

/// Request to request a quote to melt ecash proofs for a BOLT11 payment (NUT-05)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeltQuoteRequest {
    pub request: String,
    pub unit: String,
}

/// Response containing fee and amount to pay the Lightning invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeltQuoteResponse {
    pub quote: String,
    pub amount: u64,
    pub fee_reserve: u64,
    pub state: String,
    pub expiry: Option<u64>,
}

/// Cashu Mint/Melt Bridge for Lightning Swaps
pub struct CashuLightningBridge {
    mint_url: String,
    http_client: reqwest::Client,
}

impl CashuLightningBridge {
    pub fn new(mint_url: impl Into<String>) -> Self {
        Self {
            mint_url: mint_url.into(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Request a Lightning invoice to mint ecash (NUT-04)
    pub async fn create_mint_quote(&self, amount_sats: u64) -> Result<MintQuoteResponse> {
        let url = format!("{}/v1/mint/quote/bolt11", self.mint_url.trim_end_matches('/'));
        let body = MintQuoteRequest {
            amount: amount_sats,
            unit: "sat".to_string(),
        };

        let resp = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| LightningError::ProviderError(format!("Mint quote HTTP failed: {e}")))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(LightningError::ProviderError(format!(
                "Mint quote rejected: {err_text}"
            )));
        }

        let quote: MintQuoteResponse = resp
            .json()
            .await
            .map_err(|e| LightningError::ProviderError(format!("Failed to parse mint quote: {e}")))?;

        Ok(quote)
    }

    /// Check the payment status of a mint quote (NUT-04)
    pub async fn check_mint_quote(&self, quote_id: &str) -> Result<MintQuoteResponse> {
        let url = format!(
            "{}/v1/mint/quote/bolt11/{}",
            self.mint_url.trim_end_matches('/'),
            quote_id
        );

        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| LightningError::ProviderError(format!("Check quote HTTP failed: {e}")))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(LightningError::ProviderError(format!(
                "Check quote rejected: {err_text}"
            )));
        }

        let quote: MintQuoteResponse = resp
            .json()
            .await
            .map_err(|e| LightningError::ProviderError(format!("Failed to parse quote state: {e}")))?;

        Ok(quote)
    }

    /// Request a quote to pay a BOLT11 invoice using ecash (NUT-05)
    pub async fn create_melt_quote(&self, bolt11: &str) -> Result<MeltQuoteResponse> {
        let url = format!("{}/v1/melt/quote/bolt11", self.mint_url.trim_end_matches('/'));
        let body = MeltQuoteRequest {
            request: bolt11.to_string(),
            unit: "sat".to_string(),
        };

        let resp = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| LightningError::ProviderError(format!("Melt quote HTTP failed: {e}")))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(LightningError::ProviderError(format!(
                "Melt quote rejected: {err_text}"
            )));
        }

        let quote: MeltQuoteResponse = resp
            .json()
            .await
            .map_err(|e| LightningError::ProviderError(format!("Failed to parse melt quote: {e}")))?;

        Ok(quote)
    }
}
