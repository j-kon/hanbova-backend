use std::{collections::HashMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Test,
    Pilot,
    Production,
}

impl Environment {
    pub fn is_development(&self) -> bool {
        matches!(self, Self::Development)
    }

    pub fn is_test(&self) -> bool {
        matches!(self, Self::Test)
    }

    pub fn is_pilot(&self) -> bool {
        matches!(self, Self::Pilot)
    }

    pub fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Pilot => "pilot",
            Self::Production => "production",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderMode {
    Mock,
    Sandbox,
    Production,
}

impl ProviderMode {
    pub fn is_mock(&self) -> bool {
        matches!(self, Self::Mock)
    }

    pub fn is_sandbox(&self) -> bool {
        matches!(self, Self::Sandbox)
    }

    pub fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }
}

impl fmt::Display for ProviderMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Mock => "mock",
            Self::Sandbox => "sandbox",
            Self::Production => "production",
        })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid configuration: {problems}")]
pub struct ConfigError {
    pub problems: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub environment: Environment,
    pub host: String,
    pub port: u16,
    pub database_url: Option<String>,
    pub app_version: String,
    pub jwt_secret: String,
    pub mint_url: String,
    pub provider_mode: ProviderMode,
    pub cors_allowed_origins: Vec<String>,
    pub lightning_enabled: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_iter(std::env::vars())
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_iter<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let vars: HashMap<String, String> = vars
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();
        let mut problems = Vec::new();

        let environment = match vars.get("HANBOVA_ENV").map(String::as_str) {
            Some("development") => Environment::Development,
            Some("test") => Environment::Test,
            Some("pilot") => Environment::Pilot,
            Some("production") => Environment::Production,
            Some(value) => {
                problems.push(format!(
                    "HANBOVA_ENV must be development, test, pilot, or production (got {value})"
                ));
                Environment::Development
            }
            None => {
                problems.push("HANBOVA_ENV must be set explicitly".to_string());
                Environment::Development
            }
        };
        let pilot = environment.is_pilot();
        let production = environment.is_production();
        let pilot_or_prod = pilot || production;
        let env_label = environment.to_string();

