//! Core identifiers shared by every backend.

use std::fmt;
use std::str::FromStr;

use serde::Serialize;

/// The package managers `upm` knows how to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    Brew,
    Apt,
    Dnf,
    Pacman,
    Zypper,
    Winget,
    Scoop,
    Choco,
}

impl BackendKind {
    pub const ALL: [BackendKind; 8] = [
        BackendKind::Brew,
        BackendKind::Apt,
        BackendKind::Dnf,
        BackendKind::Pacman,
        BackendKind::Zypper,
        BackendKind::Winget,
        BackendKind::Scoop,
        BackendKind::Choco,
    ];

    /// Stable lowercase identifier used in config, JSON and CLI flags.
    pub fn id(self) -> &'static str {
        match self {
            BackendKind::Brew => "brew",
            BackendKind::Apt => "apt",
            BackendKind::Dnf => "dnf",
            BackendKind::Pacman => "pacman",
            BackendKind::Zypper => "zypper",
            BackendKind::Winget => "winget",
            BackendKind::Scoop => "scoop",
            BackendKind::Choco => "choco",
        }
    }

    /// The executable used to detect and drive this backend.
    pub fn binary(self) -> &'static str {
        self.id()
    }

    /// Human-friendly name.
    pub fn label(self) -> &'static str {
        match self {
            BackendKind::Brew => "Homebrew",
            BackendKind::Apt => "APT (Debian/Ubuntu)",
            BackendKind::Dnf => "DNF (Fedora/RHEL)",
            BackendKind::Pacman => "pacman (Arch)",
            BackendKind::Zypper => "zypper (openSUSE)",
            BackendKind::Winget => "winget (Windows)",
            BackendKind::Scoop => "Scoop (Windows)",
            BackendKind::Choco => "Chocolatey (Windows)",
        }
    }
}

impl FromStr for BackendKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "brew" | "homebrew" => Ok(BackendKind::Brew),
            "apt" | "apt-get" | "debian" => Ok(BackendKind::Apt),
            "dnf" | "yum" => Ok(BackendKind::Dnf),
            "pacman" | "arch" => Ok(BackendKind::Pacman),
            "zypper" | "opensuse" => Ok(BackendKind::Zypper),
            "winget" => Ok(BackendKind::Winget),
            "scoop" => Ok(BackendKind::Scoop),
            "choco" | "chocolatey" => Ok(BackendKind::Choco),
            other => Err(format!(
                "unknown backend `{other}` (expected one of: brew, apt, dnf, pacman, zypper, winget, scoop, choco)"
            )),
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases() {
        assert_eq!(
            "homebrew".parse::<BackendKind>().unwrap(),
            BackendKind::Brew
        );
        assert_eq!("apt-get".parse::<BackendKind>().unwrap(), BackendKind::Apt);
        assert_eq!("yum".parse::<BackendKind>().unwrap(), BackendKind::Dnf);
        assert_eq!(
            "chocolatey".parse::<BackendKind>().unwrap(),
            BackendKind::Choco
        );
    }

    #[test]
    fn rejects_unknown_backend() {
        assert!("opkg".parse::<BackendKind>().is_err());
    }

    #[test]
    fn ids_round_trip() {
        for kind in BackendKind::ALL {
            assert_eq!(kind.id().parse::<BackendKind>().unwrap(), kind);
        }
    }
}
