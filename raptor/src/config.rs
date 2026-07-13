use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,
    pub database_url: String,
    pub artifact_dir: PathBuf,
    /// Max artifact upload size in bytes.
    #[serde(default = "default_max_artifact_size")]
    pub max_artifact_size: u64,
    /// External base URL used in _links; derived from the Host header when unset.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub ddi: DdiConfig,
    pub mgmt: MgmtConfig,
    /// How often the rollout group-threshold evaluator runs, in seconds.
    #[serde(default = "default_rollout_eval_interval_secs")]
    pub rollout_eval_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DdiConfig {
    #[serde(default)]
    pub anonymous: bool,
    #[serde(default)]
    pub gateway_token: Option<String>,
    #[serde(default = "default_polling")]
    pub polling_interval: String,
}

impl Default for DdiConfig {
    fn default() -> Self {
        Self {
            anonymous: false,
            gateway_token: None,
            polling_interval: default_polling(),
        }
    }
}

impl DdiConfig {
    /// Parse "HH:MM:SS"; falls back to 5 minutes on malformed input.
    pub fn polling_duration(&self) -> std::time::Duration {
        let parts: Vec<u64> = self
            .polling_interval
            .split(':')
            .filter_map(|p| p.parse().ok())
            .collect();
        match parts.as_slice() {
            [h, m, s] => std::time::Duration::from_secs(h * 3600 + m * 60 + s),
            _ => std::time::Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MgmtConfig {
    pub username: String,
    pub password_hash: String,
}

fn default_bind() -> SocketAddr {
    "0.0.0.0:8080".parse().unwrap()
}
fn default_max_artifact_size() -> u64 {
    1024 * 1024 * 1024
}
fn default_polling() -> String {
    "00:05:00".into()
}
fn default_rollout_eval_interval_secs() -> u64 {
    5
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self, Box<figment::Error>> {
        let mut fig = Figment::new();
        if let Some(p) = path {
            fig = fig.merge(Toml::file(p));
        }
        fig.merge(Env::prefixed("RAPTOR_").split("__"))
            .extract()
            .map_err(Box::new)
    }
}

#[cfg(test)]
#[allow(clippy::result_large_err)] // figment::Jail::expect_with's closure error type is fixed by the crate
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
database_url = "sqlite://test.db"
artifact_dir = "/tmp/artifacts"
[mgmt]
username = "admin"
password_hash = "$argon2id$fake"
"#;

    #[test]
    fn loads_minimal_toml_with_defaults() {
        figment::Jail::expect_with(|jail| {
            jail.create_file("raptor.toml", MINIMAL)?;
            let cfg = Config::load(Some(std::path::Path::new("raptor.toml"))).unwrap();
            assert_eq!(cfg.bind.to_string(), "0.0.0.0:8080");
            assert_eq!(cfg.database_url, "sqlite://test.db");
            assert_eq!(cfg.max_artifact_size, 1024 * 1024 * 1024);
            assert!(!cfg.ddi.anonymous);
            assert_eq!(cfg.ddi.polling_interval, "00:05:00");
            assert_eq!(cfg.mgmt.username, "admin");
            Ok(())
        });
    }

    #[test]
    fn env_overrides_toml() {
        figment::Jail::expect_with(|jail| {
            jail.create_file("raptor.toml", MINIMAL)?;
            jail.set_env("RAPTOR_BIND", "127.0.0.1:9999");
            jail.set_env("RAPTOR_DDI__ANONYMOUS", "true");
            jail.set_env("RAPTOR_DDI__GATEWAY_TOKEN", "gwsecret");
            let cfg = Config::load(Some(std::path::Path::new("raptor.toml"))).unwrap();
            assert_eq!(cfg.bind.to_string(), "127.0.0.1:9999");
            assert!(cfg.ddi.anonymous);
            assert_eq!(cfg.ddi.gateway_token.as_deref(), Some("gwsecret"));
            Ok(())
        });
    }

    #[test]
    fn polling_duration_parses_hhmmss() {
        let ddi = DdiConfig {
            polling_interval: "01:30:10".into(),
            ..Default::default()
        };
        assert_eq!(
            ddi.polling_duration(),
            std::time::Duration::from_secs(3600 + 30 * 60 + 10)
        );
    }
}
