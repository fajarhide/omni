#!/bin/bash
# OMNI — Universal Install Script
# Usage: curl -fsSL https://raw.githubusercontent.com/fajarhide/omni/main/scripts/install.sh | sh
#
# Installs the latest OMNI binary to ~/.local/bin/omni
# Supports: macOS (arm64, x86_64), Linux (arm64, x86_64)

set -euo pipefail

REPO="fajarhide/omni"
INSTALL_DIR="${OMNI_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${OMNI_VERSION:-latest}"

# ─── Colors ──────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[omni]${NC} $*"; }
ok()    { echo -e "${GREEN}[omni]${NC} $*"; }
warn()  { echo -e "${YELLOW}[omni]${NC} $*"; }
error() { echo -e "${RED}[omni]${NC} $*" >&2; exit 1; }

# ─── Platform Detection ─────────────────────────────────
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Darwin) os="apple-darwin" ;;
        Linux)  os="unknown-linux-musl" ;;
        *)      error "Unsupported OS: $(uname -s). OMNI supports macOS and Linux." ;;
    esac

    case "$(uname -m)" in
        arm64|aarch64) arch="aarch64" ;;
        x86_64|amd64)  arch="x86_64" ;;
        *)             error "Unsupported architecture: $(uname -m). OMNI supports arm64 and x86_64." ;;
    esac

    echo "${arch}-${os}"
}

# ─── Version Resolution ─────────────────────────────────
resolve_version() {
    if [ "$VERSION" = "latest" ]; then
        VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
            | grep '"tag_name"' | sed 's/.*"tag_name": *"//;s/".*//')
        if [ -z "$VERSION" ]; then
            error "Failed to fetch latest version from GitHub."
        fi
    fi
    echo "$VERSION"
}

# ─── Download & Install ─────────────────────────────────
download_and_install() {
    local platform="$1"
    local version="$2"
    local url="https://github.com/$REPO/releases/download/${version}/omni-${version}-${platform}.tar.gz"
    local tmpdir
    tmpdir=$(mktemp -d)

    info "Downloading omni ${version} for ${platform}..."
    if ! curl -fsSL "$url" -o "$tmpdir/omni.tar.gz"; then
        error "Download failed. Check that version ${version} exists at:\n  $url"
    fi

    # Verify SHA-256 if available
    local sha_url="${url}.sha256"
    if curl -fsSL "$sha_url" -o "$tmpdir/omni.tar.gz.sha256" 2>/dev/null; then
        info "Verifying SHA-256 checksum..."
        local expected actual
        expected=$(awk '{print $1}' "$tmpdir/omni.tar.gz.sha256")
        actual=$(shasum -a 256 "$tmpdir/omni.tar.gz" | awk '{print $1}')
        if [ "$expected" != "$actual" ]; then
            error "SHA-256 mismatch!\n  Expected: $expected\n  Got:      $actual"
        fi
        ok "Checksum verified ✓"
    fi

    # Extract
    tar xzf "$tmpdir/omni.tar.gz" -C "$tmpdir"

    # Install
    mkdir -p "$INSTALL_DIR"
    cp "$tmpdir/omni" "$INSTALL_DIR/omni"
    chmod +x "$INSTALL_DIR/omni"

    # Cleanup
    rm -rf "$tmpdir"
}

# ─── PATH Check ──────────────────────────────────────────
check_path() {
    if ! echo "$PATH" | tr ':' '\n' | grep -q "$INSTALL_DIR"; then
        warn "$INSTALL_DIR is not in your PATH."
        echo ""
        echo "  Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        echo ""
        echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
        echo ""
    fi
}

# ─── Main ────────────────────────────────────────────────
main() {
    echo ""
    echo "  ┌─────────────────────────────────────┐"
    echo "  │  OMNI Installer                     │"
    echo "  │  Less noise. More signal.           │"
    echo "  └─────────────────────────────────────┘"
    echo ""

    local platform version
    platform=$(detect_platform)
    version=$(resolve_version)

    info "Platform: $platform"
    info "Version:  $version"
    info "Target:   $INSTALL_DIR/omni"
    echo ""

    download_and_install "$platform" "$version"

    ok "OMNI installed at $INSTALL_DIR/omni ✓"
    echo ""

    # Verify
    if "$INSTALL_DIR/omni" version >/dev/null 2>&1; then
        ok "Verified: $($INSTALL_DIR/omni version)"
    fi

    check_path

    echo ""
    echo "  Next steps:"
    echo "    omni init --hook   # Setup Claude Code hooks"
    echo "    omni doctor        # Verify installation"
    echo "    omni stats         # View token savings"
    echo ""
}

main "$@"
