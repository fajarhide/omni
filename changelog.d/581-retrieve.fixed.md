- **Following a marker returned the marker.** A fold prints `omni retrieve <handle>`, and
  the bytes that came back passed through the hook, hashed the same, and were folded into
  the very marker that had sent the reader to fetch them. An agent that did what the
  marker said got the instruction again, and `OMNI_PASSTHROUGH=1` on the retrieve did not
  help because the fold lands on the later read rather than on that command. A pull now
  marks the archived row owed, and the delivery answering it is handed over verbatim. It
  costs one delivery and not an exemption: the next repeat folds again, which matters
  because 15.05% of the archive on this installation has been pulled at least once. (#581)
