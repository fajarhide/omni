# OMNI demo — "without OMNI" half. Renders media/demo-git-without.gif.
# Run from repo root:  vhs media/demo-git-without.tape
# Shows a raw `git log` — the wall of Author/Date/body lines an agent pays to read.
# Pairs with media/demo-git-with.tape; the README labels the two.

Output media/demo-git-without.gif

Set Shell bash
Set FontSize 20
Set Width 950
Set Height 720
Set Padding 24
Set Theme "Dracula"
Set TypingSpeed 35ms

Hide
Type `export PS1="❯ "`
Enter
Type "clear"
Enter
Show
Sleep 500ms

Type "git log -15"
Enter
Sleep 4500ms
