- **The landing page leads with what OMNI buys and the use-case page covers eleven
  situations instead of six.** No figure changed. The three largest measured ones, 97.2%
  off a repeated file read, 94% off `git log -15` and 92.9% off a `cargo test` run, were
  each buried inside a page and are now on the first screen beside the 4,940 bytes the
  lean MCP surface takes off every request. The aggregate and the 97.3% of calls that save
  nothing keep their own section, because a tool that claims to help everywhere is one
  nobody can predict. Five situations are new: reading one file in several passes,
  dispatching a subagent, following a marker back, a context that gets compacted, and the
  tool list every request carries. Both languages.
- **`Your first hour` teaches the two checks that make the rest worth trusting.** It now
  opens with what the hour buys, checking OMNI rather than trusting it, and adds the two
  commands that do it: pulling content back through a handle, and telling a real marker
  apart from one that is only text. Both were verified against a built binary before being
  written down, exit 1 with `the documentation example` for the reserved handle and exit 0
  with the content for a real one. The `omni stats` section also says its absolute figures
  are bytes now and why they used to be something else. Both languages.
- **`Seeing what it saved` says what its numbers are counted in, and names a breaking
  change.** Every absolute figure is bytes now, and the page explains why they used to be
  something else and why the percentages never moved with them. It also documents that
  `omni stats --json` renamed `commands[].tokens_saved` to `bytes_saved`: the field held
  bytes under the old name for one release, and a machine-readable surface asserting the
  wrong unit is the defect rather than a cosmetic slip.
- **`What OMNI is` covers the second thing OMNI edits: itself.** Tool definitions sit in
  the prefix of every request, so a prefix byte is carried from the first request while a
  removed output byte was inserted in the middle. Sixteen of twenty-five advertised tools
  had never been called across 229 sessions, and they weighed 4,940 bytes against the
  4,942 the distillers remove from output in a busy session. Both languages throughout.
