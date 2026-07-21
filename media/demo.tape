# OMNI demo — renders media/demo.gif from REAL command output (nothing faked).
#
# Render it:   brew install vhs   &&   vhs media/demo.tape
# (vhs runs each command live and records the terminal, so the GIF shows exactly
#  what OMNI does. Re-run after any distiller change to keep it honest.)
#
# It pipes a real captured `cargo test` log (media/demo/cargo-test.txt, 378 lines
# of a green-ish run) through OMNI's pipe mode — instant, so the GIF stays snappy —
# then shows the honest `omni stats` dashboard. Swap the log/command for git, docker
# or kubectl if you like; anything OMNI recognises compresses the same way.

Output media/demo.gif

Set Shell bash
Set FontSize 20
Set Width 1180
Set Height 640
Set Padding 24
Set Theme "Dracula"
Set TypingSpeed 45ms

Hide
Type "export OMNI_QUIET=1; clear"
Enter
Sleep 500ms
Show

# ── Beat 1: the problem ──────────────────────────────────────────────
Type "# A cargo test run — 378 lines your agent pays to read:"
Enter
Sleep 700ms
Type "wc -l < media/demo/cargo-test.txt"
Enter
Sleep 2s

# ── Beat 2: the same output, through OMNI ────────────────────────────
Type '# Through OMNI it keeps the verdict, drops the noise:'
Enter
Sleep 700ms
Type 'cat media/demo/cargo-test.txt | OMNI_CMD="cargo test" omni'
Enter
Sleep 4s

# ── Beat 3: the honest dashboard ─────────────────────────────────────
Type "# And the savings are measured, not aspirational:"
Enter
Sleep 700ms
Type "omni stats"
Enter
Sleep 5s
