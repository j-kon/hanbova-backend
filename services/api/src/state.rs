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
    #[error("production requires a PostgreSQL connection")]
    MissingDatabase,
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
}

impl AppState {
    /// Builds state only when the selected environment has the dependencies it
    /// needs.  The current provider adapters are deterministic mocks, so they
    /// are deliberately unavailable to a production process.
    pub fn try_new(config: AppConfig, pool: Option<PgPool>) -> Result<Self, StateError> {
        if config.is_production() && pool.is_none() {
            return Err(StateError::MissingDatabase);
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

        Self {
            config,
            db_pool: pool,
            payment_service,
            auth_service,
            protected_message_repo,
            lightning_provider,
            cashu_bridge,
        }
    }
}
