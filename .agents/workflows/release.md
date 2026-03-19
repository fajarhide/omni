---
description: how to release a new version of OMNI (Git, GitHub, and Homebrew Tap)
---

Follow these steps to ensure a complete and consistent release of OMNI:

1. **Verify Build & Stability**:
   Ensure everything is clean and working locally.
   ```bash
   cd core && zig build -Doptimize=ReleaseFast -p ../ && ../bin/omni monitor
   ```

2. **Update Version**:
   Determine the new version (e.g., `0.3.1`). Update it in `core/build.zig.zon`.
   ```bash
   # In core/build.zig.zon
   .version = "0.3.1",
   ```

3. **Run Release Script**:
   Use the master release script to handle tagging and local formula updates.
   // turbo
   ```bash
   ./scripts/omni-release.sh <version>
   ```
   *Note: If this script warns "Tap repository not found", proceed to step 4.*

4. **Sync Homebrew Tap (If Manual Update Needed)**:
   Ensure the `fajarhide/omni` tap is updated with the new version and SHA256.
   - Locate the tap path: `brew --repository fajarhide/omni`
   - Update `omni.rb` in the tap repository.
   - Commit and push the tap update.

5. **Verify GitHub Release**:
   Check that the GitHub Action has created the release entry and uploaded assets.
   ```bash
   gh release list
   ```

6. **Post-Release Check**:
   Wait a few minutes and verify the installation:
   ```bash
   brew update && brew upgrade fajarhide/tap/omni && omni version
   ```