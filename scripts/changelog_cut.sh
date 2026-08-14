#!/bin/bash
# Fold changelog.d/ fragments into CHANGELOG.md as a released version section.
#
# Run this BEFORE `make bump`, and commit the result. `build.rs` counts what is
# still uncut and `omni doctor` prints it, so a tag cut over unfolded entries
# ships a binary that accuses itself (#137).
#
# Usage: scripts/changelog_cut.sh 0.7.5
set -euo pipefail

NEW="${1:-}"
if [ -z "$NEW" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.7.5"
    exit 1
fi

if ! echo "$NEW" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
    echo "Error: version must be X.Y.Z or X.Y.Z-prerelease (got: $NEW)"
    exit 1
fi

if ! grep -q '^## \[Unreleased\]$' CHANGELOG.md; then
    echo "Error: CHANGELOG.md has no '## [Unreleased]' heading to cut."
    exit 1
fi

if grep -q "^## \[$NEW\]" CHANGELOG.md; then
    echo "Error: CHANGELOG.md already has a section for $NEW."
    exit 1
fi

DATE=$(date +%Y-%m-%d)
BLOCK=$(mktemp)
FOLDED=$(mktemp)
trap 'rm -f "$BLOCK" "$FOLDED"' EXIT

{
    echo "## [$NEW] - $DATE"
    echo
} > "$BLOCK"

# Keep a Changelog's order, not alphabetical. A section with no fragments emits
# no heading, so an empty `### Removed` never appears.
for section in Added Changed Fixed Removed; do
    lower=$(echo "$section" | tr '[:upper:]' '[:lower:]')
    files=$(ls changelog.d/*."$lower".md 2>/dev/null || true)
    [ -z "$files" ] && continue
    echo "### $section" >> "$BLOCK"
    for f in $files; do
        cat "$f" >> "$BLOCK"
        echo "$f" >> "$FOLDED"
    done
    echo >> "$BLOCK"
done

# `$(cat)` drops trailing newlines, so the carried-over section below does not
# arrive after a double blank line.
printf '%s\n' "$(cat "$BLOCK")" > "$BLOCK.trim"
mv "$BLOCK.trim" "$BLOCK"

# Bullets still written directly into the section, from before fragments existed
# or from a branch that missed the convention. They are carried, not lost, but
# they arrive under their own `### Added` / `### Fixed` heading and can land
# beside one the fragments just wrote.
CARRIED=$(sed -n '/^## \[Unreleased\]$/,/^## \[[0-9]/p' CHANGELOG.md | grep -c '^- ' || true)

# One insertion point: the `## [Unreleased]` heading becomes an empty
# `## [Unreleased]` followed by the new version section. Whatever was already
# written under the old heading stays where it is, which now puts it under the
# version heading, below the folded fragments.
awk -v blockfile="$BLOCK" '
  /^## \[Unreleased\]$/ && !done {
    print "## [Unreleased]"
    print ""
    while ((getline line < blockfile) > 0) print line
    done = 1
    next
  }
  { print }
' CHANGELOG.md > CHANGELOG.md.new
mv CHANGELOG.md.new CHANGELOG.md

if [ -s "$FOLDED" ]; then
    while read -r f; do rm -f "$f"; done < "$FOLDED"
    echo "Folded $(wc -l < "$FOLDED" | tr -d ' ') fragment(s) into ## [$NEW] - $DATE"
else
    echo "No fragments in changelog.d/. Cut ## [$NEW] - $DATE from the existing section."
fi

if [ "$CARRIED" -gt 0 ]; then
    echo "Note: $CARRIED bullet(s) were written into the section directly, not as"
    echo "      fragments. They are carried under ## [$NEW] with their own headings,"
    echo "      so you may need to merge two '### Added' or '### Fixed' by hand once."
fi

echo "Review CHANGELOG.md, commit it, then run: make bump VERSION=$NEW"
