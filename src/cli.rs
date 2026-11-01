//! Command-line interface definition.

use std::path::PathBuf;

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

    /// Never prompt (implies --yes)
    #[arg(long, global = true)]
    pub non_interactive: bool,

    /// Reinstall or act even when the package is already present
    #[arg(long, global = true)]
    pub force: bool,

    /// Continue with remaining packages when one fails
    #[arg(long, global = true)]
    pub keep_going: bool,

    /// Never prepend sudo (fail if elevation is required)
    #[arg(long, global = true)]
    pub no_elevate: bool,

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
        /// Apply security updates only
        #[arg(long)]
        security: bool,

        /// Pick packages interactively
        #[arg(long)]
        interactive: bool,

        #[arg(value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// Refresh package metadata for every detected backend
    Update,

    /// Search across available backends
    Search {
        #[arg(value_name = "TERM")]
        term: String,

        /// Only exact name matches
        #[arg(long)]
        exact: bool,

        /// Only installed packages
        #[arg(long)]
        installed: bool,

        /// Only packages with an available update
        #[arg(long)]
        outdated: bool,

        /// Maximum number of results
        #[arg(long, default_value_t = 25, value_name = "N")]
        limit: usize,

        /// Select and install a result interactively
        #[arg(long)]
        interactive: bool,
    },

    /// List installed packages
    List {
        /// Only packages with an available update
        #[arg(long)]
        outdated: bool,

        /// Filter by name or description
        #[arg(long, value_name = "TERM")]
        search: Option<String>,

        /// Maximum number of results
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
    },

    /// List packages with available upgrades
    Outdated {
        /// Only security updates
        #[arg(long)]
        security: bool,
    },

    /// Show details for a package
    Info {
        #[arg(value_name = "PACKAGE")]
        package: String,
    },

    /// Show detected backends and environment details
    Which,

    /// Diagnose the environment and backend health
    Doctor {
        /// Print safe repair suggestions for detected problems
        #[arg(long)]
        fix: bool,
    },

    /// Resolve package names without installing anything
    Resolve {
        /// Explain every resolution decision
        #[arg(long)]
        explain: bool,

        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// Explain what an action would do, without executing it
    Explain {
        /// install, remove or upgrade
        #[arg(value_name = "ACTION")]
        action: String,

        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// Show backend capability matrix
    Capabilities {
        /// Limit output to one backend
        #[arg(value_name = "BACKEND")]
        backend: Option<String>,
    },

    /// Reclaim package-manager caches and orphaned packages
    Clean {
        /// Perform a deeper cleanup where supported
        #[arg(long)]
        all: bool,
    },

    /// Pin packages to their current version
    Pin {
        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// Unpin packages
    Unpin {
        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// List pinned packages
    Pins,

    /// Show what depends on a package
    Why {
        #[arg(value_name = "PACKAGE")]
        package: String,
    },

    /// Show what a package depends on
    Depends {
        #[arg(value_name = "PACKAGE")]
        package: String,
    },

    /// Show or export transaction history
    History {
        #[command(subcommand)]
        cmd: Option<HistoryCmd>,
    },

    /// Best-effort rollback of an install or remove transaction
    Rollback {
        /// Transaction id (defaults to the last reversible transaction)
        #[arg(value_name = "ID")]
        id: Option<u64>,
    },

    /// Create a manifest from the current system
    Init {
        /// Capture installed packages
        #[arg(long)]
        from_system: bool,

        /// Manifest path
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,
    },

    /// Export the current system to a manifest
    Export {
        /// Manifest path
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,
    },

    /// Install everything declared in a manifest
    Sync {
        /// Manifest path
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,
    },

    /// Build a transaction plan and print or save it
    Plan {
        /// install, remove or upgrade
        #[arg(value_name = "ACTION")]
        action: String,

        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,

        /// Write the plan to a JSON file
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
    },

    /// Apply a transaction plan
    Apply {
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Manage package profiles
    Profile {
        #[command(subcommand)]
        cmd: ProfileCmd,
    },

    /// Manage environment snapshots
    Snapshot {
        #[command(subcommand)]
        cmd: SnapshotCmd,
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
pub enum HistoryCmd {
    /// Show one transaction
    Show {
        #[arg(value_name = "ID")]
        id: u64,
    },
    /// Export history as JSON lines
    Export {
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProfileCmd {
    /// List profiles
    List,
    /// Create an empty profile
    Create {
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Delete a profile
    Delete {
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Add packages to a profile
    Add {
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },
    /// Remove packages from a profile
    Remove {
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },
    /// Show a profile
    Show {
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Install everything in a profile
    Install {
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Export a profile to a file
    Export {
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Import a profile from a file
    Import {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long, value_name = "NAME")]
        name: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum SnapshotCmd {
    /// Capture the current environment
    Create {
        #[arg(value_name = "NAME")]
        name: Option<String>,
    },
    /// List snapshots
    List,
    /// Show a snapshot
    Show {
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Delete a snapshot
    Delete {
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Export a snapshot to a file
    Export {
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Compare two snapshots (name or file path)
    Diff {
        #[arg(value_name = "A")]
        a: String,
        #[arg(value_name = "B")]
        b: String,
    },
    /// Install packages missing from a snapshot
    Restore {
        #[arg(value_name = "NAME")]
        name: String,
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

    #[test]
    fn parses_search_options() {
        let cli = Cli::try_parse_from([
            "upm",
            "search",
            "code editor",
            "--exact",
            "--installed",
            "--limit",
            "5",
        ])
        .unwrap();
        match cli.command {
            Command::Search {
                term,
                exact,
                installed,
                limit,
                ..
            } => {
                assert_eq!(term, "code editor");
                assert!(exact);
                assert!(installed);
                assert_eq!(limit, 5);
            }
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn parses_profile_and_snapshot_commands() {
        assert!(Cli::try_parse_from(["upm", "profile", "add", "dev", "git"]).is_ok());
        assert!(Cli::try_parse_from(["upm", "snapshot", "diff", "a", "b"]).is_ok());
    }
}
