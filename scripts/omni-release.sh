#!/bin/bash
# omni-release.sh — Pre-release validation and tag creation (Rust Edition)
# Usage: ./scripts/omni-release.sh 0.5.0

set -euo pipefail

VERSION="${1:?Usage: omni-release.sh <version>}"
TAG="v${VERSION}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

ok()   { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC} $*"; }
fail() { echo -e "${RED}✗${NC} $*"; exit 1; }
info() { echo -e "${CYAN}▸${NC} $*"; }

echo "═══════════════════════════════════════"
echo " OMNI Release: ${TAG}"
echo "═══════════════════════════════════════"

# ─── Pre-flight checks ──────────────────

echo ""
info "Pre-flight checks"

# 1. Verify version in Cargo.toml
CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
if [ "$CARGO_VERSION" != "$VERSION" ]; then
    fail "Cargo.toml version ($CARGO_VERSION) ≠ release version ($VERSION)\n  Fix: edit Cargo.toml version field, or run: ./scripts/bump_version.sh $VERSION"
fi
ok "Cargo.toml version: $CARGO_VERSION"

# 2. Git status check
if ! git diff --quiet HEAD; then
    fail "Working directory is not clean. Commit or stash changes first."
fi
ok "Git working directory: clean"

# 3. Branch check
BRANCH=$(git branch --show-current)
if [ "$BRANCH" != "main" ] && [ "$BRANCH" != "homebrew_fix" ]; then
    warn "Not on main branch (on: $BRANCH). Continue? [y/N]"
    read -r confirm
    [ "$confirm" = "y" ] || exit 1
fi
ok "Branch: $BRANCH"

# 4. Tag check
if git tag --list | grep -q "^${TAG}$"; then
    fail "Tag ${TAG} already exists"
fi
ok "Tag ${TAG}: not yet created"

# ─── Build validation ────────────────────

echo ""
info "Build validation"

# 5. cargo fmt check
cargo fmt --check || fail "cargo fmt check failed. Run: cargo fmt"
ok "cargo fmt: clean"

# 6. cargo clippy
echo "   Running clippy..."
cargo clippy --all-targets -- -D warnings > /tmp/omni-clippy 2>&1 || { cat /tmp/omni-clippy && fail "clippy warnings found"; }
ok "cargo clippy: no warnings"

# 7. cargo test
echo "   Running tests..."
cargo test --all > /tmp/omni-test 2>&1 || { tail -n 20 /tmp/omni-test && fail "tests failed"; }
ok "cargo test: all pass"

# 8. Release build
echo "   Building release..."
cargo build --release > /tmp/omni-build 2>&1 || { tail -n 20 /tmp/omni-build && fail "release build failed"; }
BINARY_SIZE=$(du -k target/release/omni | cut -f1)
ok "Release build: ${BINARY_SIZE}KB"
if [ "$BINARY_SIZE" -gt 5120 ]; then
    warn "Binary size ${BINARY_SIZE}KB exceeds 5MB target"
fi

# 9. Smoke test
if [ -x tests/smoke_test.sh ]; then
    echo "   Running smoke tests..."
    bash tests/smoke_test.sh ./target/release/omni > /tmp/omni-smoke 2>&1 || { cat /tmp/omni-smoke && fail "smoke test failed"; }
    ok "Smoke test: all pass"
fi

# ─── Confirm and tag ────────────────────

echo ""
echo "═══════════════════════════════════════"
echo " All checks passed! Ready to release ${TAG}"
echo "═══════════════════════════════════════"
echo ""
echo " This will:"
echo "  1. Create git tag ${TAG}"
echo "  2. Push tag to origin (triggers GitHub Actions release)"
echo "  3. GitHub Actions will build 4 platform binaries"
echo "  4. GitHub Release will be created automatically"
echo ""
echo " After release completes (~10 min):"
echo "  5. Run: ./scripts/update_homebrew_sha.sh ${VERSION}"
echo "  6. Update Homebrew tap with newest omni.rb"
echo ""
read -r -p "Proceed with release? [y/N] " confirm
[ "$confirm" = "y" ] || { echo "Aborted."; exit 0; }

git tag -a "${TAG}" -m "Release ${TAG}"
git push origin "${BRANCH}" "${TAG}"

echo ""
ok "Tag ${TAG} pushed! Monitor: https://github.com/fajarhide/omni/actions"
echo "   GitHub Actions will create the release in ~10 minutes."
