//! `Config`: deserialized from `raptor.toml` (and `RAPTOR_`-prefixed env
//! overrides) via figment. Owns only parsing/defaults — no validation beyond
//! what `serde` gives for free, and no I/O besides the file/env sources.

use figment::Figment;
use figment::providers::{Env, Format, Toml};
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
    /// Tenant name this instance answers to on the DDI `/{tenant}/controller/v1/...`
    /// path segment. DDI requests naming any other tenant are rejected —
    /// raptor is single-tenant by design (run one instance per fleet); see
    /// docs/superpowers/specs/2026-08-26-multi-tenancy-design.md. Compared
    /// case-insensitively, since Zephyr's `CONFIG_HAWKBIT_TENANT` defaults to
    /// `"default"`.
    #[serde(default = "default_tenant")]
    pub tenant: String,
    /// How often the rollout group-threshold evaluator runs, in seconds.
    #[serde(default = "default_rollout_eval_interval_secs")]
    pub rollout_eval_interval_secs: u64,
    /// When true, a newly created rollout lands in `waiting_for_approval` and
    /// cannot be started until an operator approves it. Mirrors hawkBit's
    /// `rollout.approval.enabled` tenant flag, which is what this value is
    /// reported as on `/rest/v1/system/configs`. Off by default, so rollouts
    /// stay directly startable unless an operator asks for the gate.
    #[serde(default)]
    pub rollout_approval_enabled: bool,
    /// Per-entity growth caps, mirroring hawkBit's quotas. Every key defaults
    /// to hawkBit's own default, and `0` means unlimited.
    #[serde(default)]
    pub quota: QuotaConfig,
    /// OpenTelemetry (OTLP) export. Absent by default; when present with an
    /// endpoint, traces/metrics/logs are shipped to the collector. Requires the
    /// `otel` build feature — without it, this section is parsed but ignored.
    #[serde(default)]
    pub otel: Option<OtelConfig>,
}

/// hawkBit's per-entity quotas (`hawkbit.server.security.dos.*`), with its
/// default values. These bound unbounded growth — a chatty device appending
/// action-status rows forever, a runaway upload loop — rather than implementing
/// a rate limit.
///
/// `0` disables a quota, matching hawkBit's own `QuotaHelper`, which treats any
/// `limit <= 0` as unlimited. Violations are reported as `429 Too Many
/// Requests` with `hawkbit.server.error.quota.tooManyEntries`.
#[derive(Debug, Clone, Deserialize)]
pub struct QuotaConfig {
    /// Status entries a device may report against one action.
    #[serde(default = "d_1000")]
    pub max_status_entries_per_action: u32,
    /// Messages a device may attach to one reported status entry.
    #[serde(default = "d_50")]
    pub max_messages_per_action_status: u32,
    /// Attributes a device may report about itself.
    #[serde(default = "d_100")]
    pub max_attribute_entries_per_target: u32,
    #[serde(default = "d_100")]
    pub max_metadata_entries_per_target: u32,
    #[serde(default = "d_100")]
    pub max_metadata_entries_per_software_module: u32,
    #[serde(default = "d_100")]
    pub max_metadata_entries_per_distribution_set: u32,
    #[serde(default = "d_50")]
    pub max_artifacts_per_software_module: u32,
    #[serde(default = "d_100")]
    pub max_software_modules_per_distribution_set: u32,
    #[serde(default = "d_500")]
    pub max_rollout_groups_per_rollout: u32,
    #[serde(default = "d_20000")]
    pub max_targets_per_rollout_group: u32,
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            max_status_entries_per_action: d_1000(),
            max_messages_per_action_status: d_50(),
            max_attribute_entries_per_target: d_100(),
            max_metadata_entries_per_target: d_100(),
            max_metadata_entries_per_software_module: d_100(),
            max_metadata_entries_per_distribution_set: d_100(),
            max_artifacts_per_software_module: d_50(),
            max_software_modules_per_distribution_set: d_100(),
            max_rollout_groups_per_rollout: d_500(),
            max_targets_per_rollout_group: d_20000(),
        }
    }
}

