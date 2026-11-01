//! User configuration.
//!
//! Location: `$UPM_CONFIG`, else `<config_dir>/upm/config.toml`.
//! Everything is optional; an absent file behaves like the defaults.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::models::BackendKind;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub general: General,
    /// `[packages.<name>]` overrides the embedded alias registry.
    pub packages: BTreeMap<String, BTreeMap<String, String>>,
    /// `[backends.<id>] enabled = false` disables a backend.
    pub backends: BTreeMap<String, BackendConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Backend ids to try first, in order.
    pub prefer: Vec<String>,
    /// Ask for confirmation before mutating actions.
    pub confirm: bool,
    /// Force colour on/off. `None` means auto-detect.
    pub color: Option<bool>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            prefer: Vec::new(),
            confirm: true,
            color: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BackendConfig {
    pub enabled: Option<bool>,
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        if let Some(explicit) = std::env::var_os("UPM_CONFIG") {
            return Some(PathBuf::from(explicit));
        }
        dirs::config_dir().map(|dir| dir.join("upm").join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("could not read config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid config file {}", path.display()))
    }

    /// Write a commented default config if none exists. Returns the path.
    pub fn write_default() -> Result<PathBuf> {
        let path = Self::path().context("could not determine a config directory")?;
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("could not create {}", parent.display()))?;
            }
            let body = toml::to_string_pretty(&Self::default())
                .context("could not serialise default config")?;
            std::fs::write(&path, body)
                .with_context(|| format!("could not write {}", path.display()))?;
            set_private_permissions(&path);
        }
        Ok(path)
    }

    pub fn backend_enabled(&self, kind: BackendKind) -> bool {
        self.backends
            .get(kind.id())
            .and_then(|backend| backend.enabled)
            .unwrap_or(true)
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn defaults_are_sane() {
        let cfg = Config::default();
        assert!(cfg.general.confirm);
        assert!(cfg.general.prefer.is_empty());
        assert!(cfg.backend_enabled(BackendKind::Brew));
    }

    #[test]
    fn parses_partial_config() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            file,
            "[general]\nprefer = [\"brew\", \"apt\"]\nconfirm = false\n\n[backends.choco]\nenabled = false\n\n[packages.nodejs]\nbrew = \"node\"\n"
        )
        .unwrap();
        let cfg = Config::load_from(file.path()).unwrap();
        assert_eq!(cfg.general.prefer, vec!["brew", "apt"]);
        assert!(!cfg.general.confirm);
        assert!(!cfg.backend_enabled(BackendKind::Choco));
        assert_eq!(
            cfg.packages
                .get("nodejs")
                .and_then(|m| m.get("brew"))
                .map(String::as_str),
            Some("node")
        );
    }

    #[test]
    fn invalid_config_is_an_error() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "this is not toml = = =").unwrap();
        assert!(Config::load_from(file.path()).is_err());
    }
}
