use serde::Serialize;

use crate::models::BackendKind;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Capabilities {
    pub install: bool,
    pub remove: bool,
    pub upgrade: bool,
    pub upgrade_all: bool,
    pub update: bool,
    pub search: bool,
    pub info: bool,
    pub list: bool,
    pub outdated: bool,
    pub clean: bool,
    pub pin: bool,
    pub pins: bool,
    pub why: bool,
    pub depends: bool,
    pub repos: bool,
    pub rollback: bool,
    pub security_updates: bool,
    pub interactive: bool,
}

impl Capabilities {
    pub fn of(kind: BackendKind) -> Self {
        let mut caps = Self {
            install: true,
            remove: true,
            upgrade: true,
            upgrade_all: true,
            update: true,
            search: true,
            info: true,
            list: true,
            outdated: true,
            clean: true,
            pin: true,
            pins: true,
            why: true,
            depends: true,
            repos: true,
            rollback: true,
            security_updates: false,
            interactive: true,
        };

        match kind {
            BackendKind::Brew | BackendKind::Apt => {}
            BackendKind::Dnf | BackendKind::Zypper => caps.security_updates = true,
            BackendKind::Pacman => {
                caps.pin = false;
                caps.pins = false;
            }
            BackendKind::Winget => {
                caps.clean = false;
                caps.why = false;
                caps.depends = false;
            }
            BackendKind::Scoop => {
                caps.pins = false;
                caps.why = false;
                caps.depends = false;
            }
            BackendKind::Choco => {
                caps.update = false;
                caps.clean = false;
                caps.why = false;
                caps.depends = false;
            }
        }

        caps
    }

    pub fn entries(&self) -> [(&'static str, bool); 18] {
        [
            ("install", self.install),
            ("remove", self.remove),
            ("upgrade", self.upgrade),
            ("upgrade_all", self.upgrade_all),
            ("update", self.update),
            ("search", self.search),
            ("info", self.info),
            ("list", self.list),
            ("outdated", self.outdated),
            ("clean", self.clean),
            ("pin", self.pin),
            ("pins", self.pins),
            ("why", self.why),
            ("depends", self.depends),
            ("repos", self.repos),
            ("rollback", self.rollback),
            ("security_updates", self.security_updates),
            ("interactive", self.interactive),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_backends_can_install_and_search() {
        for kind in BackendKind::ALL {
            let caps = Capabilities::of(kind);
            assert!(caps.install);
            assert!(caps.search);
        }
    }

    #[test]
    fn unsupported_features_are_reported() {
        assert!(!Capabilities::of(BackendKind::Winget).clean);
        assert!(!Capabilities::of(BackendKind::Pacman).pin);
        assert!(Capabilities::of(BackendKind::Brew).clean);
        assert!(Capabilities::of(BackendKind::Dnf).security_updates);
        assert!(!Capabilities::of(BackendKind::Apt).security_updates);
    }
}
