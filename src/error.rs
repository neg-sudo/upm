//! Categorized errors and stable exit codes.
//!
//! Exit codes are part of the public contract so scripts can branch on them.

use std::fmt;

pub const EXIT_ERROR: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_PRIVILEGE: i32 = 3;
pub const EXIT_NOT_FOUND: i32 = 4;
pub const EXIT_NO_BACKEND: i32 = 5;
pub const EXIT_UNSUPPORTED: i32 = 6;

/// An error carrying a specific process exit code.
#[derive(Debug)]
pub struct Exit {
    pub code: i32,
    pub message: String,
}

impl fmt::Display for Exit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Exit {}

/// Build an [`anyhow::Error`] that will map to `code` at the process boundary.
pub fn exit(code: i32, message: impl Into<String>) -> anyhow::Error {
    Exit {
        code,
        message: message.into(),
    }
    .into()
}

/// Extract the exit code from an error chain, defaulting to [`EXIT_ERROR`].
pub fn code_of(err: &anyhow::Error) -> i32 {
    err.downcast_ref::<Exit>()
        .map(|e| e.code)
        .unwrap_or(EXIT_ERROR)
}

/// A short, actionable hint for common backend failures.
pub fn hint_of(err: &anyhow::Error) -> Option<&'static str> {
    let text = format!("{err:#}").to_ascii_lowercase();
    let checks: [(&str, &str); 9] = [
        (
            "permission denied",
            "run with elevated privileges (sudo) or check the backend's permissions",
        ),
        (
            "needs root",
            "run with elevated privileges (sudo) or check the backend's permissions",
        ),
        (
            "could not get lock",
            "another package manager is running; wait for it to finish or remove the stale lock",
        ),
        (
            "dpkg was interrupted",
            "run `sudo dpkg --configure -a`, then retry",
        ),
        (
            "unable to locate package",
            "refresh metadata with `upm update`, then search with `upm search <term>`",
        ),
        (
            "target not found",
            "refresh metadata with `upm update`, then search with `upm search <term>`",
        ),
        (
            "could not resolve host",
            "check your network connection or proxy settings",
        ),
        ("timed out", "check your network connection and retry"),
        ("no space left", "free disk space and retry"),
    ];
    checks
        .iter()
        .find(|(needle, _)| text.contains(needle))
        .map(|(_, hint)| *hint)
        .or_else(|| {
            if text.contains("not found") || text.contains("no package named") {
                Some("refresh metadata with `upm update`, then search with `upm search <term>`")
            } else if text.contains("already installed") || text.contains("same version") {
                Some("use `upm install --force` to reinstall")
            } else if text.contains("unsupported") {
                Some("run `upm capabilities` to see what this backend supports")
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_error_code_is_one() {
        let err = anyhow::anyhow!("boom");
        assert_eq!(code_of(&err), EXIT_ERROR);
    }

    #[test]
    fn custom_code_survives_chain() {
        let err = exit(EXIT_PRIVILEGE, "need root");
        assert_eq!(code_of(&err), EXIT_PRIVILEGE);
        assert!(format!("{err:#}").contains("need root"));
    }

    #[test]
    fn hints_match_common_failures() {
        let err = anyhow::anyhow!("E: Unable to locate package nope");
        assert!(hint_of(&err).unwrap().contains("upm update"));
        let err = anyhow::anyhow!("Could not resolve host: github.com");
        assert!(hint_of(&err).unwrap().contains("network"));
    }
}
