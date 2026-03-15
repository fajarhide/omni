#!/bin/bash
# OMNI Release Master 🚀
# Automates: Tagging, Pushing, SHA256 Calculation, and Formula Update

set -e

if [ -z "$1" ]; then
    echo "Usage: ./scripts/omni-release.sh <version>"
    echo "Example: ./scripts/omni-release.sh 0.2.1"
    exit 1
fi

VERSION=$1
TAG="v$VERSION"
REPO="fajarhide/omni"
TAP_REPO_PATH="../homebrew-omni" # Default assumption

echo "🌌 Preparing OMNI $VERSION release..."

# 1. Update Homebrew Formula URL
sed -i '' "s|tags/v.*.tar.gz|tags/$TAG.tar.gz|g" omni.rb

# 2. Update Dynamic Versioning in Scripts or Code if needed
# (Already handled by build.zig -Dversion in deployment)

# 3. Commit and Tag
git add .
git commit -m "chore: bump version to $VERSION" || echo "No changes to commit"
git push origin main

# 4. Create/Update Tag
echo "🏷  Tagging $TAG..."
git tag -f "$TAG"
git push -f origin "$TAG"

# 5. Calculate New SHA256
echo "📥 Fetching archive to calculate SHA256..."
sleep 3 # Wait for GitHub to process the tag
TARBALL_URL="https://github.com/$REPO/archive/refs/tags/$TAG.tar.gz"
TEMP_TARBALL="/tmp/omni-$VERSION.tar.gz"
curl -L "$TARBALL_URL" -o "$TEMP_TARBALL"
NEW_SHA=$(shasum -a 256 "$TEMP_TARBALL" | awk '{print $1}')

echo "✨ New SHA256: $NEW_SHA"

# 6. Update local omni.rb with new SHA
sed -i '' "s|sha256 \".*\"|sha256 \"$NEW_SHA\"|g" omni.rb

# 7. Sync with Tap if exists
if [ -d "$TAP_REPO_PATH" ]; then
    echo "🔄 Syncing with Homebrew Tap at $TAP_REPO_PATH..."
    cp omni.rb "$TAP_REPO_PATH/omni.rb"
    (cd "$TAP_REPO_PATH" && git add omni.rb && git commit -m "update omni to $TAG" && git push origin main)
    echo "✅ Tap updated!"
else
    echo "⚠️  Tap repository not found at $TAP_REPO_PATH. Please manual update it with SHA: $NEW_SHA"
fi

# 8. Final Sync for local omni.rb
git add omni.rb
git commit -m "chore: update formula SHA256 for $VERSION"
git push origin main

echo "🚀 OMNI $VERSION is live!"
echo "Check: https://github.com/$REPO/releases"
