---
description: how to release a new version of OMNI (Git, GitHub, and Homebrew Tap)
---

# Release Workflow

// turbo-all

## Steps

## Steps

1. **Determine the new version** (e.g., `0.5.1`).

2. **Run the release bump**:
   ```bash
   make bump VERSION=0.5.1
   ```

   This updates `Cargo.toml`, builds, and commits the version bump.

3. **Run the release validation and tagging**:
   ```bash
   make release VERSION=0.5.1
   ```

   This will automatically:
   - Run `fmt`, `clippy`, and `tests`
   - Build a production binary and check size (< 5MB)
   - Run smoke tests
   - Create and push the git tag `v0.5.1`

4. **Wait for GitHub Actions** to build 4 cross-platform binaries and create the GitHub Release. (Monitor: https://github.com/fajarhide/omni/actions)

5. **Update Homebrew formula**:
   ```bash
   make release-sha VERSION=0.5.1
   ```
   This fetches the new SHA256 hashes and updates `omni.rb`.

6. **Verify and Push Formula**:
   ```bash
   git add omni.rb
   git commit -m "chore: update formula for v0.5.1"
   git push
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