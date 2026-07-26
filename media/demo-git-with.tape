# OMNI demo — "with OMNI" half. Renders media/demo-git-with.gif.
# Run from repo root:  cargo build --release && vhs media/demo-git-with.tape
# The tape puts ./target/release on PATH so the GIF shows the current build.
# Same `git log -15` as demo-git-without.tape, through OMNI: every commit kept as a
# compact `hash subject` line, with a footer computed from the REAL byte counts.

Output media/demo-git-with.gif

Set Shell bash
Set FontSize 20
Set Width 950
Set Height 720
Set Padding 24
Set Theme "Dracula"
Set TypingSpeed 35ms

# Setup (hidden): current build on PATH, quiet mode, clean prompt, the RAW size of
# the same log, and a `bar` footer helper (tee to a file, measure exact bytes). The
# trailing `clear` wipes all of it before recording resumes.
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

Type `git log -15 | OMNI_CMD="git log -15" omni 2>/dev/null | bar`
Enter
Sleep 4500ms
