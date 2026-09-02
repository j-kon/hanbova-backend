use super::*;
use chrono::Duration;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BitnobAdapter {
    api_key: Option<String>,
    environment: String, // "sandbox", "production", "mock"
}

impl BitnobAdapter {
    pub fn new() -> Self {
        let api_key = std::env::var("BITNOB_API_KEY").ok().filter(|s| !s.trim().is_empty());
        let environment = std::env::var("BITNOB_ENVIRONMENT")
            .unwrap_or_else(|_| if api_key.is_some() { "sandbox".to_string() } else { "mock".to_string() });

        Self {
            api_key,
            environment,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.api_key.is_some() || self.environment == "mock" || self.environment == "sandbox"
    }
}

#[async_trait]
impl PayoutProvider for BitnobAdapter {
    async fn get_supported_corridors(&self, country: Option<&str>) -> ProviderResult<Vec<PayoutCorridor>> {
        let all_corridors = vec![
            PayoutCorridor {
                id: "ke_mpesa".to_string(),
                country: "KE".to_string(),
                currency: "KES".to_string(),
                channel: "m_pesa".to_string(),
                name: "M-Pesa Kenya (Safaricom)".to_string(),
                min_amount_fiat: 100.0,
                max_amount_fiat: 150000.0,
                estimated_fee_sats: 250,
            },
            PayoutCorridor {
                id: "ke_bank".to_string(),
                country: "KE".to_string(),
                currency: "KES".to_string(),
                channel: "bank_transfer".to_string(),
                name: "Kenya Local Bank Transfer".to_string(),
                min_amount_fiat: 500.0,
                max_amount_fiat: 500000.0,
                estimated_fee_sats: 500,
            },
            PayoutCorridor {
                id: "ng_bank".to_string(),
                country: "NG".to_string(),
                currency: "NGN".to_string(),
                channel: "bank_transfer".to_string(),
                name: "Nigeria Instant Bank Transfer (NIP)".to_string(),
                min_amount_fiat: 1000.0,
                max_amount_fiat: 5000000.0,
                estimated_fee_sats: 300,
            },
            PayoutCorridor {
                id: "gh_momo".to_string(),
                country: "GH".to_string(),
                currency: "GHS".to_string(),
                channel: "mobile_money".to_string(),
                name: "Ghana Mobile Money (MTN/Vodafone/Telecel)".to_string(),
                min_amount_fiat: 10.0,
                max_amount_fiat: 10000.0,
                estimated_fee_sats: 200,
            },
            PayoutCorridor {
                id: "ug_momo".to_string(),
                country: "UG".to_string(),
                currency: "UGX".to_string(),
                channel: "mobile_money".to_string(),
                name: "Uganda Mobile Money (MTN / Airtel)".to_string(),
                min_amount_fiat: 5000.0,
                max_amount_fiat: 5000000.0,
                estimated_fee_sats: 350,
            },
            PayoutCorridor {
                id: "rw_momo".to_string(),
                country: "RW".to_string(),
                currency: "RWF".to_string(),
                channel: "mobile_money".to_string(),
                name: "Rwanda Mobile Money (MTN / Airtel)".to_string(),
                min_amount_fiat: 1000.0,
                max_amount_fiat: 2000000.0,
                estimated_fee_sats: 250,
            },
            PayoutCorridor {
                id: "za_bank".to_string(),
                country: "ZA".to_string(),
                currency: "ZAR".to_string(),
                channel: "bank_transfer".to_string(),
                name: "South Africa EFT Bank Transfer".to_string(),
                min_amount_fiat: 50.0,
                max_amount_fiat: 50000.0,
                estimated_fee_sats: 400,
            },
        ];

        if let Some(c) = country {
            let country_upper = c.trim().to_uppercase();
            let filtered: Vec<_> = all_corridors.into_iter().filter(|co| co.country == country_upper).collect();
            if filtered.is_empty() {
                return Err(ProviderError::UnsupportedCountry(format!("No payout corridors available for {}", country_upper)));
            }
            Ok(filtered)
        } else {
            Ok(all_corridors)
        }
    }

