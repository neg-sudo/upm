//! Backend adapters.
//!
//! Every package manager is driven through the same small interface. Commands
//! are returned as argv vectors and executed by [`crate::exec::Runner`].

use std::path::PathBuf;

use anyhow::Result;

use crate::exec::Runner;
use crate::models::BackendKind;

#[derive(Debug, Clone, Copy)]
pub struct Backend {
    kind: BackendKind,
}

impl Backend {
    pub fn new(kind: BackendKind) -> Self {
        Self { kind }
    }

    pub fn kind(self) -> BackendKind {
        self.kind
    }

    pub fn id(self) -> &'static str {
        self.kind.id()
    }

    pub fn label(self) -> &'static str {
        self.kind.label()
    }

    pub fn binary(self) -> &'static str {
        self.kind.binary()
    }

    pub fn available(self) -> bool {
        which::which(self.binary()).is_ok()
    }

    pub fn path(self) -> Option<PathBuf> {
        which::which(self.binary()).ok()
    }

    /// Whether upm should prepend `sudo` on Unix.
    pub fn needs_elevation(self) -> bool {
        matches!(
            self.kind,
            BackendKind::Apt
                | BackendKind::Dnf
                | BackendKind::Pacman
                | BackendKind::Zypper
                | BackendKind::Choco
        )
    }

    pub fn install_cmd(self, package: &str, yes: bool) -> Vec<String> {
        let (cask, name) = split_cask(package);
        match self.kind {
            BackendKind::Brew => {
                let mut argv = strings(&["brew", "install"]);
                if cask {
                    argv.push("--cask".into());
                }
                argv.push(name.into());
                argv
            }
            BackendKind::Apt => simple("apt-get", "install", name, yes, "-y"),
            BackendKind::Dnf => simple("dnf", "install", name, yes, "-y"),
            BackendKind::Pacman => {
                let mut argv = strings(&["pacman", "-S", "--needed"]);
                if yes {
                    argv.push("--noconfirm".into());
                }
                argv.push(name.into());
                argv
            }
            BackendKind::Zypper => zypper_cmd("install", name, yes),
            BackendKind::Winget => winget_action("install", name, yes),
            BackendKind::Scoop => strings(&["scoop", "install", name]),
            BackendKind::Choco => choco_action("install", name, yes),
        }
    }

    pub fn remove_cmd(self, package: &str, yes: bool) -> Vec<String> {
        let (cask, name) = split_cask(package);
        match self.kind {
            BackendKind::Brew => {
                let mut argv = strings(&["brew", "uninstall"]);
                if cask {
                    argv.push("--cask".into());
                }
                argv.push(name.into());
                argv
            }
            BackendKind::Apt => simple("apt-get", "remove", name, yes, "-y"),
            BackendKind::Dnf => simple("dnf", "remove", name, yes, "-y"),
            BackendKind::Pacman => {
                let mut argv = strings(&["pacman", "-R"]);
                if yes {
                    argv.push("--noconfirm".into());
                }
                argv.push(name.into());
                argv
            }
            BackendKind::Zypper => zypper_cmd("remove", name, yes),
            BackendKind::Winget => winget_action("uninstall", name, yes),
            BackendKind::Scoop => strings(&["scoop", "uninstall", name]),
            BackendKind::Choco => choco_action("uninstall", name, yes),
        }
    }

    pub fn upgrade_cmd(self, package: &str, yes: bool) -> Vec<String> {
        let (cask, name) = split_cask(package);
        match self.kind {
            BackendKind::Brew => {
                let mut argv = strings(&["brew", "upgrade"]);
                if cask {
                    argv.push("--cask".into());
                }
                argv.push(name.into());
                argv
            }
            BackendKind::Apt => {
                let mut argv = strings(&["apt-get", "install", "--only-upgrade"]);
                if yes {
                    argv.push("-y".into());
                }
                argv.push(name.into());
                argv
            }
            BackendKind::Dnf => simple("dnf", "upgrade", name, yes, "-y"),
            BackendKind::Pacman => {
                let mut argv = strings(&["pacman", "-S"]);
                if yes {
                    argv.push("--noconfirm".into());
                }
                argv.push(name.into());
                argv
            }
            BackendKind::Zypper => zypper_cmd("update", name, yes),
            BackendKind::Winget => winget_action("upgrade", name, yes),
            BackendKind::Scoop => strings(&["scoop", "update", name]),
            BackendKind::Choco => choco_action("upgrade", name, yes),
        }
    }

    /// Upgrade every installed package for this backend.
    pub fn upgrade_all_cmd(self, yes: bool) -> Vec<String> {
        match self.kind {
            BackendKind::Brew => strings(&["brew", "upgrade"]),
            BackendKind::Apt => simple("apt-get", "upgrade", "", yes, "-y")
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect(),
            BackendKind::Dnf => simple("dnf", "upgrade", "", yes, "-y")
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect(),
            BackendKind::Pacman => {
                let mut argv = strings(&["pacman", "-Syu"]);
                if yes {
                    argv.push("--noconfirm".into());
                }
                argv
            }
            BackendKind::Zypper => {
                let mut argv = strings(&["zypper"]);
                if yes {
                    argv.push("--non-interactive".into());
                }
                argv.push("update".into());
                argv
            }
            BackendKind::Winget => winget_upgrade_all(yes),
            BackendKind::Scoop => strings(&["scoop", "update", "*"]),
            BackendKind::Choco => choco_action("upgrade", "all", yes),
        }
    }

    /// Refresh package metadata. `None` means the backend has no such step.
    pub fn update_cmd(self) -> Option<Vec<String>> {
        match self.kind {
            BackendKind::Brew => Some(strings(&["brew", "update"])),
            BackendKind::Apt => Some(strings(&["apt-get", "update"])),
            BackendKind::Dnf => Some(strings(&["dnf", "makecache"])),
            BackendKind::Pacman => Some(strings(&["pacman", "-Sy"])),
            BackendKind::Zypper => Some(strings(&["zypper", "refresh"])),
            BackendKind::Winget => Some(strings(&["winget", "source", "update"])),
            BackendKind::Scoop => Some(strings(&["scoop", "update"])),
            BackendKind::Choco => None,
        }
    }

    pub fn search_cmd(self, term: &str) -> Vec<String> {
        match self.kind {
            BackendKind::Brew => strings(&["brew", "search", term]),
            BackendKind::Apt => strings(&["apt-cache", "search", term]),
            BackendKind::Dnf => strings(&["dnf", "search", term]),
            BackendKind::Pacman => strings(&["pacman", "-Ss", term]),
            BackendKind::Zypper => strings(&["zypper", "search", term]),
            BackendKind::Winget => {
                let mut argv = strings(&["winget", "search", term]);
                argv.push("--accept-source-agreements".into());
                argv
            }
            BackendKind::Scoop => strings(&["scoop", "search", term]),
            BackendKind::Choco => strings(&["choco", "search", term]),
        }
    }

    pub fn list_cmd(self) -> Vec<String> {
        match self.kind {
            BackendKind::Brew => strings(&["brew", "list"]),
            BackendKind::Apt => strings(&["apt", "list", "--installed"]),
            BackendKind::Dnf => strings(&["dnf", "list", "--installed"]),
            BackendKind::Pacman => strings(&["pacman", "-Q"]),
            BackendKind::Zypper => strings(&["zypper", "search", "--installed-only"]),
            BackendKind::Winget => {
                let mut argv = strings(&["winget", "list"]);
                argv.push("--accept-source-agreements".into());
                argv
            }
            BackendKind::Scoop => strings(&["scoop", "list"]),
            BackendKind::Choco => strings(&["choco", "list"]),
        }
    }

    pub fn info_cmd(self, package: &str) -> Option<Vec<String>> {
        let (cask, name) = split_cask(package);
        let mut argv = match self.kind {
            BackendKind::Brew => strings(&["brew", "info"]),
            BackendKind::Apt => strings(&["apt-cache", "show"]),
            BackendKind::Dnf => strings(&["dnf", "info"]),
            BackendKind::Pacman => strings(&["pacman", "-Si"]),
            BackendKind::Zypper => strings(&["zypper", "info"]),
            BackendKind::Winget => strings(&["winget", "show", "--id"]),
            BackendKind::Scoop => strings(&["scoop", "info"]),
            BackendKind::Choco => strings(&["choco", "info"]),
        };
        if self.kind == BackendKind::Winget {
            argv.push(name.into());
            argv.push("--exact".into());
            argv.push("--accept-source-agreements".into());
        } else {
            if self.kind == BackendKind::Brew && cask {
                argv.push("--cask".into());
            }
            argv.push(name.into());
        }
        Some(argv)
    }

    /// Command whose success (and output shape) indicates an installed package.
    pub fn probe_cmd(self, package: &str) -> Option<Vec<String>> {
        let (cask, name) = split_cask(package);
        let argv = match self.kind {
            BackendKind::Brew => {
                let mut argv = strings(&["brew", "list"]);
                argv.push(if cask { "--cask" } else { "--formula" }.into());
                argv.push(name.into());
                argv
            }
            BackendKind::Apt => strings(&["dpkg-query", "-W", "-f=${db:Status-Abbrev}", name]),
            BackendKind::Dnf | BackendKind::Zypper => strings(&["rpm", "-q", name]),
            BackendKind::Pacman => strings(&["pacman", "-Q", name]),
            BackendKind::Winget => {
                let mut argv = strings(&["winget", "list", "--id", name, "--exact"]);
                argv.push("--accept-source-agreements".into());
                argv.push("--disable-interactivity".into());
                argv
            }
            BackendKind::Scoop => strings(&["scoop", "list"]),
            BackendKind::Choco => strings(&["choco", "list"]),
        };
        Some(argv)
    }

    pub fn interpret_installed(self, package: &str, success: bool, stdout: &str) -> Option<bool> {
        let (_, name) = split_cask(package);
        match self.kind {
            BackendKind::Apt => Some(success && stdout.trim_start().starts_with("ii")),
            BackendKind::Winget | BackendKind::Scoop | BackendKind::Choco => Some(
                success
                    && stdout
                        .to_ascii_lowercase()
                        .contains(&name.to_ascii_lowercase()),
            ),
            _ => Some(success),
        }
    }

    /// Heuristic availability check used only when the alias registry has no
    /// entry for a package.
    pub fn has_package(self, name: &str, runner: &Runner) -> Result<bool> {
        let argv = self.search_cmd(name);
        let out = runner.capture(&argv)?;
        if !out.status.success() {
            return Ok(false);
        }
        let haystack = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
        Ok(haystack.contains(&name.to_ascii_lowercase()))
    }
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_string()).collect()
}

