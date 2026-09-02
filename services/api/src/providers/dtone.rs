use super::*;
use chrono::Duration;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DtOneAdapter {
    api_key: Option<String>,
    api_secret: Option<String>,
    environment: String, // "sandbox", "production", "mock"
}

impl DtOneAdapter {
    pub fn new() -> Self {
        let api_key = std::env::var("DTONE_API_KEY").ok().filter(|s| !s.trim().is_empty());
        let api_secret = std::env::var("DTONE_API_SECRET").ok().filter(|s| !s.trim().is_empty());
        let environment = std::env::var("DTONE_ENVIRONMENT")
            .unwrap_or_else(|_| if api_key.is_some() { "sandbox".to_string() } else { "mock".to_string() });

        Self {
            api_key,
            api_secret,
            environment,
        }
    }

    pub fn is_configured(&self) -> bool {
        (self.api_key.is_some() && self.api_secret.is_some()) || self.environment == "mock" || self.environment == "sandbox"
    }
}

#[async_trait]
impl DigitalServicesProvider for DtOneAdapter {
    async fn get_supported_services(&self, country: &str) -> ProviderResult<Vec<BillServiceType>> {
        let c = country.trim().to_uppercase();
        match c.as_str() {
            "KE" => Ok(vec![
                BillServiceType::Airtime,
                BillServiceType::Data,
                BillServiceType::Electricity,
                BillServiceType::Water,
                BillServiceType::Tv,
                BillServiceType::Internet,
            ]),
            "NG" => Ok(vec![
                BillServiceType::Airtime,
                BillServiceType::Data,
                BillServiceType::Electricity,
                BillServiceType::Tv,
                BillServiceType::Internet,
            ]),
            "GH" => Ok(vec![
                BillServiceType::Airtime,
                BillServiceType::Data,
                BillServiceType::Electricity,
                BillServiceType::Water,
                BillServiceType::Tv,
            ]),
            "ZA" => Ok(vec![
                BillServiceType::Airtime,
                BillServiceType::Data,
                BillServiceType::Electricity,
                BillServiceType::Tv,
                BillServiceType::Internet,
            ]),
            "UG" => Ok(vec![
                BillServiceType::Airtime,
                BillServiceType::Data,
                BillServiceType::Electricity,
                BillServiceType::Water,
                BillServiceType::Tv,
            ]),
            "RW" => Ok(vec![
                BillServiceType::Airtime,
                BillServiceType::Data,
                BillServiceType::Electricity,
                BillServiceType::Water,
                BillServiceType::Tv,
            ]),
            _ => Err(ProviderError::UnsupportedCountry(format!("Digital bill services not available in {}", c))),
        }
    }

