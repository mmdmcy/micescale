use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::paths::{AUDIT_ENV, CONFIG_ENV, default_audit_path, default_config_path};

pub const SUPPORTED_CARRIERS: [&str; 2] = ["headscale", "wireguard"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub carrier: String,
    #[serde(default)]
    pub control_server: Option<String>,
    #[serde(default)]
    pub node_name: Option<String>,
    #[serde(default)]
    pub audit_log: Option<String>,
}

impl Config {
    pub fn new(carrier: String, control_server: Option<String>, node_name: Option<String>) -> Self {
        Self {
            carrier,
            control_server,
            node_name,
            audit_log: None,
        }
    }

    pub fn audit_path(&self) -> PathBuf {
        self.audit_log
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| env::var_os(AUDIT_ENV).map(PathBuf::from))
            .unwrap_or_else(default_audit_path)
    }
}

pub fn default_path() -> PathBuf {
    env::var_os(CONFIG_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(default_config_path)
}

pub fn load(path: &Path) -> Result<Option<Config>, CoreError> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(CoreError::Io(error)),
    };
    let config = toml::from_str::<Config>(&source).map_err(|error| {
        CoreError::Config(format!("invalid config at {}: {error}", path.display()))
    })?;
    validate(&config)?;
    Ok(Some(config))
}

pub fn save(path: &Path, config: &Config) -> Result<(), CoreError> {
    validate(config)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CoreError::Config(format!("cannot create {}: {error}", parent.display()))
        })?;
    }
    let rendered = toml::to_string_pretty(config).expect("config serializes");
    fs::write(path, rendered).map_err(CoreError::Io)
}

pub fn validate(config: &Config) -> Result<(), CoreError> {
    if !SUPPORTED_CARRIERS.contains(&config.carrier.as_str()) {
        return Err(CoreError::Config(format!(
            "unsupported carrier {:?}; supported: {}",
            config.carrier,
            SUPPORTED_CARRIERS.join(", ")
        )));
    }
    if config.carrier == "headscale" {
        let server = config
            .control_server
            .as_deref()
            .ok_or_else(|| CoreError::Config("headscale carrier requires control_server".into()))?;
        let https = server.strip_prefix("https://");
        let loopback_http = server.strip_prefix("http://").is_some_and(|rest| {
            let authority = rest.split('/').next().unwrap_or(rest);
            let host = authority
                .strip_prefix('[')
                .and_then(|bracketed| bracketed.split_once(']'))
                .map(|(host, _)| host)
                .unwrap_or_else(|| authority.split(':').next().unwrap_or(authority));
            host == "localhost" || host == "127.0.0.1" || host == "::1" || host.starts_with("127.")
        });
        if https.is_none() && !loopback_http {
            return Err(CoreError::Config(
                "control_server must be https://, or http:// on loopback only".into(),
            ));
        }
    }
    if config.audit_log.as_deref().is_some_and(str::is_empty) {
        return Err(CoreError::Config("audit_log must not be empty".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_toml() {
        let config = Config::new(
            "headscale".into(),
            Some("https://headscale.example.com".into()),
            Some("lab-node".into()),
        );
        let rendered = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&rendered).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn rejects_plain_http_remote_control_server() {
        let config = Config::new(
            "headscale".into(),
            Some("http://control.example.com".into()),
            None,
        );
        assert!(validate(&config).is_err());
    }

    #[test]
    fn accepts_loopback_http_for_dogfood() {
        for server in [
            "http://127.0.0.1:8080",
            "http://localhost:8080",
            "http://[::1]:8080",
        ] {
            let config = Config::new("headscale".into(), Some(server.into()), None);
            validate(&config).expect(server);
        }
    }

    #[test]
    fn rejects_unknown_carrier() {
        let config = Config::new(
            "magic".into(),
            Some("https://headscale.example.com".into()),
            None,
        );
        assert!(validate(&config).is_err());
    }

    #[test]
    fn wireguard_carrier_needs_no_control_server() {
        let config = Config::new("wireguard".into(), None, None);
        validate(&config).expect("wireguard carrier is valid without control_server");
    }
}
