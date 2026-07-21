use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;
use std::collections::HashMap;
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
    /// OpenTelemetry (OTLP) export. Absent by default; when present with an
    /// endpoint, traces/metrics/logs are shipped to the collector. Requires the
    /// `otel` build feature — without it, this section is parsed but ignored.
    #[serde(default)]
    pub otel: Option<OtelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtelConfig {
    /// OTLP collector endpoint, e.g. `http://localhost:4317` (gRPC) or
    /// `http://localhost:4318` (HTTP). Its presence is what enables export.
    pub endpoint: String,
    /// `service.name` reported on every span/metric/log. Defaults to `raptor`.
    #[serde(default = "default_service_name")]
    pub service_name: String,
    /// OTLP wire protocol. Defaults to gRPC (the `endpoint` default port 4317).
    #[serde(default)]
    pub protocol: OtelProtocol,
    /// Extra headers sent to the collector (e.g. auth tokens for Datadog/Grafana
    /// Cloud). gRPC sends them as request metadata; HTTP as request headers.
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OtelProtocol {
    #[default]
    Grpc,
    Http,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DdiConfig {
    #[serde(default)]
    pub anonymous: bool,
    #[serde(default)]
    pub gateway_token: Option<String>,
    #[serde(default = "default_polling")]
    pub polling_interval: String,
    /// When true, a new assignment first requires confirmation (device- or
    /// operator-driven) before it becomes an active deployment. Mirrors
    /// hawkBit's `user.confirmation.flow.enabled` tenant flag. Off by default.
    #[serde(default)]
    pub confirmation_flow: bool,
}

impl Default for DdiConfig {
    fn default() -> Self {
        Self {
            anonymous: false,
            gateway_token: None,
            polling_interval: default_polling(),
            confirmation_flow: false,
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
fn default_service_name() -> String {
    "raptor".into()
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
            assert!(cfg.otel.is_none());
            Ok(())
        });
    }

    #[test]
    fn parses_otel_section_with_defaults() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "raptor.toml",
                &format!("{MINIMAL}\n[otel]\nendpoint = \"http://localhost:4317\"\n"),
            )?;
            let cfg = Config::load(Some(std::path::Path::new("raptor.toml"))).unwrap();
            let otel = cfg.otel.expect("otel section present");
            assert_eq!(otel.endpoint, "http://localhost:4317");
            assert_eq!(otel.service_name, "raptor");
            assert_eq!(otel.protocol, OtelProtocol::Grpc);
            assert!(otel.headers.is_empty());
            Ok(())
        });
    }

    #[test]
    fn parses_otel_protocol_and_headers() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "raptor.toml",
                &format!(
                    "{MINIMAL}\n[otel]\nendpoint = \"https://collector:4318\"\n\
                     service_name = \"raptor-edge\"\nprotocol = \"http\"\n\
                     [otel.headers]\nauthorization = \"Bearer tok\"\n"
                ),
            )?;
            let cfg = Config::load(Some(std::path::Path::new("raptor.toml"))).unwrap();
            let otel = cfg.otel.expect("otel section present");
            assert_eq!(otel.service_name, "raptor-edge");
            assert_eq!(otel.protocol, OtelProtocol::Http);
            assert_eq!(
                otel.headers.get("authorization").map(String::as_str),
                Some("Bearer tok")
            );
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
