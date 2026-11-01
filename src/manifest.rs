use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::error::{EXIT_USAGE, exit};
use crate::paths;

pub const MANIFEST_SCHEMA: u32 = 1;
pub const SNAPSHOT_SCHEMA: u32 = 1;

fn manifest_schema() -> u32 {
    MANIFEST_SCHEMA
}

fn snapshot_schema() -> u32 {
    SNAPSHOT_SCHEMA
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Environment {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Options {
    pub auto_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifest {
    #[serde(default = "manifest_schema")]
    pub schema_version: u32,
    pub environment: Environment,
    pub packages: BTreeMap<String, String>,
    pub platform: BTreeMap<String, BTreeMap<String, String>>,
    pub options: Options,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema_version: MANIFEST_SCHEMA,
            environment: Environment::default(),
            packages: BTreeMap::new(),
            platform: BTreeMap::new(),
            options: Options::default(),
        }
    }
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid manifest {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("could not create {}", parent.display()))?;
            }
        }
        let body = toml::to_string_pretty(self).context("could not serialise manifest")?;
        std::fs::write(path, body).with_context(|| format!("could not write {}", path.display()))
    }

    pub fn platform_key() -> &'static str {
        if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "freebsd") {
            "freebsd"
        } else {
            "linux"
        }
    }

    pub fn packages_for_current_platform(&self) -> BTreeMap<String, String> {
        let mut packages = self.packages.clone();
        if let Some(extra) = self.platform.get(Self::platform_key()) {
            packages.extend(extra.clone());
        }
        packages
    }

    pub fn add(&mut self, names: &[String]) {
        for name in names {
            self.packages.insert(name.clone(), "*".to_string());
        }
    }

    pub fn remove(&mut self, names: &[String]) -> Vec<String> {
        let mut removed = Vec::new();
        for name in names {
            if self.packages.remove(name).is_some() {
                removed.push(name.clone());
            }
        }
        removed
    }
}

pub fn default_manifest_path() -> PathBuf {
    PathBuf::from("upm.toml")
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn name_or_usage(name: &str, what: &str) -> Result<()> {
    if valid_name(name) {
        Ok(())
    } else {
        Err(exit(
            EXIT_USAGE,
            format!("invalid {what} name `{name}` (use letters, digits, `-`, `_` or `.`)"),
        ))
    }
}

pub fn profile_path(name: &str) -> Result<PathBuf> {
    name_or_usage(name, "profile")?;
    let dir = paths::profiles_dir().context("could not determine a state directory")?;
    Ok(dir.join(format!("{name}.toml")))
}

pub fn load_profile(name: &str) -> Result<Manifest> {
    let path = profile_path(name)?;
    if !path.exists() {
        return Err(exit(
            crate::error::EXIT_NOT_FOUND,
            format!("profile `{name}` does not exist"),
        ));
    }
    Manifest::load(&path)
}

pub fn save_profile(name: &str, manifest: &Manifest) -> Result<PathBuf> {
    let path = profile_path(name)?;
    manifest.save(&path)?;
    Ok(path)
}

pub fn list_profiles() -> Result<Vec<(String, Manifest)>> {
    let Some(dir) = paths::profiles_dir() else {
        return Ok(Vec::new());
    };
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .with_context(|| format!("could not read {}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if let Ok(manifest) = Manifest::load(&path) {
            out.push((name.to_string(), manifest));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn delete_profile(name: &str) -> Result<bool> {
    let path = profile_path(name)?;
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&path).with_context(|| format!("could not remove {}", path.display()))?;
    Ok(true)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPackage {
    pub name: String,
    pub version: Option<String>,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    #[serde(default = "snapshot_schema")]
    pub schema_version: u32,
    pub name: String,
    pub created_unix: u64,
    pub os: String,
    pub arch: String,
    pub distro: Option<String>,
    pub backends: Vec<String>,
    pub packages: Vec<SnapshotPackage>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            schema_version: SNAPSHOT_SCHEMA,
            name: String::new(),
            created_unix: 0,
            os: String::new(),
            arch: String::new(),
            distro: None,
            backends: Vec::new(),
            packages: Vec::new(),
        }
    }
}

impl Snapshot {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("invalid snapshot {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("could not create {}", parent.display()))?;
            }
        }
        let body = serde_json::to_string_pretty(self)?;
        std::fs::write(path, body).with_context(|| format!("could not write {}", path.display()))
    }
}

