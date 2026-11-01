# Design

`upm` is a resolver and an executor. It does not install software itself.

## Layers

```
cli  ──►  resolver  ──►  backend adapter  ──►  exec::Runner  ──►  OS process
             │                                          │
        alias registry                              sudo / dry-run / confirm
```

1. **cli** parses flags and subcommands (`clap`).
2. **detect** chooses the backends for the current OS, filtered by availability
   and config, ordered by priority and `general.prefer`.
3. **resolver** maps each requested name to `(backend, native name)`.
4. **backend** turns an operation into an argv vector.
5. **exec** optionally prefixes `sudo`, prints the command, runs it with
   inherited stdio, and maps failures to exit codes.

## Resolution rules

1. `--backend` wins, with the raw name unless an alias exists for that backend.
2. Highest-priority available backend that has an alias entry.
3. Highest-priority backend whose `search` output mentions the name.
4. Highest-priority backend with the raw name (the backend reports the error).

Aliases are seeded from `src/aliases.toml` and merged with `[packages.*]` from the
user config. User keys override embedded ones.

## Safety

* No shell: every command is `Command::new(bin).args([...])`.
* Names are validated (`[A-Za-z0-9@/_.:+-~^=[]]`, no leading `-`, no whitespace
  or control characters).
* `sudo` is only added on Unix, only for backends that need it, and never in
  dry-run. If `sudo` is missing, the command fails with exit code 3.
* Windows relies on winget's UAC prompt and on Chocolatey/Scoop requiring an
  elevated or user-scoped shell respectively.
* `install` probes for existing packages and skips them unless `--force`.

## Idempotency and probes

Each backend exposes a `probe_cmd` plus an interpreter:

| Backend | Probe |
| ------- | ----- |
| brew | `brew list --formula/--cask NAME` |
| apt | `dpkg-query -W -f=${db:Status-Abbrev} NAME` and require `ii` |
| dnf, zypper | `rpm -q NAME` |
| pacman | `pacman -Q NAME` |
| winget | `winget list --id NAME --exact` and match output |
| scoop, choco | list output match |

If a probe is unavailable or cannot be interpreted, `upm` proceeds and lets the
backend decide. It never blocks an install on a failed probe.

## Non-goals

* Hosting packages or maintaining a binary repository.
* Reimplementing dependency resolution.
* Downloading and executing install scripts.
* GUI or TUI (the CLI is scriptable; a TUI could be added later).
