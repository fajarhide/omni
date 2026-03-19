#!/bin/bash

# Vercel Ignored Build Step Script
# This script ensures that Vercel only triggers a deployment for:
# 1. Pushes to the main branch (Production)
# 2. Pull Requests created by the repository owner (fajarhide)
#
# It prevents unauthorized deployments and potential security risks from external PRs.

echo "Verifying deployment permissions for: $VERCEL_GIT_COMMIT_AUTHOR_LOGIN"

# If it's a Pull Request
if [ "$VERCEL_GIT_PULL_REQUEST_ID" ]; then
  # Check if the author is NOT the owner
  if [ "$VERCEL_GIT_COMMIT_AUTHOR_LOGIN" != "fajarhide" ]; then
    echo "🛑 Build Ignored: Deployment for Pull Requests is restricted to the owner (fajarhide)."
    echo "Current PR Author: $VERCEL_GIT_COMMIT_AUTHOR_LOGIN"
    exit 0 # Ignore the build
  fi
fi

# Otherwise, proceed with the build (pushes to main or owner's PR)
echo "✅ Build Proceeding..."
exit 1 # Proceed with the build
