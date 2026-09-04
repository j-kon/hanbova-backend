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

    #[test]
    fn production_refuses_missing_pool() {
        let error = match AppState::try_new(production_config(), None) {
            Ok(_) => panic!("production must not start without PostgreSQL"),
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
            Ok(_) => panic!("production must not start with mock providers"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("providers"));
    }
}