    async fn get_billers(&self, country: &str, service: Option<&BillServiceType>) -> ProviderResult<Vec<Biller>> {
        let c = country.trim().to_uppercase();
        let mut billers = Vec::new();

        match c.as_str() {
            "KE" => {
                billers.extend(vec![
                    Biller {
                        id: "ke_safaricom".to_string(),
                        country: "KE".to_string(),
                        service_type: BillServiceType::Airtime,
                        name: "Safaricom Airtime".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "0712345678".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ke_safaricom_data".to_string(),
                        country: "KE".to_string(),
                        service_type: BillServiceType::Data,
                        name: "Safaricom Mobile Bundles".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "0712345678".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ke_airtel".to_string(),
                        country: "KE".to_string(),
                        service_type: BillServiceType::Airtime,
                        name: "Airtel Kenya Airtime".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "0733123456".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ke_kplc_prepaid".to_string(),
                        country: "KE".to_string(),
                        service_type: BillServiceType::Electricity,
                        name: "KPLC Prepaid Electricity (Tokens)".to_string(),
                        account_reference_label: "Meter Number".to_string(),
                        account_reference_example: "14123456789".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ke_nairobi_water".to_string(),
                        country: "KE".to_string(),
                        service_type: BillServiceType::Water,
                        name: "Nairobi City Water & Sewerage".to_string(),
                        account_reference_label: "Account Number".to_string(),
                        account_reference_example: "NCWSC-88910".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ke_dstv".to_string(),
                        country: "KE".to_string(),
                        service_type: BillServiceType::Tv,
                        name: "DStv Kenya".to_string(),
                        account_reference_label: "SmartCard / IUC Number".to_string(),
                        account_reference_example: "1023456789".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ke_zuku".to_string(),
                        country: "KE".to_string(),
                        service_type: BillServiceType::Internet,
                        name: "Zuku Fiber Home Internet".to_string(),
                        account_reference_label: "Account Number".to_string(),
                        account_reference_example: "ZK-109283".to_string(),
                        is_active: true,
                    },
                ]);
            }
            "NG" => {
                billers.extend(vec![
                    Biller {
                        id: "ng_mtn".to_string(),
                        country: "NG".to_string(),
                        service_type: BillServiceType::Airtime,
                        name: "MTN Nigeria Airtime".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "08031234567".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ng_mtn_data".to_string(),
                        country: "NG".to_string(),
                        service_type: BillServiceType::Data,
                        name: "MTN Nigeria Data Plans".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "08031234567".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ng_airtel".to_string(),
                        country: "NG".to_string(),
                        service_type: BillServiceType::Airtime,
                        name: "Airtel Nigeria Airtime".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "08021234567".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ng_ikedc".to_string(),
                        country: "NG".to_string(),
                        service_type: BillServiceType::Electricity,
                        name: "Ikeja Electric (IKEDC Prepaid)".to_string(),
                        account_reference_label: "Meter Number".to_string(),
                        account_reference_example: "01011234567".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ng_dstv".to_string(),
                        country: "NG".to_string(),
                        service_type: BillServiceType::Tv,
                        name: "DStv Nigeria".to_string(),
                        account_reference_label: "SmartCard / IUC Number".to_string(),
                        account_reference_example: "7023456789".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ng_spectranet".to_string(),
                        country: "NG".to_string(),
                        service_type: BillServiceType::Internet,
                        name: "Spectranet 4G LTE Internet".to_string(),
                        account_reference_label: "User ID / Account".to_string(),
                        account_reference_example: "SPEC-55443".to_string(),
                        is_active: true,
                    },
                ]);
            }
            "GH" => {
                billers.extend(vec![
                    Biller {
                        id: "gh_mtn".to_string(),
                        country: "GH".to_string(),
                        service_type: BillServiceType::Airtime,
                        name: "MTN Ghana Airtime".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "0241234567".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "gh_mtn_data".to_string(),
                        country: "GH".to_string(),
                        service_type: BillServiceType::Data,
                        name: "MTN Ghana Data Bundles".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "0241234567".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "gh_ecg".to_string(),
                        country: "GH".to_string(),
                        service_type: BillServiceType::Electricity,
                        name: "Electricity Company of Ghana (ECG)".to_string(),
                        account_reference_label: "Meter Number".to_string(),
                        account_reference_example: "P12345678".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "gh_water".to_string(),
                        country: "GH".to_string(),
                        service_type: BillServiceType::Water,
                        name: "Ghana Water Company Ltd (GWCL)".to_string(),
                        account_reference_label: "Customer Account".to_string(),
                        account_reference_example: "GW-998811".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "gh_dstv".to_string(),
                        country: "GH".to_string(),
                        service_type: BillServiceType::Tv,
                        name: "DStv Ghana".to_string(),
                        account_reference_label: "SmartCard Number".to_string(),
                        account_reference_example: "4012345678".to_string(),
                        is_active: true,
                    },
                ]);
            }
            "ZA" => {
                billers.extend(vec![
                    Biller {
                        id: "za_vodacom".to_string(),
                        country: "ZA".to_string(),
                        service_type: BillServiceType::Airtime,
                        name: "Vodacom South Africa Airtime".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "0821234567".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "za_vodacom_data".to_string(),
                        country: "ZA".to_string(),
                        service_type: BillServiceType::Data,
                        name: "Vodacom Data Bundles".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "0821234567".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "za_eskom".to_string(),
                        country: "ZA".to_string(),
                        service_type: BillServiceType::Electricity,
                        name: "Eskom Prepaid Electricity".to_string(),
                        account_reference_label: "Meter Number".to_string(),
                        account_reference_example: "04123456789".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "za_dstv".to_string(),
                        country: "ZA".to_string(),
                        service_type: BillServiceType::Tv,
                        name: "DStv South Africa".to_string(),
                        account_reference_label: "SmartCard Number".to_string(),
                        account_reference_example: "3012345678".to_string(),
                        is_active: true,
                    },
                ]);
            }
            "UG" => {
                billers.extend(vec![
                    Biller {
                        id: "ug_mtn".to_string(),
                        country: "UG".to_string(),
                        service_type: BillServiceType::Airtime,
                        name: "MTN Uganda Airtime".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "0771234567".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ug_umeme".to_string(),
                        country: "UG".to_string(),
                        service_type: BillServiceType::Electricity,
                        name: "Umeme Yaka Electricity Tokens".to_string(),
                        account_reference_label: "Meter Number".to_string(),
                        account_reference_example: "37123456789".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "ug_nwsc".to_string(),
                        country: "UG".to_string(),
                        service_type: BillServiceType::Water,
                        name: "National Water & Sewerage (NWSC)".to_string(),
                        account_reference_label: "Account Number".to_string(),
                        account_reference_example: "NWSC-44332".to_string(),
                        is_active: true,
                    },
                ]);
            }
            "RW" => {
                billers.extend(vec![
                    Biller {
                        id: "rw_mtn".to_string(),
                        country: "RW".to_string(),
                        service_type: BillServiceType::Airtime,
                        name: "MTN Rwanda Airtime".to_string(),
                        account_reference_label: "Phone Number".to_string(),
                        account_reference_example: "0788123456".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "rw_eucl".to_string(),
                        country: "RW".to_string(),
                        service_type: BillServiceType::Electricity,
                        name: "EUCL Cash Power Electricity".to_string(),
                        account_reference_label: "Meter Number".to_string(),
                        account_reference_example: "14223344556".to_string(),
                        is_active: true,
                    },
                    Biller {
                        id: "rw_wasac".to_string(),
                        country: "RW".to_string(),
                        service_type: BillServiceType::Water,
                        name: "WASAC Water Rwanda".to_string(),
                        account_reference_label: "Account Number".to_string(),
                        account_reference_example: "WASAC-10293".to_string(),
                        is_active: true,
                    },
                ]);
            }
            _ => return Err(ProviderError::UnsupportedCountry(format!("No billers found for country {}", c))),
        }

        if let Some(st) = service {
            Ok(billers.into_iter().filter(|b| &b.service_type == st).collect())
        } else {
            Ok(billers)
        }
    }

