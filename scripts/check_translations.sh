#!/usr/bin/env bash
# Fail when an English manual page changes and its Indonesian counterpart does
# not (#539).
#
# A translation nobody is told to update is a translation that quietly starts
# lying, and the reader has no way to tell how far behind it is. This does not
# ask for the prose to be translated in the same commit: touching the file is
# enough, whether that means translating the change, or adding a line saying the
# page is behind. What it stops is the change landing with nobody having looked.
#
# Pages with no Indonesian counterpart are skipped, not flagged, which is what
# keeps `develop/` deliberately untranslated rather than permanently red.
#
#   scripts/check_translations.sh [base-ref]
#
# base-ref defaults to origin/main. Compares the merge base, so a stale branch is
# judged on what it changed rather than on what main did meanwhile.
set -euo pipefail

EN="docs/website/src"
ID="docs/website/src-id"
base="${1:-origin/main}"

git rev-parse --verify --quiet "$base" >/dev/null || {
  echo "check_translations: no such ref: $base" >&2
  exit 2
}

changed="$(git diff --name-only "$base"...HEAD -- "$EN" "$ID")"
[ -n "$changed" ] || { echo "check_translations: no manual pages changed"; exit 0; }

stale=""
while IFS= read -r file; do
  case "$file" in "$EN"/*.md) ;; *) continue ;; esac
  counterpart="$ID/${file#"$EN"/}"
  [ -f "$counterpart" ] || continue
  printf '%s\n' "$changed" | grep -qxF "$counterpart" || stale="$stale  $file -> $counterpart"$'\n'
done <<< "$changed"

if [ -n "$stale" ]; then
  echo "check_translations: English pages changed without their Indonesian counterpart:" >&2
  printf '%s' "$stale" >&2
  echo >&2
  echo "Update the page on the right, or note in it that it is behind." >&2
  exit 1
fi

echo "check_translations: every changed page has its counterpart in this diff"
