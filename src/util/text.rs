#![allow(clippy::string_slice)]
// Safety: All functions in this module verify char boundaries via
// `is_char_boundary()` or `char_indices()` before indexing.

pub fn safe_truncate(s: &mut String, max_bytes: usize) {
    if s.len() <= max_bytes {
        return;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    s.truncate(boundary);
}

pub fn safe_truncate_with_ellipsis(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    let mut truncated = s[..boundary].to_string();
    truncated.push_str("...");
    truncated
}

use unicode_width::UnicodeWidthStr;

/// Truncate based on display width (columns), not bytes.
pub fn display_truncate_with_ellipsis(s: &str, max_cols: usize) -> String {
    if s.width() <= max_cols {
        return s.to_string();
    }

    let mut current_width = 0;
    let mut boundary = 0;

    for (i, c) in s.char_indices() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if current_width + cw > max_cols {
            break;
        }
        current_width += cw;
        boundary = i + c.len_utf8();
    }

    let mut truncated = s[..boundary].to_string();
    truncated.push_str("...");
    truncated
}

pub fn safe_slice(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_truncate() {
        let mut s = String::from("Hello, 🌍!");
        safe_truncate(&mut s, 8); // "Hello, " is 7 bytes, "🌍" is 4 bytes. If we cut at 8, it should fall back to 7.
        assert_eq!(s, "Hello, ");
    }

    #[test]
    fn test_safe_truncate_with_ellipsis() {
        let s = "Hello, 🌍!";
        let res = safe_truncate_with_ellipsis(s, 8);
        assert_eq!(res, "Hello, ...");
    }

    #[test]
    fn test_safe_slice() {
        let s = "Hello, 🌍!";
        let res = safe_slice(s, 8);
        assert_eq!(res, "Hello, ");
    }
}
