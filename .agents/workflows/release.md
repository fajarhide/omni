---
description: how to release a new version of OMNI (Git, GitHub, and Homebrew Tap)
---

# Release Workflow

// turbo-all

## Steps

1. **Determine the new version** (e.g., `0.5.1`).

2. **Run the release script**:
   ```bash
   ./scripts/omni-release.sh 0.5.1
   ```

   This will automatically:
   - Update `Cargo.toml` version
   - Build release binary
   - Commit and push to main
   - Create/push git tag `v0.5.1`
   - Wait for GitHub Actions to build 4 cross-platform binaries
   - Fetch SHA256SUMS from the release
   - Update `omni.rb` formula with new SHA hashes
   - Sync with Homebrew tap

3. **Verify the release**:
   ```bash
   brew update
   brew upgrade omni
   omni version    # Should show new version
   omni doctor     # Full health check
   ```

4. **Check the GitHub Release page**:
   https://github.com/fajarhide/omni/releases

## Manual Version Bump (without release)

```bash
./scripts/bump_version.sh 0.5.1
```

This updates `Cargo.toml`, builds, and commits — but does NOT tag or release.

## Release Targets

| Target | Platform |
|---|---|
| `aarch64-apple-darwin` | macOS Apple Silicon |
| `x86_64-apple-darwin` | macOS Intel |
| `x86_64-unknown-linux-musl` | Linux x86_64 (static) |
| `aarch64-unknown-linux-musl` | Linux ARM64 (static) |