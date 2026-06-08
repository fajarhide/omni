use std::io::{self, Write};

/// A writer wrapper that counts bytes written and stops writing after a maximum limit,
/// ensuring that it truncates safely at a valid UTF-8 character boundary.
pub struct TruncatingWriter<W: Write> {
    inner: W,
    max_bytes: usize,
    bytes_written: usize,
    truncation_notified: bool,
}

impl<W: Write> TruncatingWriter<W> {
    pub fn new(inner: W, max_bytes: usize) -> Self {
        Self {
            inner,
            max_bytes,
            bytes_written: 0,
            truncation_notified: false,
        }
    }

    pub fn bytes_written(&self) -> usize {
        self.bytes_written
    }

    pub fn is_truncated(&self) -> bool {
        self.truncation_notified
    }
}

impl<W: Write> Write for TruncatingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.bytes_written >= self.max_bytes {
            if !self.truncation_notified {
                self.inner
                    .write_all(b"\n... [OMNI: Stream truncated due to size limit] ...\n")?;
                self.truncation_notified = true;
            }
            return Ok(buf.len()); // Pretend we wrote it all to drain the source
        }

        let remaining = self.max_bytes - self.bytes_written;

        // If this chunk fits perfectly
        if buf.len() <= remaining {
            self.inner.write_all(buf)?;
            self.bytes_written += buf.len();
            Ok(buf.len())
        } else {
            // We need to truncate this specific buffer.
            // Ensure we don't slice in the middle of a UTF-8 character sequence.
            let mut safe_len = remaining;

            // Walk backwards to find the start of the last character
            while safe_len > 0 {
                // A valid UTF-8 boundary byte matches 0xxxxxxx or 11xxxxxx
                // Continuation bytes match 10xxxxxx
                if (buf[safe_len] & 0xC0) != 0x80 {
                    break;
                }
                safe_len -= 1;
            }

            if safe_len > 0 {
                self.inner.write_all(&buf[..safe_len])?;
                self.bytes_written += safe_len;
            }

            if !self.truncation_notified {
                self.inner
                    .write_all(b"\n... [OMNI: Stream truncated due to size limit] ...\n")?;
                self.truncation_notified = true;
            }

            // Still return the full buffer length so the caller keeps pushing or stops gracefully
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