        let host = vars
            .get("HANBOVA_API_HOST")
            .or_else(|| vars.get("HOST"))
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                if pilot_or_prod {
                    problems.push(format!("HANBOVA_API_HOST is required in {env_label}"));
                    "0.0.0.0".to_string()
                } else {
                    "127.0.0.1".to_string()
                }
            });

        let port_value = vars
            .get("HANBOVA_API_PORT")
            .or_else(|| vars.get("PORT"))
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                if pilot_or_prod {
                    problems.push(format!("HANBOVA_API_PORT is required in {env_label}"));
                    "8080".to_string()
                } else {
                    "8080".to_string()
                }
            });
        let port = port_value.parse::<u16>().unwrap_or_else(|_| {
            problems.push("HANBOVA_API_PORT must be a valid TCP port".to_string());
            8080
        });

        let database_url = vars
            .get("DATABASE_URL")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if pilot_or_prod && database_url.is_none() {
            problems.push(format!("DATABASE_URL is required in {env_label}"));
        }

        let jwt_secret = required_or_default(
            &vars,
            "JWT_SECRET",
            "hanbova-development-only-jwt-secret-change-me",
            if pilot_or_prod {
                Some(&env_label)
            } else {
                None
            },
            &mut problems,
        );
        if pilot_or_prod && jwt_secret.len() < 32 {
            problems.push(format!(
                "JWT_SECRET must be at least 32 bytes in {env_label}"
            ));
        }
        if pilot_or_prod && jwt_secret.contains("development") {
            problems.push("JWT_SECRET must not use the development default".to_string());
        }

        let mint_url = required_or_default(
            &vars,
            "MINT_URL",
            "http://127.0.0.1:3338",
            if pilot_or_prod {
                Some(&env_label)
            } else {
                None
            },
            &mut problems,
        );
        if pilot_or_prod && !mint_url.starts_with("https://") {
            problems.push(format!("MINT_URL must use HTTPS in {env_label}"));
        }
        if pilot_or_prod
            && (mint_url.contains("127.0.0.1")
                || mint_url.contains("localhost")
                || mint_url.contains("10.0.2.2"))
        {
            problems.push(format!(
                "MINT_URL must not point to localhost in {env_label}"
            ));
        }

        // Check external URL variables for forbidden localhost targets in pilot/production
        if pilot_or_prod {
            for (key, val) in &vars {
                if (key.ends_with("_BASE_URL") || key.ends_with("_URL"))
                    && key != "DATABASE_URL"
                    && (val.contains("127.0.0.1")
                        || val.contains("localhost")
                        || val.contains("10.0.2.2"))
                {
                    problems.push(format!("{key} must not point to localhost in {env_label}"));
                }
            }
        }

        let provider_mode = match vars.get("PROVIDER_MODE").map(String::as_str) {
            Some("mock") => ProviderMode::Mock,
            Some("sandbox") => ProviderMode::Sandbox,
            Some("production") => ProviderMode::Production,
            Some(value) => {
                problems.push(format!(
                    "PROVIDER_MODE must be mock, sandbox, or production (got {value})"
                ));
                ProviderMode::Mock
            }
            None if pilot => {
                problems.push("PROVIDER_MODE is required in pilot (must be sandbox)".to_string());
                ProviderMode::Sandbox
            }
            None if production => {
                problems.push("PROVIDER_MODE is required in production".to_string());
                ProviderMode::Production
            }
            None => ProviderMode::Mock,
        };

        if pilot && provider_mode != ProviderMode::Sandbox {
            problems.push(
                "PROVIDER_MODE must be sandbox in pilot (mock and production forbidden)"
                    .to_string(),
            );
        }

        if production && provider_mode != ProviderMode::Production {
            problems.push(
                "PROVIDER_MODE must be production in production (mock and sandbox forbidden)"
                    .to_string(),
            );
        }

        let cors_allowed_origins = vars
            .get("CORS_ALLOWED_ORIGINS")
            .map(|origins| {
                origins
                    .split(',')
                    .map(str::trim)
                    .filter(|origin| !origin.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if pilot_or_prod && cors_allowed_origins.is_empty() {
            problems.push(format!("CORS_ALLOWED_ORIGINS is required in {env_label}"));
        }
        for origin in &cors_allowed_origins {
            if origin == "*" && !pilot_or_prod {
                continue;
            }
            let valid = origin.parse::<axum::http::Uri>().ok().is_some_and(|uri| {
                matches!(uri.scheme_str(), Some("http" | "https"))
                    && uri.host().is_some_and(|host| !host.is_empty())
                    && uri
                        .authority()
                        .is_some_and(|authority| !authority.as_str().contains('@'))
                    && uri.path_and_query().is_none_or(|path| path.as_str() == "/")
                    && !origin.ends_with('/')
                    && axum::http::HeaderValue::from_str(origin).is_ok()
            });
            if !valid {
                problems.push("CORS_ALLOWED_ORIGINS must contain valid HTTP origins without paths, credentials or queries".to_string());
            }
        }
        if pilot_or_prod && cors_allowed_origins.iter().any(|origin| origin == "*") {
            problems.push(format!(
                "CORS_ALLOWED_ORIGINS must not contain a wildcard in {env_label}"
            ));
        }
        if pilot_or_prod
            && cors_allowed_origins
                .iter()
                .any(|origin| !origin.starts_with("https://"))
        {
            problems.push(format!(
                "CORS_ALLOWED_ORIGINS must use HTTPS in {env_label}"
            ));
        }

        if let Some(bitnob_env) = vars.get("BITNOB_ENVIRONMENT").map(String::as_str) {
            let bitnob_env_clean = bitnob_env.trim().to_lowercase();
            if provider_mode.is_sandbox() && bitnob_env_clean == "production" {
                problems.push(
                    "BITNOB_ENVIRONMENT cannot be production when PROVIDER_MODE is sandbox"
                        .to_string(),
                );
            } else if provider_mode.is_production() && bitnob_env_clean == "sandbox" {
                problems.push(
                    "BITNOB_ENVIRONMENT cannot be sandbox when PROVIDER_MODE is production"
                        .to_string(),
                );
            }
        }

        let lightning_enabled = match vars.get("LIGHTNING_ENABLED").map(String::as_str) {
            Some("true" | "1" | "yes") => true,
            Some("false" | "0" | "no") => false,
            Some(val) => {
                problems.push(format!("LIGHTNING_ENABLED must be a boolean (got {val})"));
                false
            }
            None => {
                // In Development/Test, mock lightning is allowed by default.
                // In Pilot/Production, Lightning defaults to disabled unless explicitly enabled.
                !pilot_or_prod
            }
        };

        if pilot && lightning_enabled {
            problems.push(
                "Lightning cannot be enabled in pilot until a non-mock provider is configured"
                    .to_string(),
            );
        }

        if !problems.is_empty() {
            return Err(ConfigError {
                problems: problems.join("; "),
            });
        }

        Ok(Self {
            environment,
            host,
            port,
            database_url,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            jwt_secret,
            mint_url,
            provider_mode,
            cors_allowed_origins,
            lightning_enabled,
        })
    }

    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn is_development(&self) -> bool {
        self.environment.is_development()
    }

    pub fn is_test(&self) -> bool {
        self.environment.is_test()
    }

    pub fn is_pilot(&self) -> bool {
        self.environment.is_pilot()
    }

    pub fn is_production(&self) -> bool {
        self.environment.is_production()
    }
}

fn required_or_default(
    vars: &HashMap<String, String>,
    name: &str,
    default: &str,
    required_in: Option<&str>,
    problems: &mut Vec<String>,
) -> String {
    match vars
        .get(name)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        Some(value) => value.to_string(),
        None if required_in.is_some() => {
            problems.push(format!("{name} is required in {}", required_in.unwrap()));
            default.to_string()
        }
        None => default.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_production_vars() -> Vec<(&'static str, &'static str)> {
        vec![
            ("HANBOVA_ENV", "production"),
            ("HANBOVA_API_HOST", "0.0.0.0"),
            ("HANBOVA_API_PORT", "8080"),
            ("DATABASE_URL", "postgres://hanbova:secret@db/hanbova"),
            ("JWT_SECRET", "production-secret-that-is-at-least-32-bytes"),
            ("MINT_URL", "https://mint.example.com"),
            ("PROVIDER_MODE", "production"),
            ("CORS_ALLOWED_ORIGINS", "https://app.example.com"),
        ]
    }

    fn valid_pilot_vars() -> Vec<(&'static str, &'static str)> {
        vec![
            ("HANBOVA_ENV", "pilot"),
            ("HANBOVA_API_HOST", "0.0.0.0"),
            ("HANBOVA_API_PORT", "8080"),
            ("DATABASE_URL", "postgres://hanbova:secret@db/hanbova"),
            (
                "JWT_SECRET",
                "pilot-secure-secret-that-is-at-least-32-bytes",
            ),
            ("MINT_URL", "https://test-mint.example.com"),
            ("PROVIDER_MODE", "sandbox"),
            ("CORS_ALLOWED_ORIGINS", "https://pilot.example.com"),
        ]
    }

    #[test]
    fn production_configuration_is_accepted_when_complete() {
        let config = AppConfig::from_iter(valid_production_vars()).unwrap();
        assert!(config.is_production());
        assert_eq!(config.provider_mode, ProviderMode::Production);
        assert!(!config.lightning_enabled);
    }

    #[test]
    fn pilot_configuration_is_accepted_when_complete() {
        let config = AppConfig::from_iter(valid_pilot_vars()).unwrap();
        assert!(config.is_pilot());
        assert_eq!(config.provider_mode, ProviderMode::Sandbox);
        assert!(!config.lightning_enabled);
    }

    #[test]
    fn pilot_rejects_mock_provider() {
        let mut vars = valid_pilot_vars();
        vars.retain(|(k, _)| *k != "PROVIDER_MODE");
        vars.push(("PROVIDER_MODE", "mock"));
        let err = AppConfig::from_iter(vars).unwrap_err();
        assert!(err.to_string().contains("sandbox in pilot"));
    }

    #[test]
    fn pilot_rejects_production_provider() {
        let mut vars = valid_pilot_vars();
        vars.retain(|(k, _)| *k != "PROVIDER_MODE");
        vars.push(("PROVIDER_MODE", "production"));
        let err = AppConfig::from_iter(vars).unwrap_err();
        assert!(err.to_string().contains("sandbox in pilot"));
    }

    #[test]
    fn pilot_rejects_missing_database() {
        let mut vars = valid_pilot_vars();
        vars.retain(|(k, _)| *k != "DATABASE_URL");
        let err = AppConfig::from_iter(vars).unwrap_err();
        assert!(err
            .to_string()
            .contains("DATABASE_URL is required in pilot"));
    }

    #[test]
    fn pilot_rejects_weak_or_development_jwt_secret() {
        let mut vars = valid_pilot_vars();
        vars.retain(|(k, _)| *k != "JWT_SECRET");
        vars.push(("JWT_SECRET", "short"));
        let err = AppConfig::from_iter(vars).unwrap_err();
        assert!(err
            .to_string()
            .contains("JWT_SECRET must be at least 32 bytes"));

        let mut vars2 = valid_pilot_vars();
        vars2.retain(|(k, _)| *k != "JWT_SECRET");
        vars2.push((
            "JWT_SECRET",
            "development-secret-that-is-at-least-32-bytes-long",
        ));
        let err2 = AppConfig::from_iter(vars2).unwrap_err();
        assert!(err2
            .to_string()
            .contains("must not use the development default"));
    }

    #[test]
    fn pilot_rejects_localhost_mint_url() {
        let mut vars = valid_pilot_vars();
        vars.retain(|(k, _)| *k != "MINT_URL");
        vars.push(("MINT_URL", "https://localhost:3338"));
        let err = AppConfig::from_iter(vars).unwrap_err();
        assert!(err
            .to_string()
            .contains("must not point to localhost in pilot"));
    }

    #[test]
    fn pilot_rejects_wildcard_cors() {
        let mut vars = valid_pilot_vars();
        vars.retain(|(k, _)| *k != "CORS_ALLOWED_ORIGINS");
        vars.push(("CORS_ALLOWED_ORIGINS", "*"));
        let err = AppConfig::from_iter(vars).unwrap_err();
        assert!(err
            .to_string()
            .contains("must not contain a wildcard in pilot"));
    }

    #[test]
    fn pilot_supports_host_and_port_fallbacks() {
        let mut vars = valid_pilot_vars();
        vars.retain(|(k, _)| *k != "HANBOVA_API_HOST" && *k != "HANBOVA_API_PORT");
        vars.push(("HOST", "0.0.0.0"));
        vars.push(("PORT", "9090"));
        let config = AppConfig::from_iter(vars).unwrap();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9090);
    }

    #[test]
    fn development_allows_mock_and_sandbox() {
        let dev_mock =
            AppConfig::from_iter([("HANBOVA_ENV", "development"), ("PROVIDER_MODE", "mock")])
                .unwrap();
        assert!(dev_mock.is_development());
        assert_eq!(dev_mock.provider_mode, ProviderMode::Mock);

        let dev_sandbox =
            AppConfig::from_iter([("HANBOVA_ENV", "development"), ("PROVIDER_MODE", "sandbox")])
                .unwrap();
        assert_eq!(dev_sandbox.provider_mode, ProviderMode::Sandbox);
    }

    #[test]
    fn production_rejects_sandbox_provider() {
        let mut vars = valid_production_vars();
        vars.retain(|(k, _)| *k != "PROVIDER_MODE");
        vars.push(("PROVIDER_MODE", "sandbox"));
        let err = AppConfig::from_iter(vars).unwrap_err();
        assert!(err.to_string().contains("must be production in production"));
    }

    #[test]
    fn rejects_malformed_cors_origins_before_router_construction() {
        for origin in [
            "https://app.example.com\ninvalid",
            "https://",
            "https://app.example.com/path",
            "https://app.example.com?query=1",
        ] {
            let mut vars = valid_production_vars();
            vars.retain(|(key, _)| *key != "CORS_ALLOWED_ORIGINS");
            vars.push(("CORS_ALLOWED_ORIGINS", origin));
            assert!(AppConfig::from_iter(vars).is_err(), "accepted {origin:?}");
        }
    }

    #[test]
    fn production_rejects_missing_database_and_short_secret() {
        let vars = [
            ("HANBOVA_ENV", "production"),
            ("HANBOVA_API_HOST", "0.0.0.0"),
            ("HANBOVA_API_PORT", "8080"),
            ("JWT_SECRET", "short"),
            ("MINT_URL", "https://mint.example.com"),
            ("PROVIDER_MODE", "production"),
        ];
        let error = AppConfig::from_iter(vars).unwrap_err();
        assert!(error.to_string().contains("DATABASE_URL"));
        assert!(error.to_string().contains("JWT_SECRET"));
    }

    #[test]
    fn production_rejects_http_mint_and_mock_provider() {
        let mut vars = valid_production_vars();
        vars.retain(|(name, _)| *name != "MINT_URL" && *name != "PROVIDER_MODE");
        vars.extend([
            ("MINT_URL", "http://mint.example.com"),
            ("PROVIDER_MODE", "mock"),
        ]);
        let error = AppConfig::from_iter(vars).unwrap_err();
        assert!(error.to_string().contains("HTTPS"));
        assert!(error.to_string().contains("mock"));
    }

    #[test]
    fn production_rejects_wildcard_cors_origin() {
        let mut vars = valid_production_vars();
        vars.retain(|(name, _)| *name != "CORS_ALLOWED_ORIGINS");
        vars.push(("CORS_ALLOWED_ORIGINS", "*"));

        let error = AppConfig::from_iter(vars).unwrap_err();
        assert!(error.to_string().contains("wildcard"));
    }

    #[test]
    fn environment_must_be_explicit_and_supported() {
        assert!(AppConfig::from_iter(Vec::<(&str, &str)>::new()).is_err());
        assert!(AppConfig::from_iter([("HANBOVA_ENV", "staging")]).is_err());
    }

    #[test]
    fn explicit_development_uses_safe_local_defaults() {
        let config = AppConfig::from_iter([("HANBOVA_ENV", "development")]).unwrap();
        assert!(config.is_development());
        assert_eq!(config.provider_mode, ProviderMode::Mock);
        assert_eq!(config.host, "127.0.0.1");
        assert!(config.lightning_enabled);
    }
}
