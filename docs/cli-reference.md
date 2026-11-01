# CLI reference

```
upm [global options] <command> [args]
```

## Commands

| Command | Description |
| ------- | ----------- |
| `install PACKAGE...` | Install one or more packages |
| `remove PACKAGE...` | Remove one or more packages (alias: `uninstall`) |
| `upgrade [PACKAGE...]` | Upgrade named packages, or everything when none are given |
| `update` | Refresh package metadata for every detected backend |
| `search TERM` | Search across available backends |
| `list` | List installed packages per backend |
| `info PACKAGE` | Show native name and backend details |
| `which` | Show detected backends and environment |
| `doctor` | Diagnose the environment and backend health |
| `resolve PACKAGE...` | Resolve names without changing anything |
| `config [path\|show\|init]` | Inspect or create the config file |
| `completions SHELL` | Print completions (bash, zsh, fish, powershell, elvish) |

## Global options

| Flag | Description |
| ---- | ----------- |
| `--backend BACKEND` | Force a backend: brew, apt, dnf, pacman, zypper, winget, scoop, choco |
| `-n, --dry-run` | Print commands without executing them |
| `-y, --yes` | Use non-interactive flags and skip confirmation |
| `--force` | Reinstall / act even if already present |
| `--json` | Structured output on stdout |
| `-q, --quiet` | Suppress informational output |
| `-v, --verbose` | Trace commands to stderr |
| `--no-color` | Disable ANSI colour |
| `-h, --help` | Help |
| `-V, --version` | Version |

Global options may appear before or after the subcommand.

## Exit codes

| Code | Meaning |
| ---- | ------- |
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
upm install --dry-run -y nodejs ripgrep
upm --backend brew install nodejs
upm resolve nodejs code
upm upgrade
upm search postgres
upm info ffmpeg
upm config init
upm completions zsh
```

## Environment

| Variable | Meaning |
| -------- | ------- |
| `UPM_CONFIG` | Override the config file path |
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
