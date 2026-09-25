use config::{Config, ConfigError, File};
use serde::Deserialize;
use std::{env, sync::Arc};

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub jwt: JwtConfig,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DatabaseConfig {
    pub url: String,
}

/// The secret this app signs and verifies every JWT with -- see
/// `crate::middleware::require_auth` and `routes::auth::login`. Never log
/// this value, for the same reason `db::connection::create_pool`'s
/// `skip_all` exists: it's a credential.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct JwtConfig {
    pub secret: String,
}

pub fn load_config() -> Result<Arc<AppConfig>, ConfigError> {
    dotenv::dotenv().ok();

    let mut builder = Config::builder().add_source(File::with_name("config/default"));

    if let Ok(value) = env::var("APP_SERVER_HOST") {
        builder = builder.set_override("server.host", value)?;
    }
    if let Ok(value) = env::var("APP_SERVER_PORT") {
        let port = value.parse::<u16>().map_err(|_| {
            ConfigError::Message(format!(
                "APP_SERVER_PORT must be an integer between 0 and 65535, got '{value}'"
            ))
        })?;
        builder = builder.set_override("server.port", port)?;
    }
    if let Ok(value) = env::var("APP_SERVER_LOG_LEVEL") {
        builder = builder.set_override("server.log_level", value)?;
    }
    if let Ok(value) = env::var("APP_DATABASE_URL") {
        builder = builder.set_override("database.url", value)?;
    }
    if let Ok(value) = env::var("APP_JWT_SECRET") {
        builder = builder.set_override("jwt.secret", value)?;
    }

    let config = builder.build()?.try_deserialize()?;
    Ok(Arc::new(config))
}
