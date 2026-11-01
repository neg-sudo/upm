# CLI reference

```
upm [global options] <command> [args]
```

Global options may appear before or after the subcommand.

## Package commands

| Command | Description |
| ------- | ----------- |
| `install PACKAGE...` | Install one or more packages |
| `remove PACKAGE...` | Remove one or more packages (alias: `uninstall`) |
| `upgrade [PACKAGE...]` | Upgrade named packages, or everything when none are given |
| `update` | Refresh package metadata for every detected backend |
| `search TERM` | Ranked search across available backends |
| `list` | List installed packages per backend |
| `outdated` | List packages with available upgrades |
| `info PACKAGE` | Show native name and backend details |
| `which` | Show detected backends and environment |
| `doctor` | Diagnose the environment and backend health |
| `resolve PACKAGE...` | Resolve names without changing anything |
| `explain ACTION PACKAGE...` | Show the commands an action would run |
| `capabilities [BACKEND]` | Show the backend capability matrix |
| `clean` | Reclaim package-manager caches and orphaned packages |
| `pin PACKAGE...` / `unpin PACKAGE...` / `pins` | Manage version pins |
| `why PACKAGE` / `depends PACKAGE` | Inspect dependency relationships |

### Options

| Command | Option | Description |
| ------- | ------ | ----------- |
| `upgrade` | `--security` | Apply security updates only (DNF, zypper) |
| `upgrade` | `--interactive` | Pick packages to upgrade interactively |
| `search` | `--exact` | Only exact name matches |
| `search` | `--installed` | Only installed packages |
| `search` | `--outdated` | Only packages with an available update |
| `search` | `--limit N` | Maximum results (default 25) |
| `search` | `--interactive` | Select a result and install it |
| `list` | `--outdated` | Only packages with an available update |
| `list` | `--search TERM` | Filter by name or description |
| `list` | `--limit N` | Maximum results |
| `outdated` | `--security` | Only security updates |
| `doctor` | `--fix` | Print safe repair suggestions |
| `resolve` | `--explain` | Explain each resolution decision |
| `clean` | `--all` | Deeper cleanup where supported |

## Manifest and plan commands

| Command | Description |
| ------- | ----------- |
| `init [--from-system] [--file PATH]` | Create a manifest from the current system |
| `export [--file PATH]` | Export the current system to a manifest |
| `sync [--file PATH]` | Install everything declared in a manifest |
| `plan ACTION PACKAGE... [--out PATH]` | Build a transaction plan and print or save it |
| `apply FILE` | Apply a transaction plan |

`ACTION` is `install`, `remove` or `upgrade`. The default manifest path is
`upm.toml`.

## History and rollback

| Command | Description |
| ------- | ----------- |
| `history` | Show the most recent transactions |
| `history show ID` | Show one transaction |
| `history export [PATH]` | Export history as JSON lines |
| `rollback [ID]` | Best-effort rollback of an install or remove transaction |

Without an ID, `rollback` chooses the last reversible transaction. Rollback
inverts actions; it does not restore exact versions.

## Profiles

| Command | Description |
| ------- | ----------- |
| `profile list` | List profiles |
| `profile create NAME` | Create an empty profile |
| `profile delete NAME` | Delete a profile |
| `profile add NAME PACKAGE...` | Add packages |
| `profile remove NAME PACKAGE...` | Remove packages |
| `profile show NAME` | Show a profile |
| `profile install NAME` | Install everything in a profile |
| `profile export NAME [PATH]` | Export a profile to a file |
| `profile import PATH [--name NAME]` | Import a profile from a file |

## Snapshots

| Command | Description |
| ------- | ----------- |
| `snapshot create [NAME]` | Capture the current environment |
| `snapshot list` | List snapshots |
| `snapshot show NAME` | Show a snapshot |
| `snapshot delete NAME` | Delete a snapshot |
| `snapshot export NAME [PATH]` | Export a snapshot to a file |
| `snapshot diff A B` | Compare two snapshots (names or file paths) |
| `snapshot restore NAME` | Install packages missing from a snapshot |

## Config and completions

| Command | Description |
| ------- | ----------- |
| `config` / `config show` | Print the effective configuration |
| `config path` | Print the config file path |
| `config init` | Create a default config file if none exists |
| `completions SHELL` | Print completions (bash, zsh, fish, powershell, elvish) |

## Global options

| Flag | Description |
| ---- | ----------- |
| `--backend BACKEND` | Force a backend: brew, apt, dnf, pacman, zypper, winget, scoop, choco |
| `-n, --dry-run` | Print commands without executing them |
| `-y, --yes` | Use non-interactive flags and skip confirmation |
| `--non-interactive` | Never prompt (implies `--yes`) |
| `--force` | Reinstall / act even if already present |
| `--keep-going` | Continue with remaining packages when one fails |
| `--no-elevate` | Never prepend `sudo`; fail if elevation is required |
| `--json` | Structured output on stdout |
| `-q, --quiet` | Suppress informational output |
| `-v, --verbose` | Trace commands to stderr |
| `--no-color` | Disable ANSI colour |
| `-h, --help` | Help |
| `-V, --version` | Version |

## Exit codes

| Code | Meaning |
| ---- | ----------- |
| 0 | Success |
| 1 | Execution, I/O or backend error |
| 2 | Usage error, invalid package name, forced backend unavailable |
| 3 | Privilege problem (missing `sudo`, permission denied) |
| 4 | Package or backend not found |
| 5 | No supported package manager detected |
| 6 | Backend does not support the operation |

## Examples

```bash
upm which --json
upm doctor
upm doctor --fix
upm install --dry-run -y nodejs ripgrep
upm --backend brew install nodejs
upm search postgres --limit 5
upm outdated --security
upm resolve --explain nodejs code
upm upgrade
upm upgrade --interactive
upm clean --all
upm pin nodejs
upm why openssl
upm config init
upm completions zsh
```

## Environment

| Variable | Meaning |
| -------- | ------- |
| `UPM_CONFIG` | Override the config file path |
| `UPM_STATE_DIR` | Override the state directory for history, profiles and snapshots |
| `NO_COLOR` | Disable colour when present |

## Config file

`$UPM_CONFIG`, else `<config_dir>/upm/config.toml` (for example
`~/.config/upm/config.toml` on Linux, `~/Library/Application Support/upm` on
macOS, `%APPDATA%\upm` on Windows).

```toml
[general]
prefer = ["brew", "apt"]
confirm = true
color = true

[backends.choco]
enabled = false

[packages.nodejs]
apt = "nodejs"
brew = "node"
winget = "OpenJS.NodeJS"
```

## State directory

History, profiles and snapshots are stored under `$UPM_STATE_DIR` when set,
otherwise `<state_dir>/upm` (`~/.local/state/upm` on Linux).

```
upm/
├── history.jsonl
├── profiles/
└── snapshots/
```
