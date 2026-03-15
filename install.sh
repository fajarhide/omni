#!/bin/sh
# OMNI Installer - Semantic Distillation Engine
# https://github.com/fajarhide/omni
# Usage: curl -fsSL https://raw.githubusercontent.com/fajarhide/omni/main/install.sh | sh

set -e

REPO="fajarhide/omni"
INSTALL_DIR="${OMNI_INSTALL_DIR:-$HOME/.omni}"
TEMP_DIR=$(mktemp -d)

# Colors
BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info() { printf "${GREEN}[INFO]${NC} %s\n" "$1"; }
warn() { printf "${YELLOW}[WARN]${NC} %s\n" "$1"; }
error() { printf "${RED}[ERROR]${NC} %s\n" "$1"; exit 1; }

trap 'rm -rf "$TEMP_DIR"' EXIT

echo "${BLUE}🌌 Welcome to the OMNI Installer${NC}"
echo "════════════════════════════════════════════"

# 1. Dependency Check
info "Checking dependencies..."
if ! command -v zig >/dev/null 2>&1; then
    error "Zig 0.15.2+ is required. Please install it from ziglang.org."
fi

if ! command -v node >/dev/null 2>&1; then
    error "Node.js 18+ is required. Please install it from nodejs.org."
fi

if ! command -v curl >/dev/null 2>&1; then
    error "curl is required to download OMNI."
fi

# 2. Fetch Latest Release
info "Fetching latest release information..."
LATEST_TAG=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
    warn "Could not find latest release tag via API, falling back to main branch archive."
    DOWNLOAD_URL="https://github.com/${REPO}/archive/refs/heads/main.tar.gz"
else
    info "Found latest release: $LATEST_TAG"
    DOWNLOAD_URL="https://github.com/${REPO}/archive/refs/tags/${LATEST_TAG}.tar.gz"
fi

# 3. Download & Extract
info "Downloading OMNI source..."
curl -L "$DOWNLOAD_URL" -o "$TEMP_DIR/omni.tar.gz"
mkdir -p "$TEMP_DIR/src"
tar -xzf "$TEMP_DIR/omni.tar.gz" -C "$TEMP_DIR/src" --strip-components=1

cd "$TEMP_DIR/src"

# 4. Build
info "Building OMNI Native Core (Zig)..."
cd core
zig build -Doptimize=ReleaseFast -p "$TEMP_DIR/install_root"
zig build wasm -Doptimize=ReleaseSmall -p "$TEMP_DIR/install_root"
cd ..

info "Bundling OMNI MCP Server (Node.js)..."
# Create dist structure
mkdir -p "$TEMP_DIR/install_root/dist/core"
mv "$TEMP_DIR/install_root/bin/omni-wasm.wasm" "$TEMP_DIR/install_root/dist/core/"

# Install & Build Node project
npm install
npm run build
npm prune --omit=dev

# 5. Local Installation
info "Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR/bin"
mkdir -p "$INSTALL_DIR/dist"

# Move native binary
mv "$TEMP_DIR/install_root/bin/omni" "$INSTALL_DIR/bin/"

# Move MCP Server artifacts
cp -r dist/* "$INSTALL_DIR/dist/"
cp package.json "$INSTALL_DIR/dist/"
cp -r node_modules "$INSTALL_DIR/dist/"
cp -r "$TEMP_DIR/install_root/dist/core" "$INSTALL_DIR/dist/"

# 6. Post-Install Setup
info "Running post-install setup..."
"$INSTALL_DIR/bin/omni" setup

# 7. Success & Instructions
echo ""
echo "${GREEN}✅ OMNI successfully installed in $INSTALL_DIR${NC}"
echo "════════════════════════════════════════════"

# PATH check
if ! echo "$PATH" | grep -q "$INSTALL_DIR/bin"; then
    warn "OMNI bin directory is not in your PATH."
    SHELL_NAME=$(basename "$SHELL")
    PROFILE_FILE=""
    
    case "$SHELL_NAME" in
        zsh)  PROFILE_FILE="$HOME/.zshrc" ;;
        bash) PROFILE_FILE="$HOME/.bashrc" ;;
        *)    PROFILE_FILE="$HOME/.profile" ;;
    esac

    echo "To add OMNI to your PATH, run:"
    echo "${BLUE}echo 'export PATH=\"\$INSTALL_DIR/bin:\$PATH\"' >> $PROFILE_FILE${NC}"
    echo "Then restart your shell or run: ${BLUE}source $PROFILE_FILE${NC}"
    echo ""
fi

info "OMNI is mission-ready. 🌌"
info "Run 'omni --help' to get started."