    async fn get_products(&self, _country: &str, biller_id: &str) -> ProviderResult<Vec<BillProduct>> {
        let b = biller_id.trim().to_lowercase();
        if b.contains("data") {
            Ok(vec![
                BillProduct {
                    id: format!("{}_1gb_daily", b),
                    biller_id: b.clone(),
                    name: "1 GB Daily Plan".to_string(),
                    description: Some("Valid for 24 hours".to_string()),
                    amount_fiat: 100.0,
                    is_variable_amount: false,
                    min_amount_fiat: None,
                    max_amount_fiat: None,
                },
                BillProduct {
                    id: format!("{}_3gb_weekly", b),
                    biller_id: b.clone(),
                    name: "3 GB Weekly Bundle".to_string(),
                    description: Some("Valid for 7 days".to_string()),
                    amount_fiat: 300.0,
                    is_variable_amount: false,
                    min_amount_fiat: None,
                    max_amount_fiat: None,
                },
                BillProduct {
                    id: format!("{}_10gb_monthly", b),
                    biller_id: b.clone(),
                    name: "10 GB Monthly Super Bundle".to_string(),
                    description: Some("Valid for 30 days".to_string()),
                    amount_fiat: 1000.0,
                    is_variable_amount: false,
                    min_amount_fiat: None,
                    max_amount_fiat: None,
                },
            ])
        } else {
            // Variable amount recharge for Airtime or Utilities
            Ok(vec![
                BillProduct {
                    id: format!("{}_topup", b),
                    biller_id: b,
                    name: "Flexible Recharge".to_string(),
                    description: Some("Enter any custom amount".to_string()),
                    amount_fiat: 0.0,
                    is_variable_amount: true,
                    min_amount_fiat: Some(50.0),
                    max_amount_fiat: Some(50000.0),
                }
            ])
        }
    }