fn d_50() -> u32 {
    50
}
fn d_100() -> u32 {
    100
}
fn d_500() -> u32 {
    500
}
fn d_1000() -> u32 {
    1000
}
fn d_20000() -> u32 {
    20000
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
    /// `auto_confirm` applied to newly created targets. Escape hatch for
    /// `confirmation_flow`: clients that don't implement `confirmationBase`
    /// (the mainline Zephyr hawkbit client, for one) never see a link they
    /// understand and would otherwise poll forever without installing.
    #[serde(default)]
    pub auto_confirm_default: bool,
    /// Plain-HTTP base URL to advertise in the DDI `download-http` /
    /// `md5sum-http` artifact links, e.g. `http://ota.example.com`. Set this
    /// only when plain HTTP really is reachable — devices that use
    /// `download-http` (the Zephyr hawkbit client does) will follow it. When
    /// unset, those links reuse the normal base `url`, whatever its scheme.
    #[serde(default)]
    pub artifact_http_url: Option<String>,
    /// Header carrying the real device address when raptor runs behind a reverse
    /// proxy, e.g. `x-forwarded-for` or `forwarded`. Unset by default: the
    /// header is trivially spoofable by a device, so the socket peer is used
    /// unless an operator states that a proxy is in front and rewriting it.
    #[serde(default)]
    pub trusted_proxy_header: Option<String>,
}

impl Default for DdiConfig {
    fn default() -> Self {
        Self {
            anonymous: false,
            gateway_token: None,
            polling_interval: default_polling(),
            confirmation_flow: false,
            auto_confirm_default: false,
            artifact_http_url: None,
            trusted_proxy_header: None,
        }
    }
}

impl DdiConfig {
    /// Parses `polling_interval` into a [`PollingSchedule`]. See
    /// [`parse_polling_schedule`] for the grammar.
    pub fn polling_schedule(&self) -> Result<PollingSchedule, String> {
        parse_polling_schedule(&self.polling_interval)
    }

    /// The default interval's base duration — no jitter, no override rules.
    /// Used only for the Management API's `pollStatus.overdue` estimate,
    /// which hawkBit itself computes from the default interval regardless of
    /// which override (if any) a target currently matches — a documented
    /// upstream limitation (eclipse-hawkbit/hawkbit#2533: "overdue time is
    /// calculated according to the default polling time"), mirrored here
    /// rather than fixed, since fixing it would mean re-evaluating every
    /// target's rules on every list page just for a cosmetic figure.
    /// Falls back to 5 minutes on malformed input, matching this method's
    /// pre-override behavior.
    pub fn polling_duration(&self) -> std::time::Duration {
        self.polling_schedule()
            .map(|s| s.default.duration)
            .unwrap_or(std::time::Duration::from_secs(300))
    }
}

/// A single `HH:MM:SS` interval with an optional `~NN%` jitter, `N` in
/// `0..=99` — hawkBit's `pollingTime` grammar (PR
/// eclipse-hawkbit/hawkbit#2533), minus the `d+:HH:mm:ss` and ISO-8601
/// duration forms: raptor's incident-shaped use case ("poll this device
/// faster while I'm watching it") doesn't need day-scale intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollingInterval {
    pub duration: std::time::Duration,
    pub deviation_percent: u8,
}

impl PollingInterval {
    /// Applies `~NN%` jitter — fresh randomness on every call, matching
    /// hawkBit's own `Random.nextLong` per-evaluation behavior rather than
    /// something derived from the target (source: `PollingTime.PollingInterval
    /// #getFormattedIntervalWithDeviation`) — and formats the result as
    /// `HH:MM:SS` for the DDI poll response.
    pub fn resolve(&self) -> String {
        let millis = self.duration.as_millis() as i64;
        let jittered = if self.deviation_percent > 0 {
            use rand::RngExt;
            let max_dev = millis * self.deviation_percent as i64 / 100;
            millis + rand::rng().random_range(-max_dev..=max_dev)
        } else {
            millis
        };
        format_hhmmss(std::time::Duration::from_millis(jittered.max(0) as u64))
    }
}

/// One `<RSQL> -> <interval>` override rule, evaluated in written order —
/// first match wins.
#[derive(Debug, Clone, PartialEq)]
pub struct PollingRule {
    pub filter: String,
    pub interval: PollingInterval,
}

/// A default [`PollingInterval`] plus ordered override [`PollingRule`]s.
#[derive(Debug, Clone, PartialEq)]
pub struct PollingSchedule {
    pub default: PollingInterval,
    pub rules: Vec<PollingRule>,
}

/// Caps the number of override rules a config may declare. Rules are
/// evaluated with one indexed query each on every DDI poll (see
/// `api::ddi::root::resolve_polling_interval`); this bounds the worst case
/// for what's meant to be a handful of incident-shaped overrides, not a
/// per-device routing table.
const MAX_POLLING_RULES: usize = 20;

