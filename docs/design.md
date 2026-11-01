# Design

`upm` is a resolver and an executor. It does not install software itself.

## Layers

```
cli ──► app ──► resolver ──► backend adapter ──► exec::Runner ──► OS process
          │        │                                  │
          │   alias registry                   sudo / dry-run / confirm
          │
          ├── manifest / profiles / snapshots
          ├── plan / apply
          ├── history / rollback
          └── parse / capabilities
```

1. **cli** parses flags and subcommands (`clap`).
2. **detect** chooses the backends for the current OS, filtered by availability
   and config, ordered by priority and `general.prefer`.
3. **resolver** maps each requested name to `(backend, native name)`.
4. **backend** turns an operation into an argv vector.
5. **exec** optionally prefixes `sudo`, prints the command, runs it with
   inherited stdio, and maps failures to exit codes.
6. **app** coordinates workflows, JSON reports and side effects such as history.
7. **parse** turns backend-specific search/list/outdated output into a common
   model for display and filtering.
8. **manifest** stores profiles, snapshots and environment manifests as TOML or
   JSON under the state directory.

## Resolution rules

1. `--backend` wins, with the raw name unless an alias exists for that backend.
2. Highest-priority available backend that has an alias entry.
3. Highest-priority backend whose `search` output mentions the name.
4. Highest-priority backend with the raw name (the backend reports the error).

Aliases are seeded from `src/aliases.toml` and merged with `[packages.*]` from the
user config. User keys override embedded ones.

## Read-only workflows

`search`, `list`, `outdated`, `which`, `doctor` and `capabilities` run backend
queries in parallel and combine the results. Parsers are best effort: if a
backend changes its output, `upm` falls back to showing the raw line rather than
failing the whole command.

## Transaction workflow

```
plan ──► JSON plan file ──► apply ──► same executor as install/remove/upgrade
```

A plan records the action, resolved packages and exact commands. `apply`
validates the schema and package names again, then executes through the same
runner. Plans are data, not scripts, and never contain shell syntax.

## History and rollback

Every mutating action is appended to `history.jsonl` with a monotonically
increasing ID, timestamp, action, status, packages and commands. Rollback reads
an entry and inverts it:

* `install` becomes `remove`.
* `remove` becomes `install`.
* `upgrade` is not reversible.

Rollback is best effort because package managers do not expose a universal
version snapshot.

## Safety

* No shell: every command is `Command::new(bin).args([...])`.
* Names are validated (`[A-Za-z0-9@/_.:+-~^=[]]`, no leading `-`, no whitespace
  or control characters).
* `sudo` is only added on Unix, only for backends that need it, and never in
  dry-run. If `sudo` is missing, the command fails with exit code 3.
* `--no-elevate` refuses to add `sudo` at all.
* `--keep-going` collects failures and returns a non-zero summary.
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

## Capabilities

`capabilities` is the single source of truth for what each backend can do.
Commands that need an unsupported feature fail with exit code 6 or report the
limitation before doing partial work.

## Non-goals

* Hosting packages or maintaining a binary repository.
* Reimplementing dependency resolution.
* Downloading and executing install scripts.
* Restoring exact package versions.
* GUI or TUI (the CLI is scriptable; a TUI could be added later).
