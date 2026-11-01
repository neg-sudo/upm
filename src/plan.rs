use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPackage {
    pub requested: String,
    pub backend: String,
    pub package: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    pub action: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub packages: Vec<PlanPackage>,
    #[serde(default)]
    pub commands: Vec<String>,
}

impl Plan {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("invalid plan {}", path.display()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plan.json");
        let plan = Plan {
            schema_version: SCHEMA_VERSION,
            action: "install".into(),
            dry_run: true,
            packages: vec![PlanPackage {
                requested: "git".into(),
                backend: "apt".into(),
                package: "git".into(),
            }],
            commands: vec!["apt-get install git".into()],
        };
        plan.save(&path).unwrap();
        let loaded = Plan::load(&path).unwrap();
        assert_eq!(loaded.action, "install");
        assert_eq!(loaded.packages.len(), 1);
    }
}
