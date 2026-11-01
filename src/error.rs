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
}
