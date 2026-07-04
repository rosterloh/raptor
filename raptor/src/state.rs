use crate::config::Config;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub db: DatabaseConnection,
    pub cfg: Config,
}

impl AppState {
    pub fn new(db: DatabaseConnection, cfg: Config) -> Self {
        Self(Arc::new(Inner { db, cfg }))
    }
}

impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner { &self.0 }
}
