use std::collections::HashMap;

pub fn distill_grep(content: &str) -> Option<String> {
    let line_count = content.lines().count();
    if line_count < 20 {
        return None; // Small results pass through
    }

    let mut by_file: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut file_counts: HashMap<&str, usize> = HashMap::new();

    for line in content.lines() {
        if let Some(file) = line.split(':').next()
            && !file.is_empty()
        {
            by_file.entry(file).or_default().push(line);
            *file_counts.entry(file).or_default() += 1;
        }
    }

    let file_count = by_file.len();
    if file_count == 0 {
        return None;
    }

    let mut files: Vec<&str> = file_counts.keys().copied().collect();
    // Sort by count descending
    files.sort_by(|a, b| {
        file_counts
            .get(b)
            .unwrap_or(&0)
            .cmp(file_counts.get(a).unwrap_or(&0))
    });

    let mut out = format!(
        "[OMNI Grep: {} matches in {} files]\n",
        line_count, file_count
    );

    for file in files.iter().take(10) {
        let lines = by_file.get(file).unwrap();
        let total = lines.len();
        out.push_str(&format!("\n--- {} ({} matches) ---\n", file, total));

        // Priority lines extraction
        let mut priority = Vec::new();
        let mut regular = Vec::new();

        for l in lines {
            let lower = l.to_lowercase();
            if lower.contains("error")
                || lower.contains("panic")
                || lower.contains("todo")
                || lower.contains("fixme")
                || lower.contains("unsafe")
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("token")
            {
                priority.push(*l);
            } else {
                regular.push(*l);
            }
        }

        let to_take = 3.min(total);
        let mut shown = 0;

        for l in priority.iter().take(to_take) {
            out.push_str(l);
            out.push('\n');
            shown += 1;
        }

        for l in regular.iter().take(to_take.saturating_sub(shown)) {
            out.push_str(l);
            out.push('\n');
            shown += 1;
        }

        if total > shown {
            out.push_str(&format!(
                "  ... [{} more matches in this file]\n",
                total - shown
            ));
        }
    }

    if files.len() > 10 {
        // A grep asks *where* something occurs, so a count of what was dropped
        // does not answer it: on a repo-wide search this hid 58 of 68 files, and
        // the one the caller wanted was as likely to be in the tail as the head
        // (#362). Naming the rest with their counts restores the answer for
        // about 8% of the raw bytes, which is cheap next to losing it.
        out.push_str(&format!(
            "\n--- {} more files with matches ---\n",
            files.len() - 10
        ));
        for file in files.iter().skip(10) {
            out.push_str(&format!(
                "  {} ({} matches)\n",
                file,
                file_counts.get(file).copied().unwrap_or(0)
            ));
        }
        // Phase 6: factual guard — match lines for these files are not shown.
        out.push_str(
            "[OMNI Guard: match lines shown for the 10 densest files only — use omni_retrieve for the rest]\n",
        );
    }

    if out.len() < content.len() * 8 / 10 {
        Some(out.trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::distill_grep;

    /// File `i` gets `total - i` matches, so the ranking is strict. Equal counts
    /// would leave the order to `HashMap` iteration, and a test that depends on
    /// it passes and fails on the same code.
    fn ranked_matches(files: usize) -> String {
        (0..files)
            .flat_map(|f| {
                (0..files - f)
                    .map(move |m| format!("src/file_{f:02}.rs:{}:    pub fn thing()", m + 1))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// #362: only the ten densest files were named and the rest became a bare
    /// count, so a repo-wide search answered "where" for 10 of 68 files. The
    /// caller's file is as likely to be in the tail as the head.
    #[test]
    fn names_every_file_that_matched() {
        let input = ranked_matches(30);

        let out = distill_grep(&input).expect("30 ranked files should distill");

        for f in 0..30 {
            assert!(
                out.contains(&format!("src/file_{f:02}.rs")),
                "file_{f:02} missing from the report:\n{out}"
            );
        }
    }

    /// Naming the tail must not turn into printing it: the match lines for those
    /// files stay out, which is what keeps the reduction worth having.
    #[test]
    fn keeps_match_lines_only_for_the_densest_files() {
        let input = ranked_matches(30);

        let out = distill_grep(&input).expect("should distill");

        assert!(
            out.contains("src/file_00.rs:1:"),
            "the densest file should still have its match lines quoted:\n{out}"
        );
        assert!(
            !out.contains("src/file_29.rs:1:"),
            "the sparsest file should be named, not quoted:\n{out}"
        );
        assert!(out.len() < input.len(), "must still be smaller than input");
    }
}
