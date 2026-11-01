//! Package name validation.
//!
//! We never build a shell string, but we still reject names that could be
//! interpreted as flags or that contain control characters. This prevents a
//! typo like `upm install --force` from silently becoming an option to the
//! underlying package manager.

use crate::error::{EXIT_USAGE, exit};

const MAX_LEN: usize = 200;

pub fn package_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        return Err(exit(EXIT_USAGE, "empty package name"));
    }
    if name.starts_with('-') {
        return Err(exit(
            EXIT_USAGE,
            format!("refusing package name `{name}`: names may not start with `-`"),
        ));
    }
    if name.len() > MAX_LEN {
        return Err(exit(
            EXIT_USAGE,
            format!("package name is too long (max {MAX_LEN} bytes)"),
        ));
    }
    if name.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(exit(
            EXIT_USAGE,
            format!("refusing package name `{name}`: contains whitespace or control characters"),
        ));
    }
    let allowed = name.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '-' | '_' | '.' | '+' | '@' | '/' | ':' | '~' | '^' | '=' | '[' | ']'
            )
    });
    if !allowed {
        return Err(exit(
            EXIT_USAGE,
            format!("refusing package name `{name}`: contains unsupported characters"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_names() {
        for name in [
            "ripgrep",
            "python3.12",
            "openjdk-21-jdk",
            "docker.io",
            "cask:visual-studio-code",
            "hashicorp/terraform/terraform",
            "@scope/pkg",
            "pkg@1.2.3",
            "C++",
        ] {
            assert!(package_name(name).is_ok(), "rejected {name}");
        }
    }

    #[test]
    fn rejects_flags_and_injection() {
        for name in ["--force", "-rf", "a b", "a;rm -rf /", "a$(whoami)", "a\nb"] {
            assert!(package_name(name).is_err(), "accepted {name}");
        }
    }
}
