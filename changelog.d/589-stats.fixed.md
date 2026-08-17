- **`omni stats` reported in a unit OMNI cannot defend.** The per-period summary read
  `10K to 10K tokens`, and the per-command and per-filter annotations read `-N tokens`,
  all of them a byte count divided by 3.6, a constant calibrated against `cl100k_base`.
  Every one of those now reports the bytes `distillations` counts exactly, and the
  percentages beside them are unchanged because the divisor cancels in a ratio. The
  context breakdown is the one block that could not move: it accumulates `size_bytes / 4`
  from file metadata, so there is no measured total behind it, and it now says so and
  prints `~` rather than pretending to a count. #592 fixed the same defect in the hook
  banner; this is the report surface. (#589)
