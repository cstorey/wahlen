use std::collections::HashMap;
use std::time::Duration;

use failure::{Error, ResultExt};
use log::*;
use r2d2::Pool;
use r2d2_postgres::{PostgresConnectionManager, TlsMode};
use serde::{Deserialize, Serialize};

use infra::persistence;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Config {
    pub postgres: PgConfig,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct PgConfig {
    pub url: String,
    max_size: Option<u32>,
    min_idle: Option<u32>,
    max_lifetime: Option<Duration>,
    idle_timeout: Option<Duration>,
    connection_timeout: Option<Duration>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl PgConfig {
    pub(crate) fn build(&self) -> Result<Pool<persistence::DocumentConnectionManager>, Error> {
        debug!("Build pool from {:?}", self);

        let manager = persistence::DocumentConnectionManager::new(
            PostgresConnectionManager::new(&*self.url, TlsMode::None)
                .context("connection manager")?,
        );

        let mut builder = r2d2::Pool::builder();

        if let Some(max_size) = self.max_size {
            builder = builder.max_size(max_size);
        }
        if let Some(min_idle) = self.min_idle {
            builder = builder.min_idle(Some(min_idle));
        }
        if let Some(max_lifetime) = self.max_lifetime {
            builder = builder.max_lifetime(Some(max_lifetime));
        }
        if let Some(idle_timeout) = self.idle_timeout {
            builder = builder.idle_timeout(Some(idle_timeout));
        }
        if let Some(connection_timeout) = self.connection_timeout {
            builder = builder.connection_timeout(connection_timeout);
        }

        debug!("Pool builder: {:?}", builder);
        let pool = builder.build(manager).context("build pool")?;

        Ok(pool)
    }
}

#[derive(Deserialize, Debug)]
pub struct EnvLogger {
    level: Option<LogLevel>,
    modules: HashMap<String, LogLevel>,
    timestamp_nanos: bool,
}

impl LogLevel {
    fn to_filter(&self) -> log::LevelFilter {
        match *self {
            LogLevel::Off => log::LevelFilter::Off,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

impl EnvLogger {
    pub fn builder(&self) -> env_logger::Builder {
        let mut b = env_logger::Builder::from_default_env();
        if let Some(level) = self.level.as_ref() {
            b.filter_level(level.to_filter());
        }

        for (module, level) in self.modules.iter() {
            b.filter_module(&module, level.to_filter());
        }

        b.default_format_timestamp_nanos(self.timestamp_nanos);

        b
    }
}
