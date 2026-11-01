//! OS and backend detection.

use crate::backend::Backend;
use crate::config::Config;
use crate::models::BackendKind;

pub fn os_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "freebsd") {
        "FreeBSD"
    } else {
        std::env::consts::OS
    }
}

pub fn arch() -> &'static str {
    std::env::consts::ARCH
}

/// `PRETTY_NAME` from `/etc/os-release`, when present.
pub fn distro() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    text.lines().find_map(|line| {
        line.strip_prefix("PRETTY_NAME=")
            .map(|value| value.trim_matches('"').to_string())
    })
}

pub fn container() -> Option<String> {
    if std::path::Path::new("/.dockerenv").exists() {
        return Some("Docker".to_string());
    }
    if std::path::Path::new("/run/.containerenv").exists() {
        return Some("Podman".to_string());
    }
    if let Ok(value) = std::env::var("container") {
        if !value.is_empty() {
            return Some(value);
        }
    }
    let cgroup = std::fs::read_to_string("/proc/1/cgroup").unwrap_or_default();
    let lower = cgroup.to_ascii_lowercase();
    if lower.contains("docker") {
        Some("Docker".to_string())
    } else if lower.contains("kubepods") || lower.contains("containerd") {
        Some("container".to_string())
    } else {
        None
    }
}

/// Default backend priority for the current OS.
pub fn priority() -> Vec<BackendKind> {
    if cfg!(target_os = "macos") {
        vec![BackendKind::Brew]
    } else if cfg!(target_os = "windows") {
        vec![BackendKind::Winget, BackendKind::Scoop, BackendKind::Choco]
    } else {
        // Native first, Homebrew last (it can run on Linux too).
        vec![
            BackendKind::Apt,
            BackendKind::Dnf,
            BackendKind::Pacman,
            BackendKind::Zypper,
            BackendKind::Brew,
        ]
    }
}

/// Backends available on this machine, in resolution order, honouring
/// `general.prefer` and `backends.<id>.enabled`.
pub fn detected(config: &Config) -> Vec<Backend> {
    let mut kinds = priority();

    let prefer: Vec<BackendKind> = config
        .general
        .prefer
        .iter()
        .filter_map(|value| value.parse().ok())
        .collect();
    if !prefer.is_empty() {
        kinds.sort_by_key(|kind| {
            prefer
                .iter()
                .position(|candidate| candidate == kind)
                .unwrap_or(prefer.len())
        });
    }

    kinds
        .into_iter()
        .filter(|kind| config.backend_enabled(*kind))
        .map(Backend::new)
        .filter(|backend| backend.available())
        .collect()
}

/// Every backend kind relevant to this OS, available or not (for `doctor`).
pub fn considered() -> Vec<BackendKind> {
    priority()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_has_a_priority_list() {
        assert!(!priority().is_empty());
    }

    #[test]
    fn os_name_is_known() {
        assert!(!os_name().is_empty());
        assert!(!arch().is_empty());
    }

    #[test]
    fn disabling_a_backend_removes_it() {
        let mut config = Config::default();
        for kind in BackendKind::ALL {
            config.backends.insert(
                kind.id().to_string(),
                crate::config::BackendConfig {
                    enabled: Some(false),
                },
            );
        }
        assert!(detected(&config).is_empty());
    }
}