/// Parses hawkBit's `pollingTime` value grammar:
/// `<default>[, <RSQL, no top-level commas> -> <interval>]*`. A plain
/// `str::split(',')` is safe here because hawkBit's own override grammar
/// (`OVERRIDE_PATTERN`'s `qlStr` group is `[^,]*`) already forbids a comma
/// inside an override's RSQL filter — so there is nothing this parser needs
/// to reimplement from the FIQL grammar just to find the split points.
///
/// The filter itself is *not* parsed here — only extracted as a substring —
/// and is later compiled through `api::mgmt::targets::condition`, i.e.
/// raptor's own FIQL dialect (used everywhere raptor accepts a `q=` filter),
/// not the fuller Spring RSQL grammar hawkBit's PR examples are written in.
/// In particular raptor's parser does not tolerate whitespace around an
/// operator: hawkBit's own doc example `group == 'eu'` must be written
/// `group==eu` here.
fn parse_polling_schedule(raw: &str) -> Result<PollingSchedule, String> {
    let mut segments = raw.split(',');
    let default = parse_polling_interval(segments.next().unwrap_or("").trim())
        .map_err(|e| format!("invalid default pollingTime: {e}"))?;

    let mut rules = Vec::new();
    for segment in segments {
        let segment = segment.trim();
        let (filter, interval) = segment
            .split_once("->")
            .ok_or_else(|| format!("invalid pollingTime override {segment:?}: missing '->'"))?;
        let filter = filter.trim();
        if filter.is_empty() {
            return Err(format!(
                "invalid pollingTime override {segment:?}: empty filter"
            ));
        }
        let interval = parse_polling_interval(interval.trim())
            .map_err(|e| format!("invalid pollingTime override {segment:?}: {e}"))?;
        rules.push(PollingRule {
            filter: filter.to_string(),
            interval,
        });
    }
    if rules.len() > MAX_POLLING_RULES {
        return Err(format!(
            "pollingTime declares {} override rules, more than the {MAX_POLLING_RULES} supported",
            rules.len()
        ));
    }
    Ok(PollingSchedule { default, rules })
}

/// Parses one `HH:MM:SS` or `HH:MM:SS~NN%` interval.
fn parse_polling_interval(s: &str) -> Result<PollingInterval, String> {
    let (time, pct) = match s.split_once('~') {
        Some((t, p)) => (t.trim(), Some(p.trim())),
        None => (s, None),
    };
    let parts: Vec<&str> = time.split(':').collect();
    let [h, m, sec] = parts.as_slice() else {
        return Err(format!("{s:?}: expected HH:MM:SS"));
    };
    let h: u64 = h.parse().map_err(|_| format!("{s:?}: invalid hours"))?;
    let m: u64 = m.parse().map_err(|_| format!("{s:?}: invalid minutes"))?;
    let sec: u64 = sec.parse().map_err(|_| format!("{s:?}: invalid seconds"))?;
    let duration = std::time::Duration::from_secs(h * 3600 + m * 60 + sec);

    let deviation_percent = match pct {
        None => 0,
        Some(p) => {
            let digits = p
                .strip_suffix('%')
                .ok_or_else(|| format!("{s:?}: expected ~NN%"))?;
            let n: u8 = digits
                .parse()
                .map_err(|_| format!("{s:?}: invalid deviation percent"))?;
            if n > 99 {
                return Err(format!("{s:?}: deviation percent must be 0-99"));
            }
            n
        }
    };
    Ok(PollingInterval {
        duration,
        deviation_percent,
    })
}

