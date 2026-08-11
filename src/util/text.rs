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

/// Cut `s` to `max_bytes` and say what went.
///
/// Every other cut in this codebase states a count, `… and N more`,
/// `[N similar lines collapsed]`, `... [N more rows]`, `+N more files`. The
/// hooks' final safety truncation said only `[OMNI: output truncated]`, so a
/// two-line cut and a 416-line one read identically and no caller could tell
/// which it was holding (#219, the #111 never-drop invariant).
///
/// Trims back to the last newline first, so the count is exact and nobody is
/// handed half a row. Output with no newline at all has no lines to count and
/// reports bytes instead.
pub fn truncate_with_marker(s: &mut String, max_bytes: usize) {
    if s.len() <= max_bytes {
        return;
    }
    let total_lines = s.lines().count();
    let total_bytes = s.len();
    safe_truncate(s, max_bytes);

    let marker = match s.rfind('\n') {
        Some(last_newline) => {
            s.truncate(last_newline + 1);
            format!(
                "[OMNI: output truncated, {} of {total_lines} lines kept]\n",
                s.lines().count()
            )
        }
        None => format!(
            "\n[OMNI: output truncated, {} of {total_bytes} bytes kept]\n",
            s.len()
        ),
    };
    s.push_str(&marker);
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

    /// #219: the hooks' final cut said only `[OMNI: output truncated]`, so a
    /// 416-row loss on `ps aux` was indistinguishable from a two-line one.
    #[test]
    fn truncation_marker_states_how_many_lines_survived() {
        let mut s: String = (0..100).map(|i| format!("row {i}\n")).collect();

        truncate_with_marker(&mut s, 100);

        let kept = s.lines().filter(|l| l.starts_with("row ")).count();
        assert!(
            kept > 0 && kept < 100,
            "expected a partial cut, kept {kept}"
        );
        assert!(
            s.contains(&format!(
                "[OMNI: output truncated, {kept} of 100 lines kept]"
            )),
            "the marker must name what it removed: {s}"
        );
    }

    /// The cut trims back to a line boundary, so nothing downstream is handed
    /// half a row and the count in the marker is exact rather than approximate.
    #[test]
    fn truncation_never_leaves_a_partial_line() {
        let mut s: String = (0..100).map(|i| format!("row {i:04}\n")).collect();

        // 9 bytes per row, so 58 lands four bytes into the seventh, far enough
        // in that the fragment still reads as a row and a `starts_with` filter
        // cannot quietly drop it.
        truncate_with_marker(&mut s, 58);

        let last_row = s
            .lines()
            .rfind(|l| l.starts_with("row "))
            .expect("at least one row survives");
        assert_eq!(last_row.len(), 8, "row was cut mid-line: {last_row:?}");
    }

    /// One enormous line has no lines to count, so the marker reports bytes
    /// rather than claiming `1 of 1 lines kept` for a fragment.
    #[test]
    fn reports_bytes_when_the_output_has_no_line_break() {
        let mut s = "x".repeat(200);

        truncate_with_marker(&mut s, 50);

        assert!(
            s.contains("[OMNI: output truncated, 50 of 200 bytes kept]"),
            "expected a byte count: {s}"
        );
    }

    #[test]
    fn leaves_output_untouched_when_it_fits() {
        let mut s = String::from("short\noutput\n");

        truncate_with_marker(&mut s, 50_000);

        assert_eq!(s, "short\noutput\n");
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
