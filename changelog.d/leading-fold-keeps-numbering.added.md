- **A `Read` that repeats the head of a file folds again, and the host's line
  numbers stay true.** #557 stopped a fold renumbering the lines under it by
  refusing the fold, which is correct and costs the most common shape there is:
  re-reading a file at a later offset repeats its head, so the run the ledger can
  fold is exactly the run that has content below it. The host renders
  `file.content` with `cat -n` numbering counted from `startLine`, so when the
  repeated run is at the head one number fixes all of it: move `startLine` by the
  size of the run minus the marker's own line and every survivor lands back where
  the file has it. Verified on live host transcripts before being relied on
  rather than assumed, which is the #158 lesson: a `Read` requested at offset 215
  comes back with `215` on its first line. The rule is about what survives rather
  than where the folds are: contiguous survivors all sit the same distance from
  where they started, so one number puts every one of them back, which covers a
  fold at the head and one reaching the end of the same payload. Survivors split
  into two blocks still refuse, because one starting number cannot describe two
  offsets, and
  the shape is read from the ledger's own folded indices so content that looks
  like a marker cannot change the answer. Measured on four overlapping windows of
  a markdown file under the host's output cap: **0.0% saved before, 4.7% after**,
  one fold where there had been none. Source files are unaffected, since the
  `readfile` distiller acts on those first (46.6% either way).
