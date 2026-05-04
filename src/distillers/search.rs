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
        out.push_str(&format!(
            "\n... [{} more files omitted]\n",
            files.len() - 10
        ));
    }

    if out.len() < content.len() * 8 / 10 {
        Some(out.trim().to_string())
    } else {
        None
    }
}
