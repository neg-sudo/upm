# Changelog

All notable changes to `upm` are documented here. This project follows
[Semantic Versioning](https://semver.org/).

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
