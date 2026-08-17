- **The context breakdown counted a quarter of a file's size and called it tokens.** It
  accumulated `size_bytes / 4` from file metadata, a rougher estimator than the 3.6 the
  rest of the report used and still not a Claude token count, so the block had to be
  labelled a rough estimate to be honest. It accumulates the sizes now, which are counted,
  and the label goes with the estimator. The largest-file-read line and the share card's
  `unit` moved with it: the card used to say `tokens` or `bytes` depending on whether the
  rows happened to carry a token column, so two installs could publish the same saving
  under different words. No figure `omni stats` prints is derived from another vendor's
  tokenizer now. (#589)