    async fn validate_customer(&self, biller_id: &str, account_ref: &str) -> ProviderResult<CustomerValidation> {
        let ref_clean = account_ref.trim();
        if ref_clean.is_empty() {
            return Err(ProviderError::ValidationFailed("Account reference cannot be empty".to_string()));
        }

        if ref_clean.len() < 5 {
            return Ok(CustomerValidation {
                is_valid: false,
                biller_id: biller_id.to_string(),
                customer_account: ref_clean.to_string(),
                customer_name: None,
                outstanding_amount_fiat: None,
                message: Some("Account reference too short".to_string()),
            });
        }

        Ok(CustomerValidation {
            is_valid: true,
            biller_id: biller_id.to_string(),
            customer_account: ref_clean.to_string(),
            customer_name: Some("Verified Customer (Sandbox)".to_string()),
            outstanding_amount_fiat: Some(0.0),
            message: Some("Account validated successfully".to_string()),
        })
    }

    async fn get_bill_quote(&self, req: &BillQuoteRequest) -> ProviderResult<BillQuote> {
        if req.amount_fiat <= 0.0 {
            return Err(ProviderError::ValidationFailed("Amount must be greater than zero".to_string()));
        }

        // Calibrated FX reference rate
        let rate_per_btc = if req.biller_id.starts_with("ng_") {
            95_000_000.0
        } else if req.biller_id.starts_with("gh_") {
            900_000.0
        } else if req.biller_id.starts_with("za_") {
            1_100_000.0
        } else if req.biller_id.starts_with("ug_") {
            220_000_000.0
        } else if req.biller_id.starts_with("rw_") {
            80_000_000.0
        } else {
            7_800_000.0 // Default KES
        };

        let sats_amount = ((req.amount_fiat / rate_per_btc) * 100_000_000.0).round() as u64;
        let service_type = if req.biller_id.contains("data") {
            BillServiceType::Data
        } else if req.biller_id.contains("kplc") || req.biller_id.contains("electric") || req.biller_id.contains("ecg") || req.biller_id.contains("eskom") || req.biller_id.contains("umeme") || req.biller_id.contains("eucl") {
            BillServiceType::Electricity
        } else if req.biller_id.contains("water") {
            BillServiceType::Water
        } else if req.biller_id.contains("dstv") || req.biller_id.contains("tv") {
            BillServiceType::Tv
        } else if req.biller_id.contains("zuku") || req.biller_id.contains("spectranet") || req.biller_id.contains("internet") {
            BillServiceType::Internet
        } else {
            BillServiceType::Airtime
        };

        Ok(BillQuote {
            quote_id: format!("bill_quote_{}", Uuid::new_v4()),
            biller_id: req.biller_id.clone(),
            product_id: req.product_id.clone(),
            service_type,
            amount_sats: sats_amount,
            amount_fiat: req.amount_fiat,
            fee_sats: 50,
            exchange_rate: rate_per_btc,
            customer_account: req.customer_account.clone(),
            expires_at: Utc::now() + Duration::minutes(15),
        })
    }

