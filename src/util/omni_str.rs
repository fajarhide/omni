#![allow(clippy::string_slice)]
#![allow(dead_code)]
// Safety: OmniStr methods verify char boundaries before any indexing.

use std::borrow::Cow;
use unicode_width::UnicodeWidthStr;

/// A newtype wrapper around a string slice that enforces character-boundary
/// safe slicing and truncation, preventing the multibyte panics of Issue #92.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OmniStr<'a>(&'a str);

impl<'a> OmniStr<'a> {
    pub fn new(s: &'a str) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &'a str {
        self.0
    }

    /// Truncates the string safely by bytes, snapping back to a char boundary.
    pub fn safe_truncate_bytes(&self, max_bytes: usize) -> Self {
        if self.0.len() <= max_bytes {
            return Self(self.0);
        }
        let mut boundary = max_bytes;
        while boundary > 0 && !self.0.is_char_boundary(boundary) {
            boundary -= 1;
        }
        Self(&self.0[..boundary])
    }

    /// Truncates based on display width (columns), appending an ellipsis if truncated.
    pub fn display_truncate_with_ellipsis(&self, max_cols: usize) -> Cow<'a, str> {
        if self.0.width() <= max_cols {
            return Cow::Borrowed(self.0);
        }

        let mut current_width = 0;
        let mut boundary = 0;

        for (i, c) in self.0.char_indices() {
            let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if current_width + cw > max_cols {
                break;
            }
            current_width += cw;
            boundary = i + c.len_utf8();
        }

        let mut truncated = self.0[..boundary].to_string();
        truncated.push_str("...");
        Cow::Owned(truncated)
    }
}

impl<'a> std::fmt::Display for OmniStr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> From<&'a str> for OmniStr<'a> {
    fn from(s: &'a str) -> Self {
        Self(s)
    }
}
