#!/bin/bash
# OMNI Release Master (Rust Edition)
# Automates: Version bump, Tagging, SHA256 Calculation, and Formula Update
#
# Usage: ./scripts/omni-release.sh <version>
# Example: ./scripts/omni-release.sh 0.5.0

set -euo pipefail

if [ -z "${1:-}" ]; then
    echo "Usage: ./scripts/omni-release.sh <version>"
    echo "Example: ./scripts/omni-release.sh 0.5.0"
    exit 1
fi

VERSION="$1"
TAG="v$VERSION"
REPO="fajarhide/omni"
TAP_REPO_PATH="../homebrew-tap/Formula"

echo "═══════════════════════════════════════════"
echo " OMNI Release — $VERSION"
echo "═══════════════════════════════════════════"

# 1. Version bump
echo ""
echo "📦 Step 1: Bump version to $VERSION..."
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Verify build compiles
echo "   Building release..."
cargo build --release --quiet
echo "   ✓ Build successful"

# Verify version output
ACTUAL=$("./target/release/omni" version 2>&1)
if echo "$ACTUAL" | grep -q "$VERSION"; then
    echo "   ✓ Version verified: $ACTUAL"
else
    echo "   ⚠ Version mismatch: got '$ACTUAL', expected '$VERSION'"
fi

# 2. Update Homebrew Formula URLs
echo ""
echo "📦 Step 2: Update Homebrew formula..."
sed -i '' "s|version \".*\"|version \"$VERSION\"|" omni.rb

echo "   ✓ Formula version updated"

# 3. Commit and Tag
echo ""
echo "🏷  Step 3: Commit and tag..."
git add Cargo.toml Cargo.lock omni.rb
git commit -m "chore: release v$VERSION" || echo "   No changes to commit"
git push origin main

echo "   Creating tag $TAG..."
git tag -f "$TAG"
git push -f origin "$TAG"
echo "   ✓ Tag pushed"

# 4. Wait for GitHub Actions release build
echo ""
echo "⏳ Step 4: Waiting for GitHub Actions release build..."
echo "   The release.yml workflow will:"
echo "     - Build 4 targets (macOS arm64/x86, Linux musl arm64/x86)"
echo "     - Generate SHA256SUMS"
echo "     - Create GitHub Release"
echo ""
echo "   Monitor: https://github.com/$REPO/actions"
echo ""
echo "   Waiting 10s for release to start..."
sleep 10

# 5. Fetch SHA256 sums from release (retry loop)
echo ""
echo "📥 Step 5: Fetching SHA256 from release artifacts..."

MAX_RETRIES=30
RETRY_INTERVAL=10
SHA256_URL="https://github.com/$REPO/releases/download/$TAG/SHA256SUMS"

for i in $(seq 1 $MAX_RETRIES); do
    if curl -fsSL "$SHA256_URL" -o /tmp/omni-sha256sums 2>/dev/null; then
        echo "   ✓ SHA256SUMS downloaded"
        cat /tmp/omni-sha256sums
        break
    fi
    echo "   Attempt $i/$MAX_RETRIES — release not ready yet, retrying in ${RETRY_INTERVAL}s..."
    sleep $RETRY_INTERVAL
done

if [ ! -f /tmp/omni-sha256sums ]; then
    echo "   ⚠ Could not fetch SHA256SUMS automatically."
    echo "   Manually update omni.rb with SHA256 values from:"
    echo "   https://github.com/$REPO/releases/tag/$TAG"
    exit 0
fi

# 6. Update Formula SHA256 values
echo ""
echo "📝 Step 6: Updating formula SHA256 hashes..."

AARCH64_MACOS=$(grep "aarch64-apple-darwin" /tmp/omni-sha256sums | awk '{print $1}')
X86_64_MACOS=$(grep "x86_64-apple-darwin" /tmp/omni-sha256sums | awk '{print $1}')
AARCH64_LINUX=$(grep "aarch64-unknown-linux-musl" /tmp/omni-sha256sums | awk '{print $1}')
X86_64_LINUX=$(grep "x86_64-unknown-linux-musl" /tmp/omni-sha256sums | awk '{print $1}')

# Update SHA256 in formula (macOS arm64)
if [ -n "$AARCH64_MACOS" ]; then
    # Replace the first sha256 (after on_arm in on_macos)
    python3 -c "
import re
with open('omni.rb') as f: c = f.read()
shas = ['$AARCH64_MACOS', '$X86_64_MACOS', '$AARCH64_LINUX', '$X86_64_LINUX']
i = 0
def repl(m):
    global i
    s = 'sha256 \"' + shas[i] + '\"' if i < len(shas) else m.group(0)
    i += 1
    return s
c = re.sub(r'sha256 \"[A-Fa-f0-9_]+\"', repl, c)
with open('omni.rb', 'w') as f: f.write(c)
"
    echo "   ✓ SHA256 hashes updated in omni.rb"
fi

# 7. Commit SHA updates
git add omni.rb
git commit -m "chore: update formula SHA256 for v$VERSION" || echo "   No SHA changes"
git push origin main

# 8. Sync with Homebrew Tap
echo ""
echo "🍺 Step 7: Syncing with Homebrew Tap..."

BREW_TAP_PATH=$(brew --repository fajarhide/omni 2>/dev/null || echo "")

if [ -d "$TAP_REPO_PATH" ]; then
    echo "   Syncing with $TAP_REPO_PATH..."
    cp omni.rb "$TAP_REPO_PATH/omni.rb"
    (cd "$TAP_REPO_PATH" && git add omni.rb && git commit -m "update omni to $TAG" && git push origin main)
    echo "   ✓ Tap updated!"
elif [ -n "$BREW_TAP_PATH" ] && [ -d "$BREW_TAP_PATH" ]; then
    echo "   Syncing with $BREW_TAP_PATH..."
    cp omni.rb "$BREW_TAP_PATH/Formula/omni.rb"
    (cd "$BREW_TAP_PATH" && git add Formula/omni.rb && git commit -m "update omni to $TAG" && git push origin main)
    echo "   ✓ Tap updated!"
else
    echo "   ⚠ Tap not found. Manually copy omni.rb to your Homebrew tap."
fi

# Done
echo ""
echo "═══════════════════════════════════════════"
echo " ✅ OMNI $VERSION is live!"
echo "═══════════════════════════════════════════"
echo ""
echo "  Release:  https://github.com/$REPO/releases/tag/$TAG"
echo "  Install:  brew upgrade omni"
echo "  Verify:   omni doctor"
echo ""