fn format_hhmmss(d: std::time::Duration) -> String {
    let total = d.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

#[derive(Debug, Clone, Deserialize)]
pub struct MgmtConfig {
    pub username: String,
    pub password_hash: String,
}

/// 8088 rather than 8080: still unprivileged, but far less likely to collide
/// with whatever else is already listening on a developer's machine.
fn default_bind() -> SocketAddr {
    "0.0.0.0:8088".parse().unwrap()
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
fn default_tenant() -> String {
    "DEFAULT".into()
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
            assert_eq!(cfg.bind.to_string(), "0.0.0.0:8088");
            assert_eq!(cfg.database_url, "sqlite://test.db");
            assert_eq!(cfg.max_artifact_size, 1024 * 1024 * 1024);
            assert!(!cfg.ddi.anonymous);
            assert_eq!(cfg.ddi.polling_interval, "00:05:00");
            assert_eq!(cfg.mgmt.username, "admin");
            assert_eq!(cfg.tenant, "DEFAULT");
            assert!(!cfg.rollout_approval_enabled);
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

    #[test]
    fn polling_schedule_bare_interval_has_no_rules() {
        let schedule = parse_polling_schedule("00:05:00").unwrap();
        assert_eq!(
            schedule.default.duration,
            std::time::Duration::from_secs(300)
        );
        assert_eq!(schedule.default.deviation_percent, 0);
        assert!(schedule.rules.is_empty());
    }

    #[test]
    fn polling_schedule_parses_default_jitter() {
        let schedule = parse_polling_schedule("01:00:00~10%").unwrap();
        assert_eq!(
            schedule.default.duration,
            std::time::Duration::from_secs(3600)
        );
        assert_eq!(schedule.default.deviation_percent, 10);
    }

    /// The exact string from hawkBit's own PR description (eclipse-hawkbit/hawkbit#2533).
    #[test]
    fn polling_schedule_parses_hawkbit_example_verbatim() {
        let schedule = parse_polling_schedule(
            "01:00:00~10%, group == 'eu' -> 00:02:00~15%, status != in_sync -> 00:05:00",
        )
        .unwrap();
        assert_eq!(
            schedule.default.duration,
            std::time::Duration::from_secs(3600)
        );
        assert_eq!(schedule.default.deviation_percent, 10);
        assert_eq!(schedule.rules.len(), 2);
        assert_eq!(schedule.rules[0].filter, "group == 'eu'");
        assert_eq!(
            schedule.rules[0].interval.duration,
            std::time::Duration::from_secs(120)
        );
        assert_eq!(schedule.rules[0].interval.deviation_percent, 15);
        assert_eq!(schedule.rules[1].filter, "status != in_sync");
        assert_eq!(
            schedule.rules[1].interval.duration,
            std::time::Duration::from_secs(300)
        );
        assert_eq!(schedule.rules[1].interval.deviation_percent, 0);
    }

    /// Whitespace variants from hawkBit's own `PollingTimeTest` (PR #2533).
    #[test]
    fn polling_schedule_tolerates_hawkbit_whitespace_variants() {
        for s in [
            "01:00:00~10%, group == 'eu'  -> 00:02:00~15%, status != in_sync ->00:05:00",
            " 01:00:00~10%, group == 'eu'  -> 00:02:00~15%, status != in_sync ->00:05:00  ",
            " 01:00:00~10% , group == 'eu'  -> 00:02:00 ~15%, status != in_sync ->00:05:00  ",
        ] {
            let schedule = parse_polling_schedule(s).unwrap();
            assert_eq!(schedule.rules.len(), 2, "input: {s:?}");
        }
    }

    #[test]
    fn polling_schedule_rejects_malformed_input() {
        for bad in [
            "not-a-time",
            "01:00:00, group == 'eu'",               // rule missing '->'
            "01:00:00, -> 00:05:00",                 // empty filter
            "01:00:00, group == 'eu' -> not-a-time", // bad rule interval
            "01:00:00~120%",                         // deviation out of 0-99 range
        ] {
            assert!(
                parse_polling_schedule(bad).is_err(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn polling_schedule_rejects_too_many_rules() {
        let rules = (0..=MAX_POLLING_RULES)
            .map(|i| format!("controllerId == 'dev-{i}' -> 00:01:00"))
            .collect::<Vec<_>>()
            .join(", ");
        assert!(parse_polling_schedule(&format!("00:05:00, {rules}")).is_err());
    }

    #[test]
    fn polling_interval_resolve_without_jitter_is_exact_and_zero_padded() {
        let interval = PollingInterval {
            duration: std::time::Duration::from_secs(300),
            deviation_percent: 0,
        };
        assert_eq!(interval.resolve(), "00:05:00");
    }

    #[test]
    fn polling_interval_resolve_with_jitter_stays_in_bounds() {
        let interval = PollingInterval {
            duration: std::time::Duration::from_secs(1000),
            deviation_percent: 10,
        };
        for _ in 0..200 {
            let resolved = interval.resolve();
            let parts: Vec<u64> = resolved.split(':').map(|p| p.parse().unwrap()).collect();
            let secs = parts[0] * 3600 + parts[1] * 60 + parts[2];
            assert!(
                (900..=1100).contains(&secs),
                "{resolved} out of ±10% bounds"
            );
        }
    }

    /// A no-rules, no-jitter config must round-trip byte-identically through
    /// `resolve()` — the DDI golden-fixture guarantee this feature must not
    /// break for stock clients (SWUpdate, rauc-hawkbit-updater).
    #[test]
    fn polling_schedule_default_resolves_byte_identical_to_configured_string() {
        for configured in ["00:05:00", "01:30:10", "23:59:59"] {
            let schedule = parse_polling_schedule(configured).unwrap();
            assert_eq!(schedule.default.resolve(), configured);
        }
    }
}
