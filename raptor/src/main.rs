//! `raptor` binary: CLI entry point. `serve` loads config, runs migrations,
//! spawns the rollout/auto-assignment sweep loop, and starts the axum server
//! built by `app::build_app`; `hash-password` is a standalone utility for
//! generating `raptor.toml` credentials. All request-handling logic lives in
//! the library crate.

use clap::Parser;
use migration::MigratorTrait;
use raptor::config::Config;
use raptor::state::AppState;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "raptor", version)]
enum Cmd {
    /// Run the server
    Serve {
        #[arg(long, default_value = "raptor.toml")]
        config: PathBuf,
    },
    /// Read a password from stdin and print its argon2id hash for raptor.toml
    HashPassword,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cmd::parse() {
        Cmd::HashPassword => {
            use argon2::password_hash::{PasswordHasher, SaltString};
            let mut pw = String::new();
            std::io::stdin().read_line(&mut pw)?;
            let salt = SaltString::encode_b64(&raptor::util::random_salt())
                .map_err(|e| format!("salt encode error: {e}"))?;
            match argon2::Argon2::default().hash_password(pw.trim_end().as_bytes(), &salt) {
                Ok(hash) => println!("{hash}"),
                Err(e) => return Err(format!("hash password error: {}", e).into()),
            }
        }
        Cmd::Serve { config } => {
            // Config first: telemetry init needs the [otel] section, and it
            // installs the subscriber before anything else logs.
            let cfg = Config::load(Some(&config))?;
            validate_polling_schedule(&cfg)?;
            let (telemetry, metrics) = raptor::telemetry::init(cfg.otel.as_ref())?;
            if let Some(u) = &cfg.ddi.artifact_http_url
                && !u.starts_with("http://")
            {
                tracing::warn!(
                    url = %u,
                    "[ddi] artifact_http_url is not an http:// URL; it is advertised as the \
                     plain-HTTP artifact link (download-http), so devices will follow it as-is."
                );
            }
            if cfg.ddi.confirmation_flow && !cfg.ddi.auto_confirm_default {
                tracing::warn!(
                    "[ddi] confirmation_flow is enabled: assignments wait for a confirmationBase \
                     call. Clients that don't implement confirmationBase (including the mainline \
                     Zephyr hawkbit client) will poll forever without installing. Set [ddi] \
                     auto_confirm_default = true, or activate autoConfirm per target."
                );
            }

            let db = sea_orm::Database::connect(&cfg.database_url).await?;
            migration::Migrator::up(&db, None).await?;
            let store = raptor::storage::ArtifactStore::new(cfg.artifact_dir.clone())?;
            let bind = cfg.bind;
            let eval_interval = cfg.rollout_eval_interval_secs.max(1);
            let state = AppState::with_metrics(db, cfg, store, metrics);
            let eval_state = state.clone();
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(eval_interval));
                loop {
                    interval.tick().await;
                    if let Err(e) = raptor::domain::rollout::evaluate_rollouts(&eval_state).await {
                        tracing::error!(error = ?e, "rollout evaluation failed");
                    }
                    if let Err(e) =
                        raptor::domain::target_filter::auto_assign_all(&eval_state).await
                    {
                        tracing::error!(error = ?e, "auto-assignment sweep failed");
                    }
                    // Refresh fleet-state gauges alongside the sweep so metrics
                    // track a real snapshot without an async observable callback.
                    if eval_state.metrics.enabled()
                        && let Err(e) = observe_fleet(&eval_state).await
                    {
                        tracing::warn!(error = ?e, "fleet metric observation failed");
                    }
                }
            });
            // A separate task rather than a branch in the sweep above: cleanup
            // reclaims storage on a window measured in days, so it has no
            // business running at the evaluator's seconds-scale cadence.
            if state.cfg.cleanup.enabled {
                let cleanup_state = state.clone();
                let cleanup_interval = cleanup_state.cfg.cleanup.interval_secs.max(1);
                tokio::spawn(async move {
                    let mut interval =
                        tokio::time::interval(std::time::Duration::from_secs(cleanup_interval));
                    loop {
                        interval.tick().await;
                        match raptor::domain::cleanup::run_sweep(&cleanup_state).await {
                            Ok(0) => {}
                            Ok(n) => tracing::info!(deleted = n, "action cleanup sweep"),
                            Err(e) => tracing::error!(error = ?e, "action cleanup failed"),
                        }
                    }
                });
            }
            let app = raptor::app::build_app(state);
            let listener = tokio::net::TcpListener::bind(bind).await?;
            tracing::info!(%bind, "raptor listening");
            // with_connect_info so DDI handlers can record the device's source
            // address (see util::client_address).
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown_signal())
            .await?;
            tracing::info!("shutting down; flushing telemetry");
            telemetry.shutdown();
        }
    }
    Ok(())
}