    async fn get_payout_quote(&self, req: &PayoutQuoteRequest) -> ProviderResult<PayoutQuote> {
        let corridors = self.get_supported_corridors(None).await?;
        let corridor = corridors
            .into_iter()
            .find(|c| c.id == req.corridor_id)
            .ok_or_else(|| ProviderError::ValidationFailed(format!("Unknown corridor {}", req.corridor_id)))?;

        if req.amount_fiat < corridor.min_amount_fiat || req.amount_fiat > corridor.max_amount_fiat {
            return Err(ProviderError::ValidationFailed(format!(
                "Amount {} {} outside corridor limits (min: {}, max: {})",
                req.amount_fiat, corridor.currency, corridor.min_amount_fiat, corridor.max_amount_fiat
            )));
        }

        // Calibrated reference rates (e.g., 1 BTC = 7.8M KES, 95M NGN, 900k GHS, 1.1M ZAR, 220M UGX, 80M RWF)
        let rate_per_btc = match corridor.currency.as_str() {
            "KES" => 7_800_000.0,
            "NGN" => 95_000_000.0,
            "GHS" => 900_000.0,
            "ZAR" => 1_100_000.0,
            "UGX" => 220_000_000.0,
            "RWF" => 80_000_000.0,
            _ => 60_000.0,
        };

        let sats_amount = ((req.amount_fiat / rate_per_btc) * 100_000_000.0).round() as u64;

        Ok(PayoutQuote {
            quote_id: format!("bitnob_quote_{}", Uuid::new_v4()),
            corridor_id: corridor.id,
            amount_sats: sats_amount,
            amount_fiat: req.amount_fiat,
            fee_sats: corridor.estimated_fee_sats,
            exchange_rate: rate_per_btc,
            recipient_account: req.recipient_account.clone(),
            expires_at: Utc::now() + Duration::minutes(15),
        })
    }

    async fn create_payout(&self, req: &CreatePayoutRequest) -> ProviderResult<PayoutTransaction> {
        if req.recipient_account.trim().is_empty() {
            return Err(ProviderError::ValidationFailed("Recipient account cannot be empty".to_string()));
        }

        Ok(PayoutTransaction {
            id: format!("payout_{}", Uuid::new_v4()),
            quote_id: req.quote_id.clone(),
            corridor_id: "corridor_auto".to_string(),
            recipient_name: req.recipient_name.clone(),
            recipient_account: req.recipient_account.clone(),
            amount_sats: 1000,
            amount_fiat: 100.0,
            status: "completed".to_string(),
            provider: "bitnob".to_string(),
            created_at: Utc::now(),
        })
    }

    async fn get_payout_status(&self, payout_id: &str) -> ProviderResult<PayoutTransaction> {
        Ok(PayoutTransaction {
            id: payout_id.to_string(),
            quote_id: "quote_ref".to_string(),
            corridor_id: "ke_mpesa".to_string(),
            recipient_name: "John Doe".to_string(),
            recipient_account: "254712345678".to_string(),
            amount_sats: 1280,
            amount_fiat: 100.0,
            status: "completed".to_string(),
            provider: "bitnob".to_string(),
            created_at: Utc::now(),
        })
    }
}

#[async_trait]
impl CardProvider for BitnobAdapter {
    async fn check_card_eligibility(&self, country: &str) -> ProviderResult<CardEligibility> {
        let country_upper = country.trim().to_uppercase();
        let (eligible, reason) = match country_upper.as_str() {
            "NG" | "KE" | "GH" | "ZA" | "UG" | "RW" => (
                true,
                None,
            ),
            _ => (
                false,
                Some("Virtual card issuing is currently restricted in this jurisdiction pending partner compliance.".to_string()),
            ),
        };

        Ok(CardEligibility {
            is_eligible: eligible,
            country: country_upper,
            supported_types: if eligible { vec!["virtual_visa".to_string(), "virtual_mastercard".to_string()] } else { vec![] },
            min_funding_sats: 5000,
            reason,
        })
    }

    async fn create_virtual_card(&self, req: &CreateCardRequest) -> ProviderResult<VirtualCard> {
        if req.funding_amount_sats < 5000 {
            return Err(ProviderError::ValidationFailed("Minimum card funding is 5,000 sats".to_string()));
        }

        let now = Utc::now();
        Ok(VirtualCard {
            id: format!("card_{}", Uuid::new_v4()),
            masked_pan: "4111 •••• •••• 8821".to_string(),
            cardholder_name: req.label.clone(),
            expiry_month: 12,
            expiry_year: (now.format("%Y").to_string().parse::<u32>().unwrap_or(2026)) + 3,
            currency: "USD".to_string(),
            balance_sats: req.funding_amount_sats,
            status: "active".to_string(),
            created_at: now,
        })
    }

    async fn get_card_status(&self, card_id: &str) -> ProviderResult<VirtualCard> {
        Ok(VirtualCard {
            id: card_id.to_string(),
            masked_pan: "4111 •••• •••• 8821".to_string(),
            cardholder_name: "Hanbova Traveler".to_string(),
            expiry_month: 12,
            expiry_year: 2029,
            currency: "USD".to_string(),
            balance_sats: 15000,
            status: "active".to_string(),
            created_at: Utc::now(),
        })
    }
}
