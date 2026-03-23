#!/bin/bash
# Usage: scripts/bump_version.sh 0.6.0
set -euo pipefail

NEW="${1:-}"
if [ -z "$NEW" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.6.0"
    exit 1
fi

# Validate version format
if ! echo "$NEW" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "Error: version must be in X.Y.Z format (got: $NEW)"
    exit 1
fi

echo "Bumping version to $NEW..."

# 1. Update Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$NEW\"/" Cargo.toml
rm -f Cargo.toml.bak

# 2. Update Cargo.lock
cargo check --quiet 2>/dev/null || true

# 3. Verify build
echo "Verifying build..."
cargo build --quiet

# 4. Verify version output
ACTUAL=$(./target/debug/omni version 2>&1)
if echo "$ACTUAL" | grep -q "$NEW"; then
    echo "✓ Version output: $ACTUAL"
else
    echo "⚠ Version output doesn't match: $ACTUAL (expected $NEW)"
    echo "  Note: version is read from Cargo.toml via env!(\"CARGO_PKG_VERSION\")"
fi

# 5. Stage and commit
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to $NEW"

echo ""
echo "Done! Version bumped to $NEW"
echo "Next steps:"
echo "  git push"
echo "  git tag v$NEW"
echo "  git push --tags  # triggers release workflow"