    async fn pay_bill(&self, req: &CreateBillPaymentRequest) -> ProviderResult<BillTransaction> {
        let token_code = if req.quote_id.contains("electric") {
            Some("5821-9920-1123-8874-0019".to_string())
        } else {
            None
        };

        Ok(BillTransaction {
            id: format!("bill_tx_{}", Uuid::new_v4()),
            quote_id: req.quote_id.clone(),
            biller_id: "biller_auto".to_string(),
            biller_name: "DT One Biller (Sandbox)".to_string(),
            service_type: BillServiceType::Airtime,
            customer_account: req.customer_account.clone(),
            amount_sats: 500,
            amount_fiat: 100.0,
            fee_sats: 50,
            status: "completed".to_string(),
            receipt_number: Some(format!("REC-{}", Uuid::new_v4().to_string()[..8].to_uppercase())),
            token_code,
            provider: "dtone".to_string(),
            created_at: Utc::now(),
        })
    }

    async fn get_bill_status(&self, tx_id: &str) -> ProviderResult<BillTransaction> {
        Ok(BillTransaction {
            id: tx_id.to_string(),
            quote_id: "quote_ref".to_string(),
            biller_id: "ke_safaricom".to_string(),
            biller_name: "Safaricom Airtime".to_string(),
            service_type: BillServiceType::Airtime,
            customer_account: "0712345678".to_string(),
            amount_sats: 500,
            amount_fiat: 100.0,
            fee_sats: 50,
            status: "completed".to_string(),
            receipt_number: Some("REC-98218731".to_string()),
            token_code: None,
            provider: "dtone".to_string(),
            created_at: Utc::now(),
        })
    }
}

#[async_trait]
impl EsimProvider for DtOneAdapter {
    async fn get_supported_countries(&self) -> ProviderResult<Vec<String>> {
        Ok(vec![
            "KE".to_string(),
            "NG".to_string(),
            "GH".to_string(),
            "ZA".to_string(),
            "UG".to_string(),
            "RW".to_string(),
            "EG".to_string(),
            "MA".to_string(),
            "GLOBAL".to_string(),
        ])
    }

    async fn get_esim_packages(&self, country_or_region: &str) -> ProviderResult<Vec<EsimPackage>> {
        let cr = country_or_region.trim().to_uppercase();
        let name_prefix = match cr.as_str() {
            "KE" => "Kenya Traveler",
            "NG" => "Nigeria Express",
            "GH" => "Ghana Connect",
            "ZA" => "South Africa Safari",
            "UG" => "Uganda Pearl",
            "RW" => "Rwanda Hills",
            "EG" => "Egypt Nile",
            "MA" => "Morocco Atlas",
            _ => "Africa Regional",
        };

        Ok(vec![
            EsimPackage {
                id: format!("esim_{}_1gb_7d", cr.to_lowercase()),
                country: cr.clone(),
                region: if cr == "GLOBAL" { "Global".to_string() } else { "Africa".to_string() },
                name: format!("{} 1 GB", name_prefix),
                data_allowance_mb: 1024,
                validity_days: 7,
                price_sats: 5000,
                price_fiat: 3.50,
                currency: "USD".to_string(),
                carrier: "Safaricom / MTN / Partner".to_string(),
                network_speed: "4G/5G".to_string(),
                top_up_supported: true,
            },
            EsimPackage {
                id: format!("esim_{}_3gb_15d", cr.to_lowercase()),
                country: cr.clone(),
                region: if cr == "GLOBAL" { "Global".to_string() } else { "Africa".to_string() },
                name: format!("{} 3 GB", name_prefix),
                data_allowance_mb: 3072,
                validity_days: 15,
                price_sats: 12000,
                price_fiat: 8.00,
                currency: "USD".to_string(),
                carrier: "Safaricom / MTN / Partner".to_string(),
                network_speed: "4G/5G".to_string(),
                top_up_supported: true,
            },
            EsimPackage {
                id: format!("esim_{}_10gb_30d", cr.to_lowercase()),
                country: cr.clone(),
                region: if cr == "GLOBAL" { "Global".to_string() } else { "Africa".to_string() },
                name: format!("{} 10 GB Super", name_prefix),
                data_allowance_mb: 10240,
                validity_days: 30,
                price_sats: 30000,
                price_fiat: 20.00,
                currency: "USD".to_string(),
                carrier: "Multi-Carrier 5G".to_string(),
                network_speed: "5G".to_string(),
                top_up_supported: true,
            },
        ])
    }

