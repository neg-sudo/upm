//! Command-line interface definition.

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "upm",
    version,
    about = "Universal Package Manager — one CLI for every platform's package managers",
    long_about = "upm detects the package manager(s) available on this machine and gives you one \
                  set of commands to install, remove, upgrade, search and inspect packages across \
                  Homebrew, APT, DNF, pacman, zypper, winget, Scoop and Chocolatey.\n\n\
                  Exit codes: 0 ok, 1 error, 2 usage, 3 privilege, 4 not found, 5 no backend, \
                  6 unsupported.",
    propagate_version = true,
    disable_help_subcommand = true,
    arg_required_else_help = true
)]
pub struct Cli {
    /// Force a specific backend (brew, apt, dnf, pacman, zypper, winget, scoop, choco)
    #[arg(long, global = true, value_name = "BACKEND")]
    pub backend: Option<String>,

    /// Show what would run without executing anything
    #[arg(short = 'n', long, global = true)]
    pub dry_run: bool,

    /// Assume yes: use non-interactive flags and skip the confirmation prompt
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// Reinstall or act even when the package is already present
    #[arg(long, global = true)]
    pub force: bool,

    /// Structured JSON output
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress informational output
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    /// Verbose logging to stderr
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// Disable ANSI colour
    #[arg(long, global = true)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Install one or more packages
    Install {
        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// Remove one or more packages
    #[command(alias = "uninstall")]
    Remove {
        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// Upgrade packages, or every package when none are given
    Upgrade {
        #[arg(value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// Refresh package metadata for every detected backend
    Update,

    /// Search across available backends
    Search {
        #[arg(value_name = "TERM")]
        term: String,
    },

    /// List installed packages
    List,

    /// Show details for a package
    Info {
        #[arg(value_name = "PACKAGE")]
        package: String,
    },

    /// Show detected backends and environment details
    Which,

    /// Diagnose the environment and backend health
    Doctor,

    /// Resolve package names without installing anything
    Resolve {
        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// Manage the configuration file
    Config {
        #[command(subcommand)]
        cmd: Option<ConfigCmd>,
    },

    /// Print shell completions
    Completions {
        /// bash, zsh, fish, powershell or elvish
        #[arg(value_name = "SHELL")]
        shell: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print the config file path
    Path,
    /// Print the effective config
    Show,
    /// Create a default config file if none exists
    Init,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn globals_can_follow_subcommand() {
        let cli = Cli::try_parse_from(["upm", "install", "git", "--dry-run", "-y"]).unwrap();
        assert!(cli.dry_run);
        assert!(cli.yes);
        match cli.command {
            Command::Install { packages } => assert_eq!(packages, vec!["git"]),
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn requires_a_subcommand() {
        assert!(Cli::try_parse_from(["upm"]).is_err());
    }
}
