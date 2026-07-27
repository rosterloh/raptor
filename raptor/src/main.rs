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
            let (telemetry, metrics) = raptor::telemetry::init(cfg.otel.as_ref())?;
            if let Some(u) = &cfg.ddi.artifact_http_url {
                if !u.starts_with("http://") {
                    tracing::warn!(
                        url = %u,
                        "[ddi] artifact_http_url is not an http:// URL; it is advertised as the \
                         plain-HTTP artifact link (download-http), so devices will follow it as-is."
                    );
                }
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
                    if eval_state.metrics.enabled() {
                        if let Err(e) = observe_fleet(&eval_state).await {
                            tracing::warn!(error = ?e, "fleet metric observation failed");
                        }
                    }
                }
            });
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
