use crate::config::Config;
use crate::metrics::Metrics;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub db: DatabaseConnection,
    pub cfg: Config,
    pub store: crate::storage::ArtifactStore,
    pub sessions: crate::auth::session::SessionStore,
    pub metrics: Metrics,
}

impl AppState {
    /// Construct state with metrics disabled (the default for tests and any
    /// build without OTLP export configured).
    pub fn new(db: DatabaseConnection, cfg: Config, store: crate::storage::ArtifactStore) -> Self {
        Self::with_metrics(db, cfg, store, Metrics::disabled())
    }

    /// Construct state with a live metrics handle (wired up in `main` once the
    /// OTLP meter exists).
    pub fn with_metrics(
        db: DatabaseConnection,
        cfg: Config,
        store: crate::storage::ArtifactStore,
        metrics: Metrics,
    ) -> Self {
        Self(Arc::new(Inner {
            db,
            cfg,
            store,
            sessions: Default::default(),
            metrics,
        }))
    }
}

impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}
