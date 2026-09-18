use config::{Config, ConfigError, File};
use serde::Deserialize;
use std::{env, sync::Arc};

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
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

    let config = builder.build()?.try_deserialize()?;
    Ok(Arc::new(config))
}
