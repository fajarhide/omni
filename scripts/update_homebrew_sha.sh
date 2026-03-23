#!/bin/bash
# update_homebrew_sha.sh — Update SHA-256 values in omni.rb after a GitHub Release
# Usage: ./scripts/update_homebrew_sha.sh [v0.5.0]
#
# If no version is given, reads it from Cargo.toml.

set -euo pipefail

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
    VERSION="v$(grep '^version' Cargo.toml | head -1 | sed 's/version = "//;s/"//')"
fi

# Strip leading 'v' for consistency, then re-add
VERSION="${VERSION#v}"
BASE_URL="https://github.com/fajarhide/omni/releases/download/v${VERSION}"

echo "Fetching SHA-256 for omni v${VERSION}..."

fetch_sha() {
    local target="$1"
    local url="${BASE_URL}/omni-v${VERSION}-${target}.tar.gz.sha256"
    local sha
    sha=$(curl -fsSL "$url" 2>/dev/null | awk '{print $1}')
    if [ -z "$sha" ]; then
        echo "ERROR: Could not fetch SHA for ${target}" >&2
        echo "  URL: ${url}" >&2
        return 1
    fi
    echo "$sha"
}

SHA_AARCH64_MACOS=$(fetch_sha "aarch64-apple-darwin")
SHA_X86_MACOS=$(fetch_sha "x86_64-apple-darwin")
SHA_AARCH64_LINUX=$(fetch_sha "aarch64-unknown-linux-musl")
SHA_X86_LINUX=$(fetch_sha "x86_64-unknown-linux-musl")

echo "Updating omni.rb..."

# macOS-compatible sed (uses .bak then removes it)
sed -i.bak \
    -e "s/PLACEHOLDER_AARCH64_MACOS/${SHA_AARCH64_MACOS}/" \
    -e "s/PLACEHOLDER_X86_64_MACOS/${SHA_X86_MACOS}/" \
    -e "s/PLACEHOLDER_AARCH64_LINUX/${SHA_AARCH64_LINUX}/" \
    -e "s/PLACEHOLDER_X86_64_LINUX/${SHA_X86_LINUX}/" \
    omni.rb
rm -f omni.rb.bak

# Also update the version line if it differs
CURRENT_VERSION=$(grep '  version ' omni.rb | sed 's/.*"\(.*\)".*/\1/')
if [ "$CURRENT_VERSION" != "$VERSION" ]; then
    sed -i.bak "s/version \"${CURRENT_VERSION}\"/version \"${VERSION}\"/" omni.rb
    rm -f omni.rb.bak
    echo "  Version updated: ${CURRENT_VERSION} → ${VERSION}"
fi

echo ""
echo "✓ omni.rb updated with real SHA-256 values"
echo "  AARCH64_MACOS: ${SHA_AARCH64_MACOS}"
echo "  X86_64_MACOS:  ${SHA_X86_MACOS}"
echo "  AARCH64_LINUX: ${SHA_AARCH64_LINUX}"
echo "  X86_64_LINUX:  ${SHA_X86_LINUX}"
echo ""
echo "Next: commit omni.rb and push to your homebrew tap."