/// Rejects a malformed `[ddi] polling_interval` at startup rather than at
/// poll time: parses the grammar, then compiles every override rule's RSQL
/// through the same path a device poll evaluates it with
/// (`api::mgmt::targets::condition`), so an unknown field name fails here
/// too, not silently the first time a device happens to match it.
fn validate_polling_schedule(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let schedule = cfg
        .ddi
        .polling_schedule()
        .map_err(|e| format!("[ddi] polling_interval: {e}"))?;
    for rule in &schedule.rules {
        raptor::api::mgmt::targets::condition(&rule.filter)
            .map_err(|e| format!("[ddi] polling_interval override {:?}: {e:?}", rule.filter))?;
    }
    Ok(())
}

/// Snapshot fleet state (targets by `update_status`, active actions) into the
/// metrics gauges.
async fn observe_fleet(state: &AppState) -> Result<(), sea_orm::DbErr> {
    use raptor::entity::{action, target};
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};

    let by_status: Vec<(String, i64)> = target::Entity::find()
        .select_only()
        .column(target::Column::UpdateStatus)
        .column_as(target::Column::Id.count(), "count")
        .group_by(target::Column::UpdateStatus)
        .into_tuple()
        .all(&state.db)
        .await?;
    let active = action::Entity::find()
        .filter(action::Column::Active.eq(true))
        .count(&state.db)
        .await? as i64;
    state.metrics.observe_fleet(&by_status, active);
    Ok(())
}

/// Resolve on SIGINT (Ctrl-C) or SIGTERM so exporters get a chance to flush.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl-C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
#[allow(clippy::result_large_err)] // figment::Jail::expect_with's closure error type is fixed by the crate
mod tests {
    use super::*;

    fn assert_polling_interval(polling_interval: &str, expect_ok: bool) {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "raptor.toml",
                &format!(
                    "database_url = \"sqlite::memory:\"\n\
                     artifact_dir = \"/tmp\"\n\
                     [ddi]\n\
                     polling_interval = {polling_interval:?}\n\
                     [mgmt]\n\
                     username = \"admin\"\n\
                     password_hash = \"x\"\n"
                ),
            )?;
            let cfg = Config::load(Some(std::path::Path::new("raptor.toml"))).unwrap();
            assert_eq!(
                validate_polling_schedule(&cfg).is_ok(),
                expect_ok,
                "polling_interval = {polling_interval:?}"
            );
            Ok(())
        });
    }

    #[test]
    fn accepts_a_well_formed_schedule() {
        assert_polling_interval("00:05:00, group==eu -> 00:01:00", true);
    }

    #[test]
    fn rejects_malformed_grammar() {
        assert_polling_interval("not-a-time", false);
    }

    #[test]
    fn rejects_a_rule_with_an_unknown_field() {
        assert_polling_interval("00:05:00, nope==1 -> 00:01:00", false);
    }

    /// hawkBit's own PR examples write RSQL with spaces around the operator;
    /// raptor's FIQL dialect doesn't accept that. Catching it here, at
    /// startup, is the whole point of this function.
    #[test]
    fn rejects_hawkbit_style_whitespace_around_the_operator() {
        assert_polling_interval("00:05:00, group == 'eu' -> 00:01:00", false);
    }
}