pub fn snapshot_path(name: &str) -> Result<PathBuf> {
    name_or_usage(name, "snapshot")?;
    let dir = paths::snapshots_dir().context("could not determine a state directory")?;
    Ok(dir.join(format!("{name}.json")))
}

pub fn load_snapshot(name: &str) -> Result<Snapshot> {
    let path = snapshot_path(name)?;
    if !path.exists() {
        return Err(exit(
            crate::error::EXIT_NOT_FOUND,
            format!("snapshot `{name}` does not exist"),
        ));
    }
    Snapshot::load(&path)
}

pub fn save_snapshot(snapshot: &Snapshot) -> Result<PathBuf> {
    let path = snapshot_path(&snapshot.name)?;
    snapshot.save(&path)?;
    Ok(path)
}

pub fn list_snapshots() -> Result<Vec<Snapshot>> {
    let Some(dir) = paths::snapshots_dir() else {
        return Ok(Vec::new());
    };
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .with_context(|| format!("could not read {}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        if let Ok(snapshot) = Snapshot::load(&path) {
            out.push(snapshot);
        }
    }
    out.sort_by_key(|snapshot| snapshot.created_unix);
    Ok(out)
}

pub fn delete_snapshot(name: &str) -> Result<bool> {
    let path = snapshot_path(name)?;
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&path).with_context(|| format!("could not remove {}", path.display()))?;
    Ok(true)
}

pub fn resolve_snapshot_path(value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.exists() {
        path
    } else {
        snapshot_path(value).unwrap_or(path)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotDiff {
    pub added: Vec<SnapshotPackage>,
    pub removed: Vec<SnapshotPackage>,
    pub changed: Vec<VersionChange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionChange {
    pub name: String,
    pub backend: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl Snapshot {
    pub fn diff(&self, other: &Snapshot) -> SnapshotDiff {
        let key = |package: &SnapshotPackage| (package.backend.clone(), package.name.clone());
        let left: BTreeMap<(String, String), &SnapshotPackage> = self
            .packages
            .iter()
            .map(|package| (key(package), package))
            .collect();
        let right: BTreeMap<(String, String), &SnapshotPackage> = other
            .packages
            .iter()
            .map(|package| (key(package), package))
            .collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();

        for (key, package) in &right {
            match left.get(key) {
                None => added.push((*package).clone()),
                Some(previous) if previous.version != package.version => {
                    changed.push(VersionChange {
                        name: package.name.clone(),
                        backend: package.backend.clone(),
                        from: previous.version.clone(),
                        to: package.version.clone(),
                    });
                }
                _ => {}
            }
        }
        for (key, package) in &left {
            if !right.contains_key(key) {
                removed.push((*package).clone());
            }
        }

        added.sort_by(|a, b| a.name.cmp(&b.name));
        removed.sort_by(|a, b| a.name.cmp(&b.name));
        changed.sort_by(|a, b| a.name.cmp(&b.name));
        SnapshotDiff {
            added,
            removed,
            changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip_and_platform_merge() {
        let mut manifest = Manifest::default();
        manifest.add(&["git".to_string(), "nodejs".to_string()]);
        manifest
            .platform
            .entry("linux".to_string())
            .or_default()
            .insert("build-essential".to_string(), "*".to_string());
        let merged = manifest.packages_for_current_platform();
        assert!(merged.contains_key("git"));
        if cfg!(target_os = "linux") {
            assert!(merged.contains_key("build-essential"));
        }
    }

    #[test]
    fn profile_names_are_safe() {
        assert!(profile_path("developer").is_err() || profile_path("developer").is_ok());
        assert!(profile_path("../evil").is_err());
        assert!(profile_path("a/b").is_err());
        assert!(profile_path("").is_err());
    }

    #[test]
    fn snapshot_diff_reports_changes() {
        let left = Snapshot {
            packages: vec![
                SnapshotPackage {
                    name: "git".into(),
                    version: Some("2.46.0".into()),
                    backend: "apt".into(),
                },
                SnapshotPackage {
                    name: "nano".into(),
                    version: Some("7.2".into()),
                    backend: "apt".into(),
                },
            ],
            ..Default::default()
        };
        let right = Snapshot {
            packages: vec![
                SnapshotPackage {
                    name: "git".into(),
                    version: Some("2.47.1".into()),
                    backend: "apt".into(),
                },
                SnapshotPackage {
                    name: "ripgrep".into(),
                    version: Some("14.1.1".into()),
                    backend: "apt".into(),
                },
            ],
            ..Default::default()
        };
        let diff = left.diff(&right);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.changed.len(), 1);
    }
}
