use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::config::AppConfig;

/// Connects PostgreSQL and applies migrations before the application accepts
/// traffic. Errors are deliberately returned to the caller so pilot and
/// production can fail closed instead of constructing an in-memory state.
pub async fn connect_database(config: &AppConfig) -> Result<Option<PgPool>, sqlx::Error> {
    let Some(database_url) = &config.database_url else {
        if config.is_pilot() || config.is_production() {
            return Err(sqlx::Error::Configuration(
                format!("DATABASE_URL is required in {}", config.environment).into(),
            ));
        }
        tracing::info!("No DATABASE_URL configured. Running with in-memory persistence.");
        return Ok(None);
    };

    tracing::info!("Connecting to PostgreSQL database...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(database_url)
        .await?;
    tracing::info!("PostgreSQL connected successfully.");

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations applied cleanly.");

    Ok(Some(pool))
}

#[cfg(test)]
mod tests {
    use crate::{config::AppConfig, state::AppState};

    fn production_config() -> AppConfig {
        AppConfig::from_iter([
            ("HANBOVA_ENV", "production"),
            ("HANBOVA_API_HOST", "0.0.0.0"),
            ("HANBOVA_API_PORT", "8080"),
            ("DATABASE_URL", "postgres://hanbova:secret@db/hanbova"),
            ("JWT_SECRET", "production-secret-that-is-at-least-32-bytes"),
            ("MINT_URL", "https://mint.example.com"),
            ("PROVIDER_MODE", "production"),
            ("CORS_ALLOWED_ORIGINS", "https://app.example.com"),
        ])
        .unwrap()
    }

    fn pilot_config() -> AppConfig {
        AppConfig::from_iter([
            ("HANBOVA_ENV", "pilot"),
            ("HANBOVA_API_HOST", "0.0.0.0"),
            ("HANBOVA_API_PORT", "8080"),
            ("DATABASE_URL", "postgres://hanbova:secret@db/hanbova"),
            ("JWT_SECRET", "pilot-secret-that-is-at-least-32-bytes-long"),
            ("MINT_URL", "https://test-mint.example.com"),
            ("PROVIDER_MODE", "sandbox"),
            ("CORS_ALLOWED_ORIGINS", "https://pilot.example.com"),
        ])
        .unwrap()
    }

    #[test]
    fn production_refuses_missing_pool() {
        let error = match AppState::try_new(production_config(), None) {
            Ok(_) => panic!("production must not start without PostgreSQL"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("PostgreSQL"));
    }

    #[test]
    fn pilot_refuses_missing_pool() {
        let error = match AppState::try_new(pilot_config(), None) {
            Ok(_) => panic!("pilot must not start without PostgreSQL"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("PostgreSQL"));
    }

    #[test]
    fn development_allows_in_memory_state() {
        let config = AppConfig::from_iter([("HANBOVA_ENV", "development")]).unwrap();
        let state = AppState::try_new(config, None).unwrap();
        assert!(state.db_pool.is_none());
    }

    #[tokio::test]
    async fn production_refuses_to_substitute_mock_providers() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://hanbova:secret@db/hanbova")
            .unwrap();
        let error = match AppState::try_new(production_config(), Some(pool)) {
            Ok(_) => panic!("production must not start with unconfigured providers"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("providers"));
    }

    #[tokio::test]
    async fn pilot_allows_sandbox_providers_with_db() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://hanbova:secret@db/hanbova")
            .unwrap();
        let state = AppState::try_new(pilot_config(), Some(pool)).unwrap();
        assert!(state.db_pool.is_some());
        let caps = state.capabilities();
        assert_eq!(
            caps.bitnob_rates,
            crate::providers::CapabilityStatus::Sandbox
        );
        assert_eq!(
            caps.dtone_bills,
            crate::providers::CapabilityStatus::Sandbox
        );
        assert_eq!(caps.lightning, crate::providers::CapabilityStatus::Disabled);
        assert_eq!(
            caps.protected_send,
            crate::providers::CapabilityStatus::Test
        );
    }

    #[tokio::test]
    async fn database_connection_errors_are_returned_in_production() {
        let config = AppConfig::from_iter([
            ("HANBOVA_ENV", "production"),
            ("HANBOVA_API_HOST", "0.0.0.0"),
            ("HANBOVA_API_PORT", "8080"),
            ("DATABASE_URL", "not-a-postgres-url"),
            ("JWT_SECRET", "production-secret-that-is-at-least-32-bytes"),
            ("MINT_URL", "https://mint.example.com"),
            ("PROVIDER_MODE", "production"),
            ("CORS_ALLOWED_ORIGINS", "https://app.example.com"),
        ])
        .unwrap();

        let error = super::connect_database(&config).await.unwrap_err();
        assert!(!error.to_string().is_empty());
    }

    #[tokio::test]
    async fn database_connection_errors_are_returned_in_pilot() {
        let config = AppConfig::from_iter([
            ("HANBOVA_ENV", "pilot"),
            ("HANBOVA_API_HOST", "0.0.0.0"),
            ("HANBOVA_API_PORT", "8080"),
            ("DATABASE_URL", "not-a-postgres-url"),
            ("JWT_SECRET", "pilot-secret-that-is-at-least-32-bytes-long"),
            ("MINT_URL", "https://test-mint.example.com"),
            ("PROVIDER_MODE", "sandbox"),
            ("CORS_ALLOWED_ORIGINS", "https://pilot.example.com"),
        ])
        .unwrap();

        let error = super::connect_database(&config).await.unwrap_err();
        assert!(!error.to_string().is_empty());
    }
}
