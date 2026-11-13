//! CLI config: server URL + Basic auth credentials. Precedence is
//! flags > env (`RAPTOR_URL`/`RAPTOR_USER`/`RAPTOR_PASS`) > config file.
//! `RAPTOR_TOKEN` (an OIDC bearer token) replaces user and password
//! (`~/.config/raptor/cli.toml`, mode 0600).

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub url: Option<String>,
    pub user: Option<String>,
    pub pass: Option<String>,
}

pub fn config_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "raptor")
        .context("could not determine home directory")?;
    Ok(dirs.config_dir().join("cli.toml"))
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(&path, raw)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn delete() -> Result<()> {
        let path = config_path()?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Merge in flag/env overrides and validate everything needed to make a
    /// request is present.
    pub fn resolve(mut self, url: Option<String>, user: Option<String>) -> Result<Resolved> {
        if let Ok(v) = std::env::var("RAPTOR_URL") {
            self.url = Some(v);
        }
        if let Ok(v) = std::env::var("RAPTOR_USER") {
            self.user = Some(v);
        }
        if let Ok(v) = std::env::var("RAPTOR_PASS") {
            self.pass = Some(v);
        }
        if let Some(u) = url {
            self.url = Some(u);
        }
        if let Some(u) = user {
            self.user = Some(u);
        }
        let url = self
            .url
            .context("no server URL configured — run 'raptorctl login' first")?;
        if !url.starts_with("http://") && !url.starts_with("https://") {
            bail!("server URL must start with http:// or https://");
        }
        let url = url.trim_end_matches('/').to_string();
        if let Ok(token) = std::env::var("RAPTOR_TOKEN")
            && !token.is_empty()
        {
            return Ok(Resolved {
                url,
                user: self.user.unwrap_or_default(),
                pass: String::new(),
                token: Some(token),
            });
        }
        let user = self
            .user
            .context("no username configured — run 'raptorctl login' first")?;
        let pass = self
            .pass
            .context("no password configured — run 'raptorctl login' first")?;
        Ok(Resolved {
            url,
            user,
            pass,
            token: None,
        })
    }
}

/// A fully resolved config, ready to build a [`crate::client::Client`].
pub struct Resolved {
    pub url: String,
    pub user: String,
    pub pass: String,
    /// Sent as `Authorization: Bearer` instead of Basic when set.
    pub token: Option<String>,
}

impl Resolved {
    pub fn authorization(&self) -> String {
        use base64::Engine;
        match &self.token {
            Some(t) => format!("Bearer {t}"),
            None => format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD
                    .encode(format!("{}:{}", self.user, self.pass))
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_overrides_file_overrides_default() {
        let cfg = Config {
            url: Some("http://file".into()),
            user: Some("file-user".into()),
            pass: Some("secret".into()),
        };
        // SAFETY: single-threaded test process, no concurrent env reads.
        unsafe {
            std::env::remove_var("RAPTOR_URL");
            std::env::remove_var("RAPTOR_USER");
            std::env::remove_var("RAPTOR_PASS");
            std::env::remove_var("RAPTOR_TOKEN");
        }
        let r = cfg
            .resolve(Some("http://flag".into()), None)
            .expect("resolve");
        assert_eq!(r.url, "http://flag");
        assert_eq!(r.user, "file-user");
    }

    #[test]
    fn missing_url_errors() {
        let cfg = Config::default();
        unsafe {
            std::env::remove_var("RAPTOR_URL");
            std::env::remove_var("RAPTOR_USER");
            std::env::remove_var("RAPTOR_PASS");
            std::env::remove_var("RAPTOR_TOKEN");
        }
        assert!(cfg.resolve(None, None).is_err());
    }

    #[test]
    fn token_replaces_basic_credentials() {
        let cfg = Config {
            url: Some("http://file".into()),
            ..Default::default()
        };
        // SAFETY: single-threaded test process, no concurrent env reads.
        unsafe {
            std::env::remove_var("RAPTOR_URL");
            std::env::remove_var("RAPTOR_USER");
            std::env::remove_var("RAPTOR_PASS");
            std::env::set_var("RAPTOR_TOKEN", "eyJ.x.y");
        }
        let r = cfg.resolve(None, None);
        unsafe { std::env::remove_var("RAPTOR_TOKEN") };
        assert_eq!(r.expect("resolve").authorization(), "Bearer eyJ.x.y");
        let basic = Resolved {
            url: String::new(),
            user: "u".into(),
            pass: "p".into(),
            token: None,
        };
        assert_eq!(basic.authorization(), "Basic dTpw");
    }
}
