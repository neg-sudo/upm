# upm

**Universal Package Manager — one CLI for every platform's package managers.**

[![ci](https://github.com/zzzit0ti-cell/upm/actions/workflows/ci.yml/badge.svg)](https://github.com/zzzit0ti-cell/upm/actions/workflows/ci.yml)
[![release](https://github.com/zzzit0ti-cell/upm/actions/workflows/release.yml/badge.svg)](https://github.com/zzzit0ti-cell/upm/actions/workflows/release.yml)
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
* **Automation friendly** — stable `--json` output and shell completions.

`upm` does not host packages, download scripts, or wrap installers in `curl |
bash`. It drives the managers you already trust.

## Install

Linux and macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/zzzit0ti-cell/upm/main/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/zzzit0ti-cell/upm/main/install.ps1 | iex
```

The Unix installer drops a static binary in `~/.local/bin`. The Windows
installer uses `%USERPROFILE%\.local\bin` and updates your user `PATH`.

Prebuilt binaries for Linux, macOS and Windows (x86_64 and aarch64) are attached
to every tagged release with `SHA256SUMS`.

From source:

```bash
git clone https://github.com/zzzit0ti-cell/upm.git
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
upm upgrade --dry-run           # preview a full upgrade
upm upgrade
upm update                      # refresh package metadata
upm remove jq
```

Force a backend when you want a specific one:

```bash
upm --backend brew install nodejs
upm --backend winget install code
```

## Detection order

| OS | Backends, in priority order |
| -- | --------------------------- |
| macOS | Homebrew |
| Linux | APT, DNF, pacman, zypper, Homebrew |
| Windows | winget, Scoop, Chocolatey |

Only installed backends are used. Override the order in the config file with
`general.prefer`, or disable one with `[backends.<id>] enabled = false`.

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
upm resolve --json nodejs
```

## Safety model

| Concern | Behaviour |
| ------- | --------- |
| Shell injection | Commands are argv vectors; no shell is used |
| Flag smuggling | Names beginning with `-` are rejected |
| Control characters | Rejected before execution |
| Elevation | `sudo` only on Unix and only for backends that need it; shown first |
| Windows | No `sudo`; winget elevates itself, Chocolatey expects an elevated shell |
| Existing packages | `install` skips already-installed packages unless `--force` |
| Dry run | `--dry-run` prints commands and runs nothing |
| Confirmation | Interactive prompt before mutating actions; skipped with `--yes` |
| Config permissions | Written `0600` on Unix |

## Global flags

| Flag | Description |
| ---- | ----------- |
| `--backend BACKEND` | Force brew, apt, dnf, pacman, zypper, winget, scoop or choco |
| `-n, --dry-run` | Print commands without running them |
| `-y, --yes` | Use non-interactive flags and skip confirmation |
| `--force` | Reinstall / act even if the package is present |
| `--json` | Structured output |
| `-q, --quiet` | Suppress informational output |
| `-v, --verbose` | Trace commands to stderr |
| `--no-color` | Disable ANSI colour (`NO_COLOR` is also respected) |

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

```bash
upm resolve nodejs --json
upm which --json
upm install ripgrep --dry-run --json
```

Mutating commands emit a plan summary when `--json` is set:

```json
{
  "action": "install",
  "commands": ["sudo apt-get install -y ripgrep"],
  "dry_run": true,
  "packages": [
    { "backend": "apt", "package": "ripgrep", "requested": "ripgrep" }
  ]
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

## Limitations

* `upm` is only as good as the backend underneath it. If a package is missing
  from your manager's index, `upm` reports the backend's error.
* Chocolatey has no index-refresh command, so `upm update` skips it.
* `pacman -Sy` refreshes indexes; on Arch, follow with `upm upgrade`.
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