fn simple(binary: &str, action: &str, package: &str, yes: bool, yes_flag: &str) -> Vec<String> {
    let mut argv = strings(&[binary, action]);
    if yes {
        argv.push(yes_flag.into());
    }
    if !package.is_empty() {
        argv.push(package.into());
    }
    argv
}

fn zypper_cmd(action: &str, package: &str, yes: bool) -> Vec<String> {
    let mut argv = strings(&["zypper"]);
    if yes {
        argv.push("--non-interactive".into());
    }
    argv.push(action.into());
    argv.push(package.into());
    argv
}

fn winget_action(action: &str, package: &str, yes: bool) -> Vec<String> {
    let mut argv = strings(&["winget", action, "--id", package, "--exact"]);
    if action != "uninstall" {
        argv.push("--accept-package-agreements".into());
    }
    argv.push("--accept-source-agreements".into());
    if yes {
        argv.push("--disable-interactivity".into());
    }
    argv
}

fn winget_upgrade_all(yes: bool) -> Vec<String> {
    let mut argv = strings(&[
        "winget",
        "upgrade",
        "--all",
        "--accept-package-agreements",
        "--accept-source-agreements",
    ]);
    if yes {
        argv.push("--disable-interactivity".into());
    }
    argv
}

fn choco_action(action: &str, package: &str, yes: bool) -> Vec<String> {
    let mut argv = strings(&["choco", action, package, "--no-progress"]);
    if yes {
        argv.push("-y".into());
    }
    argv
}

