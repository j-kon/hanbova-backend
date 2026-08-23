use sqlx::PgPool;
use std::sync::Arc;

use crate::{
    auth::{
        repository::{InMemoryUserRepository, PgUserRepository, UserRepository},
        AuthService,
    },
    config::AppConfig,
    repositories::{
        InMemoryPaymentIntentRepository, InMemoryProtectedMessageRepository,
        PgPaymentIntentRepository, PgProtectedMessageRepository, ProtectedMessageRepository,
    },
    services::PaymentService,
};
use hanbova_protected_payments::MockProtectedPaymentProvider;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db_pool: Option<PgPool>,
    pub payment_service: PaymentService,
    pub auth_service: AuthService,
    pub protected_message_repo: Arc<dyn ProtectedMessageRepository>,
}

impl AppState {
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
            None => Arc::new(InMemoryProtectedMessageRepository::new(Some(user_repo.clone()))),
        };

        Self {
            config,
            db_pool: pool,
            payment_service,
            auth_service,
            protected_message_repo,
        }
    }
}
