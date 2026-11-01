//! Process execution with dry-run, elevation and verbose tracing.
//!
//! Commands are always passed as an argv vector — never through a shell — so
//! package names cannot be reinterpreted as shell syntax.

use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result};

use crate::error::{EXIT_PRIVILEGE, exit};
use crate::output;

/// Executes backend commands according to the global CLI options.
#[derive(Debug, Clone, Copy)]
pub struct Runner {
    pub dry_run: bool,
    pub assume_yes: bool,
    pub quiet: bool,
}

impl Runner {
    /// Run a command, inheriting stdio so progress and password prompts work.
    pub fn run(&self, argv: &[String], elevate: bool) -> Result<()> {
        let argv = self.effective(argv, elevate)?;
        output::command(&argv);
        if self.dry_run {
            return Ok(());
        }
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        if self.quiet {
            // Keep stdin/stderr so sudo can still prompt for a password and
            // backend errors are never hidden.
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit());
        } else {
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }
        output::log(&format!("exec {}", output::shell_join(&argv)));
        let status = cmd
            .status()
            .with_context(|| format!("failed to launch `{}`", argv[0]))?;
        if !status.success() {
            return Err(anyhow::anyhow!(
                "`{}` exited with {}",
                output::shell_join(&argv),
                status
            ));
        }
        Ok(())
    }

    /// Capture a command's output for parsing (search, probes, diagnostics).
    pub fn capture(&self, argv: &[String]) -> Result<Output> {
        output::log(&format!("capture {}", output::shell_join(argv)));
        Command::new(&argv[0])
            .args(&argv[1..])
            .output()
            .with_context(|| format!("failed to launch `{}`", argv[0]))
    }

    fn effective(&self, argv: &[String], elevate: bool) -> Result<Vec<String>> {
        if !elevate || !needs_sudo() || is_root()? {
            return Ok(argv.to_vec());
        }
        let mut out = Vec::with_capacity(argv.len() + 1);
        out.push("sudo".to_string());
        out.extend_from_slice(argv);
        if !self.dry_run && which::which("sudo").is_err() {
            return Err(exit(
                EXIT_PRIVILEGE,
                format!(
                    "`{}` needs root privileges and `sudo` is not installed; run upm as root",
                    argv[0]
                ),
            ));
        }
        Ok(out)
    }
}

/// We only prepend `sudo` on Unix. On Windows the package managers elevate
/// themselves through UAC (winget) or require an already-elevated shell.
fn needs_sudo() -> bool {
    cfg!(unix)
}

pub fn is_root() -> Result<bool> {
    if cfg!(windows) {
        return Ok(false);
    }
    let out = Command::new("id")
        .arg("-u")
        .output()
        .context("failed to run `id -u`")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim() == "0")
}

/// Best-effort admin detection used by `doctor`.
pub fn is_admin() -> bool {
    if cfg!(windows) {
        Command::new("net")
            .arg("session")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else {
        is_root().unwrap_or(false)
    }
}

/// Read the first non-empty line of `<binary> --version`.
pub fn version_of(binary: &str) -> Option<String> {
    let out = Command::new(binary).arg("--version").output().ok()?;
    let text = if out.stdout.is_empty() {
        String::from_utf8_lossy(&out.stderr).to_string()
    } else {
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(120).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_never_executes() {
        let runner = Runner {
            dry_run: true,
            assume_yes: true,
            quiet: true,
        };
        let argv = vec!["this-binary-does-not-exist".into(), "x".into()];
        assert!(runner.run(&argv, false).is_ok());
    }

    #[test]
    fn missing_binary_is_reported() {
        let runner = Runner {
            dry_run: false,
            assume_yes: true,
            quiet: true,
        };
        let argv = vec!["upm-definitely-not-a-binary".into()];
        assert!(runner.run(&argv, false).is_err());
    }
}
