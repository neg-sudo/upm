use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;

pub const SCHEMA_VERSION: u32 = 1;

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryPackage {
    pub requested: String,
    pub backend: String,
    pub package: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    pub id: u64,
    pub timestamp_unix: u64,
    pub action: String,
    pub dry_run: bool,
    pub status: String,
    #[serde(default)]
    pub packages: Vec<HistoryPackage>,
    #[serde(default)]
    pub commands: Vec<String>,
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn append(
    action: &str,
    dry_run: bool,
    status: &str,
    packages: Vec<HistoryPackage>,
    commands: Vec<String>,
) -> Result<Entry> {
    let dir = paths::state_dir().context("could not determine a state directory")?;
    append_in(&dir, action, dry_run, status, packages, commands)
}

pub fn append_in(
    dir: &Path,
    action: &str,
    dry_run: bool,
    status: &str,
    packages: Vec<HistoryPackage>,
    commands: Vec<String>,
) -> Result<Entry> {
    std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
    let path = dir.join("history.jsonl");
    let entries = read_file(&path).unwrap_or_default();
    let id = entries.iter().map(|entry| entry.id).max().unwrap_or(0) + 1;
    let entry = Entry {
        schema_version: SCHEMA_VERSION,
        id,
        timestamp_unix: now_unix(),
        action: action.to_string(),
        dry_run,
        status: status.to_string(),
        packages,
        commands,
    };
    let line = serde_json::to_string(&entry)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("could not open {}", path.display()))?;
    writeln!(file, "{line}")?;
    set_private_permissions(&path);
    Ok(entry)
}

pub fn read_all() -> Result<Vec<Entry>> {
    let Some(path) = paths::history_path() else {
        return Ok(Vec::new());
    };
    read_file(&path)
}

#[cfg(test)]
pub fn read_from(dir: &Path) -> Result<Vec<Entry>> {
    read_file(&dir.join("history.jsonl"))
}

fn read_file(path: &Path) -> Result<Vec<Entry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read {}", path.display()))?;
    let mut entries: Vec<Entry> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    entries.sort_by_key(|entry| entry.id);
    Ok(entries)
}

pub fn find(id: u64) -> Result<Option<Entry>> {
    Ok(read_all()?.into_iter().find(|entry| entry.id == id))
}

pub fn last_reversible() -> Result<Option<Entry>> {
    let entries = read_all()?;
    Ok(entries.into_iter().rev().find(|entry| {
        !entry.dry_run
            && entry.status != "failed"
            && matches!(entry.action.as_str(), "install" | "remove")
    }))
}

pub fn export(path: &Path) -> Result<usize> {
    let entries = read_all()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    let mut body = String::new();
    for entry in &entries {
        body.push_str(&serde_json::to_string(entry)?);
        body.push('\n');
    }
    std::fs::write(path, body).with_context(|| format!("could not write {}", path.display()))?;
    Ok(entries.len())
}

pub fn history_path() -> Option<PathBuf> {
    paths::history_path()
}

pub fn age(timestamp_unix: u64) -> String {
    let now = now_unix();
    let seconds = now.saturating_sub(timestamp_unix);
    if seconds < 60 {
        "just now".to_string()
    } else if seconds < 3_600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h ago", seconds / 3_600)
    } else {
        format!("{}d ago", seconds / 86_400)
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

    fn package(name: &str) -> HistoryPackage {
        HistoryPackage {
            requested: name.into(),
            backend: "apt".into(),
            package: name.into(),
        }
    }

    #[test]
    fn appends_and_reads_entries() {
        let dir = tempfile::tempdir().unwrap();
        append_in(
            dir.path(),
            "install",
            false,
            "success",
            vec![package("git")],
            vec!["apt-get install git".into()],
        )
        .unwrap();
        let entry = append_in(
            dir.path(),
            "remove",
            false,
            "success",
            vec![package("git")],
            Vec::new(),
        )
        .unwrap();
        assert_eq!(entry.id, 2);
        let entries = read_from(dir.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].action, "install");
    }

    #[test]
    fn age_is_human_readable() {
        assert_eq!(age(now_unix()), "just now");
        assert!(age(now_unix().saturating_sub(120)).contains('m'));
    }
}
