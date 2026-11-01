# upm

**Universal Package Manager — one CLI for every platform's package managers.**

[![ci](https://github.com/neg-sudo/upm/actions/workflows/ci.yml/badge.svg)](https://github.com/neg-sudo/upm/actions/workflows/ci.yml)
[![release](https://github.com/neg-sudo/upm/actions/workflows/release.yml/badge.svg)](https://github.com/neg-sudo/upm/actions/workflows/release.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-teal.svg)](LICENSE)

`upm` detects the package manager(s) on your machine and gives you one set of
commands for all of them. It maps friendly names to the native package for each
backend, shows you the exact command before it runs, and never pretends to
replace the tools that actually install software.

```bash
upm install nodejs ripgrep ffmpeg
upm remove discord
upm upgrade
upm search postgres
upm which
```

## Why this one?

Package managers are excellent and completely inconsistent. `upm` is a thin,
predictable layer on top:

* **One command, every platform** — Homebrew, APT, DNF, pacman, zypper, winget,
  Scoop and Chocolatey.
* **Transparent** — every action prints the real command it will run. `--dry-run`
  shows the whole plan without touching your system.
* **Safe by construction** — commands are executed as argument vectors, never
  through a shell. Package names are validated before anything runs.
* **Smart names** — `nodejs`, `code`, `7zip` and friends resolve to the right
  native package per backend, with user overrides.
* **Honest failures** — categorized exit codes and actionable diagnostics.
* **Automation friendly** — stable `--json` output, transaction plans and shell
  completions.
* **Reproducible environments** — profiles, manifests, snapshots and a local
  transaction history with best-effort rollback.

`upm` does not host packages, download scripts, or wrap installers in `curl |
bash`. It drives the managers you already trust.

## Install

Linux and macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/neg-sudo/upm/main/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/neg-sudo/upm/main/install.ps1 | iex
```

The Unix installer drops a prebuilt binary in `~/.local/bin`. The Windows
installer uses `%USERPROFILE%\.local\bin` and updates your user `PATH`.

Prebuilt binaries for Linux (x86_64, aarch64), macOS (x86_64, aarch64) and
Windows (x86_64) are attached to every tagged release together with a
`SHA256SUMS` file. The installers verify the checksum when possible.

From source:

```bash
git clone https://github.com/neg-sudo/upm.git
cd upm
cargo build --release
./target/release/upm --version
```

`cargo install --path .` also works. Rust 1.85+ is required.

## Quick start

```bash
upm which                       # what did upm detect?
upm doctor                      # environment + backend health
upm install git jq ripgrep      # install a few things
upm info nodejs                 # native name + package details
upm search postgres             # search across every backend
upm outdated                    # see what can be upgraded
upm upgrade --dry-run           # preview a full upgrade
upm upgrade                     # upgrade everything
upm update                      # refresh package metadata
upm clean                       # reclaim caches and orphans
upm remove jq
```

Force a backend when you want a specific one:

```bash
upm --backend brew install nodejs
upm --backend winget install code
```

## Everyday workflows

### Find and inspect

```bash
upm search "code editor" --limit 10     # ranked, fuzzy search across backends
upm search ripgrep --installed          # only installed matches
upm search ripgrep --outdated           # only matches with an update
upm search ripgrep --exact              # exact-name matches only
upm search ripgrep --interactive        # choose a result and install it
upm list --search node                  # filter installed packages
upm outdated --security                 # security updates only (dnf/zypper)
upm capabilities                        # what each backend supports
upm capabilities brew --json
```

### Understand and plan

```bash
upm resolve nodejs code                 # backend + native package names
upm explain install ffmpeg              # resolve and show the exact commands
upm why openssl                         # reverse dependencies
upm depends ripgrep                     # dependencies of a package
upm plan install nodejs jq --out plan.json
upm apply plan.json                     # apply a reviewed plan
```

### Keep machines in sync

```bash
upm init --from-system --file upm.toml  # capture what is installed
upm sync --file upm.toml                # install anything missing
upm export --file upm.toml              # write the current system manifest
```

Manifests are TOML and can be committed to a dotfiles repository:

```toml
[environment]
description = "workstation"

[packages]
git = "*"
ripgrep = "*"
nodejs = "*"

[platform.macos]
nodejs = "*"

[options]
auto_update = true
```

Use profiles for reusable package sets:

```bash
upm profile create dev
upm profile add dev git jq ripgrep
upm profile install dev
upm profile export dev --file dev.toml
upm profile import dev.toml --name dev
upm profile list
```

### Snapshots, history and rollback

```bash
upm snapshot create before-upgrade
upm snapshot list
upm snapshot diff before-upgrade after-upgrade
upm snapshot restore before-upgrade        # install anything missing
upm snapshot export before-upgrade --file snapshot.json
upm history
upm history show 12
upm history export history.jsonl
upm rollback                               # undo the last reversible transaction
upm rollback 12
```

History, profiles and snapshots live under the platform state directory
(`~/.local/state/upm` on Linux) and can be relocated with `UPM_STATE_DIR`.
Rollback is best effort: it inverts `install`/`remove` actions using the
package manager and does not restore exact versions.

### Pinning and cleanup

```bash
upm pin nodejs ripgrep
upm pins
upm unpin nodejs
upm clean               # caches and orphaned packages
upm clean --all         # deeper cleanup where supported
```

Backends that cannot pin or query dependencies report that through
`upm capabilities`.

## Detection order

| OS | Backends, in priority order |
| -- | --------------------------- |
| macOS | Homebrew |
| Linux | APT, DNF, pacman, zypper, Homebrew |
| Windows | winget, Scoop, Chocolatey |

Only installed backends are used. Override the order in the config file with
`general.prefer`, or disable one with `[backends.<id>] enabled = false`.
Inside containers, `upm doctor` highlights the reduced environment it finds.

## Name resolution

Packages do not share names. `upm` resolves each request like this:

1. An explicit `--backend` wins.
2. The highest-priority backend with an alias entry for the name.
3. The highest-priority backend that reports the name in its own search.
4. The highest-priority backend with the raw name, so it can explain the error.

The embedded registry covers common tools. Homebrew casks are addressed as
`cask:<name>` and translated automatically. Extend or override the registry in
`~/.config/upm/config.toml`:

```toml
[general]
prefer = ["brew", "apt"]
confirm = true

[backends.choco]
enabled = false

[packages.nodejs]
brew = "node"
apt = "nodejs"
winget = "OpenJS.NodeJS"
```

Inspect the effective result without installing:

```bash
upm resolve nodejs code 7zip
upm resolve --explain nodejs
upm resolve --json nodejs
```

## Safety model

| Concern | Behaviour |
| ------- | --------- |
| Shell injection | Commands are argv vectors; no shell is used |
| Flag smuggling | Names beginning with `-` are rejected |
| Control characters | Rejected before execution |
| Elevation | `sudo` only on Unix and only for backends that need it; shown first |
| No elevation | `--no-elevate` fails instead of prompting for sudo |
| Windows | No `sudo`; winget elevates itself, Chocolatey expects an elevated shell |
| Existing packages | `install` skips already-installed packages unless `--force` |
| Dry run | `--dry-run` prints commands and runs nothing |
| Confirmation | Interactive prompt before mutating actions; skipped with `--yes` |
| Non-interactive | `--non-interactive` implies `--yes` and never prompts |
| Partial failure | `--keep-going` continues, then exits non-zero with a summary |
| Config permissions | Written `0600` on Unix |

## Global flags

| Flag | Description |
| ---- | ----------- |
| `--backend BACKEND` | Force brew, apt, dnf, pacman, zypper, winget, scoop or choco |
| `-n, --dry-run` | Print commands without running them |
| `-y, --yes` | Use non-interactive flags and skip confirmation |
| `--non-interactive` | Never prompt (implies `--yes`) |
| `--force` | Reinstall / act even if the package is present |
| `--keep-going` | Continue with remaining packages when one fails |
| `--no-elevate` | Never prepend `sudo`; fail if elevation is required |
| `--json` | Structured output |
| `-q, --quiet` | Suppress informational output |
| `-v, --verbose` | Trace commands to stderr |
| `--no-color` | Disable ANSI colour (`NO_COLOR` is also respected) |

## Commands

| Command | Description |
| ------- | ----------- |
| `install PACKAGE...` | Install packages |
| `remove PACKAGE...` | Remove packages (alias: `uninstall`) |
| `upgrade [PACKAGE...]` | Upgrade named packages or everything |
| `update` | Refresh package metadata |
| `search TERM` | Ranked search with filters |
| `list` | List installed packages |
| `outdated` | List available upgrades |
| `info PACKAGE` | Show package details |
| `which` | Show detected backends and environment |
| `doctor [--fix]` | Diagnose environment and backend health |
| `resolve [--explain] PACKAGE...` | Resolve names without changing anything |
| `explain ACTION PACKAGE...` | Show what an action would run |
| `capabilities [BACKEND]` | Show the backend capability matrix |
| `clean [--all]` | Reclaim caches and orphaned packages |
| `pin` / `unpin` / `pins` | Manage version pins |
| `why PACKAGE` / `depends PACKAGE` | Inspect dependency relationships |
| `history [show ID\|export PATH]` | Transaction history |
| `rollback [ID]` | Best-effort undo |
| `init` / `export` / `sync` | Manifest workflows |
| `plan ACTION PACKAGE... --out PATH` | Save a transaction plan |
| `apply FILE` | Apply a transaction plan |
| `profile ...` | Manage reusable package profiles |
| `snapshot ...` | Capture, diff and restore environments |
| `config [path\|show\|init]` | Inspect or create the config file |
| `completions SHELL` | Print shell completions |

See [docs/cli-reference.md](docs/cli-reference.md) for every subcommand and flag.

## Exit codes

| Code | Meaning |
| ---- | ------- |
| 0 | Success |
| 1 | Execution, I/O or backend error |
| 2 | Usage error, bad package name, unavailable forced backend |
| 3 | Privilege problem |
| 4 | Package or backend not found |
| 5 | No supported package manager detected |
| 6 | Backend does not support the operation |

```bash
upm info does-not-exist
case $? in
  4) echo "not found" ;;
  1) echo "backend failed" ;;
esac
```

## JSON

Every read command and every mutating action has a stable, schema-versioned JSON
shape.

```bash
upm resolve nodejs --json
upm search ripgrep --json
upm outdated --json
upm capabilities --json
upm history --json
upm install ripgrep --dry-run --json
upm plan install ripgrep --json
```

Mutating commands emit a plan summary:

```json
{
  "schema_version": 1,
  "action": "install",
  "commands": ["sudo apt-get install -y ripgrep"],
  "dry_run": true,
  "status": "planned",
  "packages": [
    { "backend": "apt", "package": "ripgrep", "requested": "ripgrep" }
  ],
  "failures": []
}
```

Diagnostics always go to stderr; JSON goes to stdout.

## Completions

```bash
upm completions bash > /etc/bash_completion.d/upm
upm completions zsh  > "${fpath[1]}/_upm"
upm completions fish > ~/.config/fish/completions/upm.fish
upm completions powershell | Out-String | Invoke-Expression
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

## Environment

| Variable | Meaning |
| -------- | ------- |
| `UPM_CONFIG` | Override the config file path |
| `UPM_STATE_DIR` | Override the state directory for history, profiles and snapshots |
| `NO_COLOR` | Disable colour when present |

## Limitations

* `upm` is only as good as the backend underneath it. If a package is missing
  from your manager's index, `upm` reports the backend's error.
* Chocolatey has no index-refresh command, so `upm update` skips it.
* `pacman -Sy` refreshes indexes; on Arch, follow with `upm upgrade`.
* Security-only upgrades are available on DNF and zypper; other backends report
  the limitation.
* Dependency inspection and pinning are not available on every backend; see
  `upm capabilities`.
* Rollback reverses actions, not versions.
* Elevation on Windows is handled by the package manager (UAC), not by `upm`.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

See [docs/cli-reference.md](docs/cli-reference.md) and
[docs/design.md](docs/design.md).

## License

MIT. See [LICENSE](LICENSE).
