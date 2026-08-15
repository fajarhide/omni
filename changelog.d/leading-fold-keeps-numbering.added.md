- **A `Read` that repeats the head of a file folds again, and the host's line
  numbers stay true.** #557 stopped a fold renumbering the lines under it by
  refusing the fold. That is correct, and it costs the most common shape there
  is: re-reading a file at a later offset repeats its head, so the run the ledger
  can fold is exactly the run with content below it. The host renders
  `file.content` with `cat -n` numbering counted from `startLine`, so moving
  `startLine` by the number of marker lines now standing above the survivors puts
  every one of them back where the file has it. Verified on live host transcripts
  before being relied on rather than assumed, which is the #158 lesson: a `Read`
  requested at offset 215 comes back with `215` on its first line.

  The rule is exact rather than clever. The survivors have to be one block
  running to the end of the payload, which makes the lines above them a
  subtraction and means nothing has to be searched for. Everything else refuses,
  including shapes that are correctable in principle, because the alternatives
  are searching the view for the survivors' text, which can match inside a
  marker, or a marker count nothing reports. A refused fold costs bytes; a wrong
  number costs an edit in the wrong place. The shape comes from the ledger's own
  folded indices, so a file whose lines begin with `[OMNI:` cannot change the
  answer.

  Measured on four overlapping windows of a markdown file under the host's output
  cap: **0.0% saved before, 4.7% after**, one fold where there had been none.
  Source files are unaffected, since the `readfile` distiller acts on those first
  (46.6% either way).
