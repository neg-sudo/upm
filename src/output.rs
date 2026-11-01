//! Terminal output with the same visual language as `extract`:
//! `ℹ` info, `✓` success, `⚠` warning, `✗` error, dim `→` commands.
//!
//! Human-readable output is suppressed by `--quiet` and `--json`. Structured
//! data always goes to stdout, diagnostics always go to stderr.

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

static COLOR: AtomicBool = AtomicBool::new(false);
static JSON: AtomicBool = AtomicBool::new(false);
static QUIET: AtomicBool = AtomicBool::new(false);
static VERBOSE: AtomicBool = AtomicBool::new(false);

const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[91m";
const GREEN: &str = "\x1b[92m";
const YELLOW: &str = "\x1b[93m";
const CYAN: &str = "\x1b[96m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

pub fn init(no_color: bool, force_color: Option<bool>, json: bool, quiet: bool, verbose: bool) {
    let env_no_color = std::env::var_os("NO_COLOR").is_some();
    let tty = std::io::stdout().is_terminal();
    let enabled = if no_color || env_no_color || json {
        false
    } else if let Some(force) = force_color {
        force
    } else {
        tty
    };
    COLOR.store(enabled, Ordering::Relaxed);
    JSON.store(json, Ordering::Relaxed);
    QUIET.store(quiet, Ordering::Relaxed);
    VERBOSE.store(verbose, Ordering::Relaxed);
}

fn paint(text: &str, code: &str) -> String {
    if COLOR.load(Ordering::Relaxed) {
        format!("{code}{text}{RESET}")
    } else {
        text.to_string()
    }
}

pub fn is_json() -> bool {
    JSON.load(Ordering::Relaxed)
}

pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

/// `ℹ` informational line on stdout.
pub fn info(message: &str) {
    if !is_quiet() && !is_json() {
        println!("{} {message}", paint("ℹ", CYAN));
    }
}

/// `✓` success line on stdout.
pub fn success(message: &str) {
    if !is_quiet() && !is_json() {
        println!("{} {message}", paint("✓", GREEN));
    }
}

/// `⚠` warning on stderr.
pub fn warn(message: &str) {
    if !is_quiet() && !is_json() {
        eprintln!("{} {message}", paint("⚠", YELLOW));
    }
}

/// `✗` error on stderr. Always emitted so failures are never silent.
pub fn error(message: &str) {
    eprintln!("{} {message}", paint("✗", RED));
}

/// Dim `[upm]` trace line, only with `--verbose`.
pub fn log(message: &str) {
    if is_verbose() && !is_quiet() && !is_json() {
        eprintln!("{} {message}", paint("[upm]", DIM));
    }
}

/// A plain indented detail line, suppressed by `--quiet` / `--json`.
pub fn detail(message: &str) {
    if !is_quiet() && !is_json() {
        println!("{message}");
    }
}

/// A bold section heading, suppressed by `--quiet` / `--json`.
pub fn header(message: &str) {
    if !is_quiet() && !is_json() {
        println!();
        println!("{}", paint(message, BOLD));
    }
}

/// A `→ command` trace line (printed in dry-run too).
pub fn command(argv: &[String]) {
    if !is_quiet() && !is_json() {
        println!("{} {}", paint("→", DIM), paint(&shell_join(argv), DIM));
    }
}

/// Emit pretty JSON on stdout.
pub fn json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn green(text: &str) -> String {
    paint(text, GREEN)
}

pub fn yellow(text: &str) -> String {
    paint(text, YELLOW)
}

pub fn red(text: &str) -> String {
    paint(text, RED)
}

pub fn bold(text: &str) -> String {
    paint(text, BOLD)
}

pub fn dim(text: &str) -> String {
    paint(text, DIM)
}

/// Quote an argv vector for safe display in a shell.
pub fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '-' | '_' | '.' | '/' | ':' | '@' | '+' | '=' | ',')
        });
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_when_needed() {
        assert_eq!(shell_quote("ripgrep"), "ripgrep");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn joins_argv() {
        let argv = vec!["apt-get".into(), "install".into(), "pkg".into()];
        assert_eq!(shell_join(&argv), "apt-get install pkg");
    }
}