    async fn purchase_esim(&self, req: &PurchaseEsimRequest) -> ProviderResult<EsimProfile> {
        let matching_id = format!("TEST-{}", Uuid::new_v4().to_string()[..8].to_uppercase());
        let smdp = "rsp.dtone.com".to_string();
        let activation_code = format!("LPA:1${}${}", smdp, matching_id);
        let iccid = format!("892340210000{}", Uuid::new_v4().to_string().chars().filter(|c| c.is_ascii_digit()).take(8).collect::<String>());

        let now = Utc::now();
        Ok(EsimProfile {
            id: format!("esim_prof_{}", Uuid::new_v4()),
            package_id: req.package_id.clone(),
            package_name: "DT One Traveler eSIM (Sandbox)".to_string(),
            country: "KE".to_string(),
            iccid: iccid.clone(),
            matching_id,
            smdp_address: smdp,
            qr_code_data: activation_code.clone(),
            ios_installation_url: format!("https://esimsetup.apple.com/esim_qrcode_provisioning?carddata={}", activation_code),
            android_installation_url: format!("intent:#Intent;action=android.telephony.euicc.action.DOWNLOAD_SUBSCRIPTION;S.activation_code={};end", activation_code),
            data_allowance_mb: 3072,
            remaining_data_mb: 3072,
            status: "active".to_string(),
            top_up_supported: true,
            created_at: now,
            expires_at: now + Duration::days(15),
        })
    }

    async fn get_esim_status(&self, profile_id: &str) -> ProviderResult<EsimProfile> {
        let now = Utc::now();
        Ok(EsimProfile {
            id: profile_id.to_string(),
            package_id: "esim_ke_3gb_15d".to_string(),
            package_name: "Kenya Traveler 3 GB".to_string(),
            country: "KE".to_string(),
            iccid: "89234021000012345678".to_string(),
            matching_id: "MATCH-1234".to_string(),
            smdp_address: "rsp.dtone.com".to_string(),
            qr_code_data: "LPA:1$rsp.dtone.com$MATCH-1234".to_string(),
            ios_installation_url: "https://esimsetup.apple.com/esim_qrcode_provisioning?carddata=LPA:1$rsp.dtone.com$MATCH-1234".to_string(),
            android_installation_url: "intent:#Intent;action=android.telephony.euicc.action.DOWNLOAD_SUBSCRIPTION;S.activation_code=LPA:1$rsp.dtone.com$MATCH-1234;end".to_string(),
            data_allowance_mb: 3072,
            remaining_data_mb: 2450,
            status: "active".to_string(),
            top_up_supported: true,
            created_at: now - Duration::days(2),
            expires_at: now + Duration::days(13),
        })
    }

    async fn top_up_esim(&self, profile_id: &str, package_id: &str) -> ProviderResult<EsimProfile> {
        let mut prof = self.get_esim_status(profile_id).await?;
        prof.package_id = package_id.to_string();
        prof.remaining_data_mb += 1024;
        prof.data_allowance_mb += 1024;
        Ok(prof)
    }
}