/// Brew casks are addressed as `cask:<name>` in the alias registry.
fn split_cask(package: &str) -> (bool, &str) {
    match package.strip_prefix("cask:") {
        Some(rest) => (true, rest),
        None => (false, package),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apt_install_gets_noninteractive_flag() {
        let b = Backend::new(BackendKind::Apt);
        assert_eq!(
            b.install_cmd("git", true),
            strings(&["apt-get", "install", "-y", "git"])
        );
    }

    #[test]
    fn pacman_needs_and_noconfirm() {
        let b = Backend::new(BackendKind::Pacman);
        assert_eq!(
            b.install_cmd("git", true),
            strings(&["pacman", "-S", "--needed", "--noconfirm", "git"])
        );
    }

    #[test]
    fn brew_cask_prefix_is_translated() {
        let b = Backend::new(BackendKind::Brew);
        assert_eq!(
            b.install_cmd("cask:firefox", false),
            strings(&["brew", "install", "--cask", "firefox"])
        );
        assert_eq!(
            b.probe_cmd("cask:firefox").unwrap(),
            strings(&["brew", "list", "--cask", "firefox"])
        );
    }

    #[test]
    fn winget_install_is_exact_and_headless() {
        let b = Backend::new(BackendKind::Winget);
        let argv = b.install_cmd("Git.Git", true);
        assert!(argv.contains(&"--exact".to_string()));
        assert!(argv.contains(&"Git.Git".to_string()));
        assert!(argv.contains(&"--disable-interactivity".to_string()));
    }

    #[test]
    fn zypper_puts_global_flag_before_action() {
        let b = Backend::new(BackendKind::Zypper);
        assert_eq!(
            b.install_cmd("git", true),
            strings(&["zypper", "--non-interactive", "install", "git"])
        );
    }

    #[test]
    fn only_system_backends_need_elevation() {
        assert!(Backend::new(BackendKind::Apt).needs_elevation());
        assert!(Backend::new(BackendKind::Choco).needs_elevation());
        assert!(!Backend::new(BackendKind::Brew).needs_elevation());
        assert!(!Backend::new(BackendKind::Winget).needs_elevation());
        assert!(!Backend::new(BackendKind::Scoop).needs_elevation());
    }

    #[test]
    fn apt_probe_reads_dpkg_status() {
        let b = Backend::new(BackendKind::Apt);
        assert_eq!(b.interpret_installed("git", true, "ii "), Some(true));
        assert_eq!(b.interpret_installed("git", true, "rc "), Some(false));
    }
}
