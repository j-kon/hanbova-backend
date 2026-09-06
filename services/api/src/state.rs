use sqlx::PgPool;
use std::sync::Arc;

use crate::{
    auth::{
        repository::{InMemoryUserRepository, PgUserRepository, UserRepository},
        AuthService,
    },
    config::{AppConfig, ProviderMode},
    repositories::{
        InMemoryPaymentIntentRepository, InMemoryProtectedMessageRepository,
        PgPaymentIntentRepository, PgProtectedMessageRepository, ProtectedMessageRepository,
    },
    services::PaymentService,
};
use hanbova_lightning::{CashuLightningBridge, LightningProvider, MockLightningProvider};
use hanbova_protected_payments::MockProtectedPaymentProvider;

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("pilot and production require a PostgreSQL connection")]
    MissingDatabase,
    #[error("mock providers are forbidden in pilot")]
    MockProviderForbiddenInPilot,
    #[error("production providers have not been configured")]
    ProductionProvidersUnavailable,
}

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db_pool: Option<PgPool>,
    pub payment_service: PaymentService,
    pub auth_service: AuthService,
    pub protected_message_repo: Arc<dyn ProtectedMessageRepository>,
    pub lightning_provider: Arc<dyn LightningProvider>,
    pub cashu_bridge: Arc<CashuLightningBridge>,
    pub rate_service: Arc<crate::services::HanbovaRateService>,
    pub digital_services_provider: Arc<dyn crate::providers::DigitalServicesProvider>,
    pub payout_provider: Arc<dyn crate::providers::PayoutProvider>,
    pub card_provider: Arc<dyn crate::providers::CardProvider>,
    pub esim_provider: Arc<dyn crate::providers::EsimProvider>,
}

impl AppState {
    /// Builds state only when the selected environment has the dependencies it
    /// needs.  Mock providers are strictly forbidden in pilot and production.
    pub fn try_new(config: AppConfig, pool: Option<PgPool>) -> Result<Self, StateError> {
        if (config.is_pilot() || config.is_production()) && pool.is_none() {
            return Err(StateError::MissingDatabase);
        }
        if config.is_pilot() && config.provider_mode == ProviderMode::Mock {
            return Err(StateError::MockProviderForbiddenInPilot);
        }
        if config.provider_mode == ProviderMode::Production {
            return Err(StateError::ProductionProvidersUnavailable);
        }

        Ok(Self::new(config, pool))
    }

    pub fn new(config: AppConfig, pool: Option<PgPool>) -> Self {
        let protected_provider = Arc::new(MockProtectedPaymentProvider::new());

        let repo: Arc<dyn crate::repositories::PaymentIntentRepository> = match &pool {
            Some(p) => Arc::new(PgPaymentIntentRepository::new(p.clone())),
            None => Arc::new(InMemoryPaymentIntentRepository::new()),
        };

        let payment_service = PaymentService::new(repo, protected_provider);

        let user_repo: Arc<dyn UserRepository> = match &pool {
            Some(p) => Arc::new(PgUserRepository::new(p.clone())),
            None => Arc::new(InMemoryUserRepository::new()),
        };

        let auth_service = AuthService::new(
            user_repo.clone(),
            config.jwt_secret.clone(),
            config.is_development(),
        );

        let protected_message_repo: Arc<dyn ProtectedMessageRepository> = match &pool {
            Some(p) => Arc::new(PgProtectedMessageRepository::new(p.clone())),
            None => Arc::new(InMemoryProtectedMessageRepository::new(Some(
                user_repo.clone(),
            ))),
        };

        let lightning_provider: Arc<dyn LightningProvider> =
            Arc::new(MockLightningProvider::new(100_000));
        let cashu_bridge = Arc::new(CashuLightningBridge::new(&config.mint_url));

        let rate_provider: Arc<dyn crate::providers::PlatformRateProvider> =
            Arc::new(crate::providers::BitnobRateProvider::new());
        let rate_service = Arc::new(crate::services::HanbovaRateService::new(rate_provider));

        let dtone_adapter = Arc::new(crate::providers::dtone::DtOneAdapter::with_mode(
            config.provider_mode,
        ));
        let bitnob_adapter = Arc::new(crate::providers::bitnob::BitnobAdapter::with_mode(
            config.provider_mode,
        ));

        let digital_services_provider: Arc<dyn crate::providers::DigitalServicesProvider> =
            dtone_adapter.clone();
        let esim_provider: Arc<dyn crate::providers::EsimProvider> = dtone_adapter;
        let payout_provider: Arc<dyn crate::providers::PayoutProvider> = bitnob_adapter.clone();
        let card_provider: Arc<dyn crate::providers::CardProvider> = bitnob_adapter;

        Self {
            config,
            db_pool: pool,
            payment_service,
            auth_service,
            protected_message_repo,
            lightning_provider,
            cashu_bridge,
            rate_service,
            digital_services_provider,
            payout_provider,
            card_provider,
            esim_provider,
        }
    }

    pub fn capabilities(&self) -> crate::providers::ProviderCapabilities {
        use crate::providers::CapabilityStatus;

        let bitnob_rates_status = match self.config.provider_mode {
            ProviderMode::Mock => CapabilityStatus::Mock,
            ProviderMode::Sandbox => CapabilityStatus::Sandbox,
            ProviderMode::Production => CapabilityStatus::Production,
        };

        let bitnob_payouts_status = match self.config.provider_mode {
            ProviderMode::Mock => CapabilityStatus::Mock,
            _ => CapabilityStatus::Disabled,
        };

        let dtone_bills_status = match self.config.provider_mode {
            ProviderMode::Mock => CapabilityStatus::Mock,
            ProviderMode::Sandbox => CapabilityStatus::Sandbox,
            ProviderMode::Production => CapabilityStatus::Disabled,
        };

        let lightning_status = if !self.config.lightning_enabled {
            CapabilityStatus::Disabled
        } else if self.config.provider_mode.is_mock() {
            CapabilityStatus::Mock
        } else {
            CapabilityStatus::Disabled
        };

        crate::providers::ProviderCapabilities {
            bitnob_rates: bitnob_rates_status,
            bitnob_wallet: CapabilityStatus::Disabled,
            bitnob_payouts: bitnob_payouts_status,
            dtone_bills: dtone_bills_status,
            lightning: lightning_status,
            protected_send: CapabilityStatus::Test,
        }
    }
}
