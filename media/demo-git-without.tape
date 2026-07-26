# OMNI demo — "without OMNI" half. Renders media/demo-git-without.gif.
# Run from repo root:  vhs media/demo-git-without.tape
# Same `git log -15` as demo-git-with.tape, raw — a screen of Author/Date/body
# noise. Matched dimensions so the two sit side by side in the README.

Output media/demo-git-without.gif

Set Shell bash
Set FontSize 18
Set Width 760
Set Height 560
Set Padding 20
Set Theme "Dracula"
Set TypingSpeed 30ms

Hide
Type `export PS1="❯ "`
Enter
Type "clear"
Enter
Show
Sleep 400ms

Type "git log -15"
Enter
Sleep 4000ms
