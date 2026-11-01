#!/usr/bin/env bash
# upm installer — Linux and macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/neg-sudo/upm/main/install.sh | bash
# Env: UPM_VERSION, UPM_INSTALL_DIR, UPM_TARGET

set -euo pipefail

REPO="neg-sudo/upm"
VERSION="${UPM_VERSION:-latest}"
INSTALL_DIR="${UPM_INSTALL_DIR:-$HOME/.local/bin}"

say()  { printf '\033[96mℹ\033[0m %s\n' "$*"; }
ok()   { printf '\033[92m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[93m⚠\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[91m✗\033[0m %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"; }
need curl
need tar

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)  os_part="unknown-linux-gnu" ;;
  Darwin) os_part="apple-darwin" ;;
  *) die "unsupported operating system: $os (use install.ps1 on Windows)" ;;
esac

case "$arch" in
  x86_64|amd64) arch_part="x86_64" ;;
  arm64|aarch64) arch_part="aarch64" ;;
  *) die "unsupported architecture: $arch" ;;
esac

TARGET="${UPM_TARGET:-${arch_part}-${os_part}}"
ASSET="upm-${TARGET}.tar.gz"

if [ "$VERSION" = "latest" ]; then
  BASE="https://github.com/${REPO}/releases/latest/download"
else
  case "$VERSION" in
    v*) TAG="$VERSION" ;;
    *)  TAG="v${VERSION}" ;;
  esac
  BASE="https://github.com/${REPO}/releases/download/${TAG}"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "Installing upm ${VERSION} for ${TARGET}"

if curl -fsSL --retry 3 -o "${tmp}/${ASSET}" "${BASE}/${ASSET}"; then
  if curl -fsSL --retry 3 -o "${tmp}/SHA256SUMS" "${BASE}/SHA256SUMS" 2>/dev/null; then
    expected="$(grep " ${ASSET}\$" "${tmp}/SHA256SUMS" | awk '{print $1}' || true)"
    if [ -n "$expected" ]; then
      if command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "${tmp}/${ASSET}" | awk '{print $1}')"
      elif command -v shasum >/dev/null 2>&1; then
        actual="$(shasum -a 256 "${tmp}/${ASSET}" | awk '{print $1}')"
      else
        actual=""
      fi
      if [ -n "$actual" ] && [ "$actual" != "$expected" ]; then
        die "checksum mismatch for ${ASSET}"
      fi
      [ -n "$actual" ] && ok "checksum verified"
    fi
  fi

  tar -xzf "${tmp}/${ASSET}" -C "$tmp"
  bin="$(find "$tmp" -maxdepth 2 -type f -name upm | head -n1)"
  [ -n "$bin" ] || die "archive did not contain the upm binary"

  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$bin" "${INSTALL_DIR}/upm"
  ok "installed ${INSTALL_DIR}/upm"
else
  warn "could not download a prebuilt binary"
  if command -v cargo >/dev/null 2>&1; then
    say "building from source with cargo"
    cargo install --git "https://github.com/${REPO}" --locked --root "$HOME/.upm"
    mkdir -p "$INSTALL_DIR"
    install -m 0755 "$HOME/.upm/bin/upm" "${INSTALL_DIR}/upm"
    ok "installed ${INSTALL_DIR}/upm"
  else
    die "no prebuilt binary for ${TARGET} and cargo is not installed"
  fi
fi

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    warn "${INSTALL_DIR} is not on your PATH"
    printf '  Add this to your shell profile:\n'
    printf '    export PATH="%s:$PATH"\n' "$INSTALL_DIR"
    ;;
esac

printf '\n'
ok "upm is ready"
printf '  Run %s to get started\n' "$(printf '\033[1mupm doctor\033[0m')"
