# Contributing

Thanks for helping make `upm` better.

## Ground rules

- Correctness and safety first. A backend must never guess silently when it can
  report an error.
- Keep commands as argv vectors. Never introduce a shell string.
- Show, do not surprise: mutating actions must remain visible and confirmable.
- Every backend command change needs a unit test.

## Setup

```bash
git clone https://github.com/zzzit0ti-cell/upm.git
cd upm
cargo build
cargo test --all
```

## Checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

## Adding a backend

1. Add a variant to `BackendKind` in `src/models.rs`.
2. Implement its command builders, probe and interpreter in `src/backend/mod.rs`.
3. Add it to `detect::priority()` for the relevant OS.
4. Seed aliases in `src/aliases.toml`.
5. Add unit tests for install/remove/upgrade/search/probe argv.
6. Update `README.md` and `docs/cli-reference.md`.

## Commits

Use imperative, scoped messages, for example:

```
backend: pass --needed to pacman installs
resolver: fall back to search when no alias exists
```

By contributing you agree that your changes are licensed under the MIT License.
