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
            use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
            let mut pw = String::new();
            std::io::stdin().read_line(&mut pw)?;
            let salt = SaltString::generate(&mut OsRng);
            match argon2::Argon2::default().hash_password(pw.trim_end().as_bytes(), &salt) {
                Ok(hash) => println!("{hash}"),
                Err(e) => return Err(format!("hash password error: {}", e).into()),
            }
        }
        Cmd::Serve { config } => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "raptor=info,tower_http=info".into()),
                )
                .init();
            let cfg = Config::load(Some(&config))?;
            let db = sea_orm::Database::connect(&cfg.database_url).await?;
            migration::Migrator::up(&db, None).await?;
            let store = raptor::storage::ArtifactStore::new(cfg.artifact_dir.clone())?;
            let bind = cfg.bind;
            let eval_interval = cfg.rollout_eval_interval_secs.max(1);
            let state = AppState::new(db, cfg, store);
            let eval_state = state.clone();
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(eval_interval));
                loop {
                    interval.tick().await;
                    if let Err(e) = raptor::domain::rollout::evaluate_rollouts(&eval_state).await {
                        tracing::error!(error = ?e, "rollout evaluation failed");
                    }
                }
            });
            let app = raptor::app::build_app(state);
            let listener = tokio::net::TcpListener::bind(bind).await?;
            tracing::info!(%bind, "raptor listening");
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}
