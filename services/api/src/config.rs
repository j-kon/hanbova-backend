use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub env: String,
    pub host: String,
    pub port: u16,
    pub database_url: Option<String>,
    pub app_version: String,
    pub jwt_secret: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let env = env::var("HANBOVA_ENV").unwrap_or_else(|_| "development".to_string());
        let host = env::var("HANBOVA_API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("HANBOVA_API_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        let database_url = env::var("DATABASE_URL").ok();
        let app_version = env!("CARGO_PKG_VERSION").to_string();
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
            "hanbova_dev_jwt_secret_key_change_in_production_32bytes".to_string()
        });

        Self {
            env,
            host,
            port,
            database_url,
            app_version,
            jwt_secret,
        }
    }

    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn is_development(&self) -> bool {
        self.env == "development"
    }
}
