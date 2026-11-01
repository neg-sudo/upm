# Changelog

All notable changes to `upm` are documented here. This project follows
[Semantic Versioning](https://semver.org/).

## [1.0.0] — 2026-11-01

First stable release.

### Added

- Unified search with fuzzy ranking, `--exact`, `--installed`, `--outdated`,
  `--limit` and `--interactive` result selection.
- `outdated` with security filtering, plus an upgrade dashboard and interactive
  or security-only upgrades.
- `explain`, `capabilities`, `why` and `depends` for understanding resolution and
  backend support before acting.
- `clean`, `pin`, `unpin` and `pins` across the backends that support them.
- Transaction `plan`/`apply` with schema-versioned JSON plan files.
- Local transaction history, `history show/export` and best-effort `rollback`.
- Environment manifests: `init`, `export` and `sync`, with platform-specific
  package sections.
- Reusable package profiles: create, add/remove, install, show, export/import.
- Environment snapshots: create, list, show, delete, export, diff and restore.
- Safer execution controls: `--keep-going`, `--non-interactive` and
  `--no-elevate`.
- Expanded `doctor` with container detection and `--fix` suggestions.
- Stable, schema-versioned JSON output for the new read and write commands.
- Release workflow now builds Linux, macOS and Windows artifacts and publishes
  `SHA256SUMS` for the installers.

### Changed

- Improved backend command coverage for listing, outdated detection, cleanup,
  pinning and dependency inspection.
- Richer error hints for missing tools, privilege failures and unsupported
  operations.
- Documentation expanded for all v1.0 workflows.

### Notes

- Rollback reverses actions, not exact versions.
- Security-only upgrades require DNF or zypper.
- Some backends still lack pinning or dependency queries; `upm capabilities`
  reports the matrix.

## [0.1.0] — 2026-10-31

Initial release.

### Added

- Backends: Homebrew, APT, DNF, pacman, zypper, winget, Scoop, Chocolatey.
- Commands: `install`, `remove`, `upgrade`, `update`, `search`, `list`, `info`,
  `which`, `doctor`, `resolve`, `config`, `completions`.
- OS-aware backend detection with configurable priority and enable/disable.
- Embedded alias registry covering common developer tools, with user overrides.
- Name validation, argv-only execution, explicit elevation, dry-run and
  confirmation prompts.
- Installed-package probes so `install` is idempotent.
- `--json` output, categorized exit codes, colour handling and completions.
- Linux/macOS and Windows installers, CI across three operating systems, and
  tag-driven release builds.
