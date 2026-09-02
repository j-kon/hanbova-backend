use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod bitnob;
pub mod dtone;

#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum ProviderError {
    #[error("Provider not configured: {0}")]
    NotConfigured(String),

    #[error("Service temporarily unavailable: {0}")]
    Unavailable(String),

    #[error("Unsupported country: {0}")]
    UnsupportedCountry(String),

    #[error("Unsupported service: {0}")]
    UnsupportedService(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Internal provider error: {0}")]
    Internal(String),
}

pub type ProviderResult<T> = Result<T, ProviderError>;

// ==========================================
// PAYOUT MODELS (Bitnob / Corridors)
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutCorridor {
    pub id: String,
    pub country: String,
    pub currency: String,
    pub channel: String, // e.g., "m_pesa", "bank_transfer", "mobile_money"
    pub name: String,
    pub min_amount_fiat: f64,
    pub max_amount_fiat: f64,
    pub estimated_fee_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutQuoteRequest {
    pub corridor_id: String,
    pub amount_fiat: f64,
    pub recipient_account: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutQuote {
    pub quote_id: String,
    pub corridor_id: String,
    pub amount_sats: u64,
    pub amount_fiat: f64,
    pub fee_sats: u64,
    pub exchange_rate: f64,
    pub recipient_account: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePayoutRequest {
    pub quote_id: String,
    pub recipient_name: String,
    pub recipient_account: String,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutTransaction {
    pub id: String,
    pub quote_id: String,
    pub corridor_id: String,
    pub recipient_name: String,
    pub recipient_account: String,
    pub amount_sats: u64,
    pub amount_fiat: f64,
    pub status: String, // "pending", "processing", "completed", "failed"
    pub provider: String,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait PayoutProvider: Send + Sync {
    async fn get_supported_corridors(&self, country: Option<&str>) -> ProviderResult<Vec<PayoutCorridor>>;
    async fn get_payout_quote(&self, req: &PayoutQuoteRequest) -> ProviderResult<PayoutQuote>;
    async fn create_payout(&self, req: &CreatePayoutRequest) -> ProviderResult<PayoutTransaction>;
    async fn get_payout_status(&self, payout_id: &str) -> ProviderResult<PayoutTransaction>;
}

// ==========================================
// VIRTUAL CARDS (Bitnob / Cards)
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardEligibility {
    pub is_eligible: bool,
    pub country: String,
    pub supported_types: Vec<String>,
    pub min_funding_sats: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCardRequest {
    pub card_type: String, // e.g. "virtual_visa", "virtual_mastercard"
    pub label: String,
    pub funding_amount_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualCard {
    pub id: String,
    pub masked_pan: String,
    pub cardholder_name: String,
    pub expiry_month: u32,
    pub expiry_year: u32,
    pub currency: String,
    pub balance_sats: u64,
    pub status: String, // "active", "frozen", "terminated"
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait CardProvider: Send + Sync {
    async fn check_card_eligibility(&self, country: &str) -> ProviderResult<CardEligibility>;
    async fn create_virtual_card(&self, req: &CreateCardRequest) -> ProviderResult<VirtualCard>;
    async fn get_card_status(&self, card_id: &str) -> ProviderResult<VirtualCard>;
}

// ==========================================
// DIGITAL SERVICES / BILL PAYMENTS (DT One)
// ==========================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillServiceType {
    Airtime,
    Data,
    Electricity,
    Water,
    Tv,
    Internet,
}

impl BillServiceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BillServiceType::Airtime => "airtime",
            BillServiceType::Data => "data",
            BillServiceType::Electricity => "electricity",
            BillServiceType::Water => "water",
            BillServiceType::Tv => "tv",
            BillServiceType::Internet => "internet",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "airtime" => Some(BillServiceType::Airtime),
            "data" => Some(BillServiceType::Data),
            "electricity" => Some(BillServiceType::Electricity),
            "water" => Some(BillServiceType::Water),
            "tv" => Some(BillServiceType::Tv),
            "internet" => Some(BillServiceType::Internet),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Biller {
    pub id: String,
    pub country: String,
    pub service_type: BillServiceType,
    pub name: String,
    pub account_reference_label: String,
    pub account_reference_example: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillProduct {
    pub id: String,
    pub biller_id: String,
    pub name: String,
    pub description: Option<String>,
    pub amount_fiat: f64,
    pub is_variable_amount: bool,
    pub min_amount_fiat: Option<f64>,
    pub max_amount_fiat: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerValidation {
    pub is_valid: bool,
    pub biller_id: String,
    pub customer_account: String,
    pub customer_name: Option<String>,
    pub outstanding_amount_fiat: Option<f64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillQuoteRequest {
    pub biller_id: String,
    pub product_id: Option<String>,
    pub amount_fiat: f64,
    pub customer_account: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillQuote {
    pub quote_id: String,
    pub biller_id: String,
    pub product_id: Option<String>,
    pub service_type: BillServiceType,
    pub amount_sats: u64,
    pub amount_fiat: f64,
    pub fee_sats: u64,
    pub exchange_rate: f64,
    pub customer_account: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBillPaymentRequest {
    pub quote_id: String,
    pub customer_account: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillTransaction {
    pub id: String,
    pub quote_id: String,
    pub biller_id: String,
    pub biller_name: String,
    pub service_type: BillServiceType,
    pub customer_account: String,
    pub amount_sats: u64,
    pub amount_fiat: f64,
    pub fee_sats: u64,
    pub status: String, // "pending", "processing", "completed", "failed"
    pub receipt_number: Option<String>,
    pub token_code: Option<String>, // e.g. electricity meter recharge token
    pub provider: String,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait DigitalServicesProvider: Send + Sync {
    async fn get_supported_services(&self, country: &str) -> ProviderResult<Vec<BillServiceType>>;
    async fn get_billers(&self, country: &str, service: Option<&BillServiceType>) -> ProviderResult<Vec<Biller>>;
    async fn get_products(&self, country: &str, biller_id: &str) -> ProviderResult<Vec<BillProduct>>;
    async fn validate_customer(&self, biller_id: &str, account_ref: &str) -> ProviderResult<CustomerValidation>;
    async fn get_bill_quote(&self, req: &BillQuoteRequest) -> ProviderResult<BillQuote>;
    async fn pay_bill(&self, req: &CreateBillPaymentRequest) -> ProviderResult<BillTransaction>;
    async fn get_bill_status(&self, tx_id: &str) -> ProviderResult<BillTransaction>;
}

// ==========================================
// ESIM PROVIDER (DT One)
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsimPackage {
    pub id: String,
    pub country: String,
    pub region: String,
    pub name: String,
    pub data_allowance_mb: u64,
    pub validity_days: u32,
    pub price_sats: u64,
    pub price_fiat: f64,
    pub currency: String,
    pub carrier: String,
    pub network_speed: String, // e.g. "4G/5G"
    pub top_up_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseEsimRequest {
    pub package_id: String,
    pub user_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsimProfile {
    pub id: String,
    pub package_id: String,
    pub package_name: String,
    pub country: String,
    pub iccid: String,
    pub matching_id: String,
    pub smdp_address: String,
    pub qr_code_data: String,
    pub ios_installation_url: String,
    pub android_installation_url: String,
    pub data_allowance_mb: u64,
    pub remaining_data_mb: u64,
    pub status: String, // "allocated", "installed", "active", "depleted", "expired"
    pub top_up_supported: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait EsimProvider: Send + Sync {
    async fn get_supported_countries(&self) -> ProviderResult<Vec<String>>;
    async fn get_esim_packages(&self, country_or_region: &str) -> ProviderResult<Vec<EsimPackage>>;
    async fn purchase_esim(&self, req: &PurchaseEsimRequest) -> ProviderResult<EsimProfile>;
    async fn get_esim_status(&self, profile_id: &str) -> ProviderResult<EsimProfile>;
    async fn top_up_esim(&self, profile_id: &str, package_id: &str) -> ProviderResult<EsimProfile>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitnob::BitnobAdapter;
    use dtone::DtOneAdapter;

    #[tokio::test]
    async fn test_bitnob_payout_corridors_and_quotes() {
        let adapter = BitnobAdapter::new();
        let corridors = adapter.get_supported_corridors(Some("KE")).await.expect("corridors");
        assert!(!corridors.is_empty());
        assert_eq!(corridors[0].country, "KE");

        // Quote calculation
        let quote = adapter.get_payout_quote(&PayoutQuoteRequest {
            corridor_id: "ke_mpesa".to_string(),
            amount_fiat: 1000.0,
            recipient_account: "0712345678".to_string(),
        }).await.expect("payout quote");

        assert_eq!(quote.amount_fiat, 1000.0);
        assert!(quote.amount_sats > 0);
        assert_eq!(quote.recipient_account, "0712345678");

        // Execution
        let tx = adapter.create_payout(&CreatePayoutRequest {
            quote_id: quote.quote_id,
            recipient_name: "John Doe".to_string(),
            recipient_account: "0712345678".to_string(),
            reference: Some("Dinner".to_string()),
        }).await.expect("create payout");

        assert_eq!(tx.status, "completed");
        assert_eq!(tx.provider, "bitnob");
    }

    #[tokio::test]
    async fn test_bitnob_card_eligibility_and_creation() {
        let adapter = BitnobAdapter::new();
        let eligibility_ke = adapter.check_card_eligibility("KE").await.expect("eligibility");
        assert!(eligibility_ke.is_eligible);

        let eligibility_unknown = adapter.check_card_eligibility("XX").await.expect("eligibility");
        assert!(!eligibility_unknown.is_eligible);
        assert!(eligibility_unknown.reason.is_some());

        let card = adapter.create_virtual_card(&CreateCardRequest {
            card_type: "virtual_visa".to_string(),
            label: "Travel Card".to_string(),
            funding_amount_sats: 10000,
        }).await.expect("create card");

        assert_eq!(card.status, "active");
        assert_eq!(card.balance_sats, 10000);
        assert!(card.masked_pan.contains("••••"));
    }

    #[tokio::test]
    async fn test_dtone_bills_and_customer_validation() {
        let adapter = DtOneAdapter::new();
        let services_ke = adapter.get_supported_services("KE").await.expect("services");
        assert!(services_ke.contains(&BillServiceType::Airtime));
        assert!(services_ke.contains(&BillServiceType::Electricity));
        assert!(services_ke.contains(&BillServiceType::Water));

        // Billers
        let billers = adapter.get_billers("KE", Some(&BillServiceType::Electricity)).await.expect("billers");
        assert!(!billers.is_empty());
        assert_eq!(billers[0].service_type, BillServiceType::Electricity);

        // Validation
        let valid = adapter.validate_customer(&billers[0].id, "14123456789").await.expect("validate");
        assert!(valid.is_valid);

        let invalid = adapter.validate_customer(&billers[0].id, "12").await.expect("validate");
        assert!(!invalid.is_valid);

        // Bill Quote & Pay
        let quote = adapter.get_bill_quote(&BillQuoteRequest {
            biller_id: billers[0].id.clone(),
            product_id: None,
            amount_fiat: 500.0,
            customer_account: "14123456789".to_string(),
        }).await.expect("quote");

        assert_eq!(quote.amount_fiat, 500.0);
        assert!(quote.amount_sats > 0);

        let tx = adapter.pay_bill(&CreateBillPaymentRequest {
            quote_id: quote.quote_id,
            customer_account: "14123456789".to_string(),
        }).await.expect("pay");

        assert_eq!(tx.status, "completed");
        assert!(tx.receipt_number.is_some());
    }

    #[tokio::test]
    async fn test_dtone_esim_packages_and_purchase() {
        let adapter = DtOneAdapter::new();
        let packages = adapter.get_esim_packages("KE").await.expect("packages");
        assert!(!packages.is_empty());
        assert_eq!(packages[0].country, "KE");

        let profile = adapter.purchase_esim(&PurchaseEsimRequest {
            package_id: packages[0].id.clone(),
            user_email: Some("traveler@hanbova.africa".to_string()),
        }).await.expect("purchase");

        assert_eq!(profile.status, "active");
        assert!(profile.iccid.starts_with("89234"));
        assert!(profile.qr_code_data.starts_with("LPA:1$"));
        assert!(profile.ios_installation_url.contains("esimsetup.apple.com"));
        assert!(profile.android_installation_url.contains("android.telephony.euicc"));
    }
}
