//! Package alias registry.
//!
//! Package names differ across managers. The embedded table maps a canonical
//! name (`nodejs`, `code`, `ripgrep`) to the native name per backend. Users can
//! extend or override it through `[packages.*]` in their config file.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::config::Config;
use crate::models::BackendKind;

const EMBEDDED: &str = include_str!("aliases.toml");

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AliasTable {
    #[serde(default)]
    pub packages: BTreeMap<String, BTreeMap<String, String>>,
}

impl AliasTable {
    /// Parse the embedded table, then layer user overrides on top.
    pub fn load(config: &Config) -> Result<Self> {
        let mut table: AliasTable =
            toml::from_str(EMBEDDED).context("embedded alias registry is invalid")?;
        for (name, mapping) in &config.packages {
            let entry = table.packages.entry(name.clone()).or_default();
            for (backend, package) in mapping {
                entry.insert(backend.clone(), package.clone());
            }
        }
        Ok(table)
    }

    pub fn lookup(&self, name: &str, backend: BackendKind) -> Option<&str> {
        self.packages
            .get(name)
            .and_then(|mapping| mapping.get(backend.id()))
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    pub fn canonical(&self, backend: BackendKind, package: &str) -> Option<&str> {
        let package = package.trim();
        self.packages.iter().find_map(|(name, mapping)| {
            mapping
                .get(backend.id())
                .filter(|native| native.eq_ignore_ascii_case(package))
                .map(|_| name.as_str())
        })
    }

    pub fn len(&self) -> usize {
        self.packages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_parses() {
        let table = AliasTable::load(&Config::default()).unwrap();
        assert!(table.len() > 20);
        assert_eq!(table.lookup("nodejs", BackendKind::Brew), Some("node"));
        assert_eq!(table.lookup("nodejs", BackendKind::Apt), Some("nodejs"));
        assert_eq!(
            table.lookup("code", BackendKind::Brew),
            Some("cask:visual-studio-code")
        );
    }

    #[test]
    fn user_overrides_win() {
        let mut config = Config::default();
        let mut mapping = BTreeMap::new();
        mapping.insert("apt".to_string(), "custom-node".to_string());
        config.packages.insert("nodejs".to_string(), mapping);

        let table = AliasTable::load(&config).unwrap();
        assert_eq!(
            table.lookup("nodejs", BackendKind::Apt),
            Some("custom-node")
        );
        assert_eq!(table.lookup("nodejs", BackendKind::Brew), Some("node"));
    }

    #[test]
    fn unknown_package_has_no_lookup() {
        let table = AliasTable::load(&Config::default()).unwrap();
        assert_eq!(
            table.lookup("definitely-not-a-real-package-xyz", BackendKind::Brew),
            None
        );
    }
}
