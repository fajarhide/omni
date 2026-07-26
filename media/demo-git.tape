# OMNI demo — one before/after on the SAME `git log -15`. Renders media/demo-git.gif.
# Run from repo root:  cargo build --release && vhs media/demo-git.tape
# The tape puts ./target/release on PATH so the GIF shows the current build.
#
# Apples-to-apples: the same command throughout.
#   1. wc anchors the size — how many lines / bytes the raw log is.
#   2. the raw verbose log — a screen of Author/Date/body noise.
#   3. the same log through OMNI — every commit kept as one hash+subject line,
#      with a footer computed from the real byte counts.

Output media/demo-git.gif

Set Shell bash
Set FontSize 20
Set Width 1100
Set Height 640
Set Padding 24
Set Theme "Dracula"
Set TypingSpeed 32ms

Hide
Type `export PATH="$PWD/target/release:$PATH" OMNI_QUIET=1`
Enter
Type `export RAW=$(git log -15 | wc -c)`
Enter
Type `bar() { local f o; f=$(mktemp); tee "$f"; o=$(wc -c < "$f"); printf '\n\033[1;38;5;120m  ⏺ %s B -> %s B  ·  %s%% smaller, every commit kept\033[0m\n' "$RAW" "$o" "$((100*(RAW-o)/RAW))"; rm -f "$f"; }`
Enter
Type `export PS1="❯ "`
Enter
Type "clear"
Enter
Show
Sleep 500ms

# 1. anchor: how big is the raw log
Type "git log -15 | wc -lc"
Enter
Sleep 2s

# 2. the raw verbose log — a screenful of the noise you pay for
Type "git log -15 | head -40"
Enter
Sleep 3s

# 3. the same log through OMNI — clean screen, every commit kept, measured
Hide
Type "clear"
Enter
Show
Sleep 300ms
Type `git log -15 | OMNI_CMD="git log -15" omni 2>/dev/null | bar`
Enter
Sleep 4s
