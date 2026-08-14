- **The mark's ink is the terminal blue, not amber**: the site's terminal block stopped
  reading in amber when it moved onto the design system's terminal ramp, which left the
  logo as the only orange left in the product. The three gradient stops are the amber
  ones converted rather than re-picked: the middle one is `--wl-terminal-blue` exactly,
  and the outer two keep the old gradient's lightness offsets and chroma ratio at that
  hue, so the modelling in the artwork survives the swap. All three are in gamut. The
  path is untouched, and `media/logo.png` is re-rendered from the SVG at the 1254px it
  already was.
