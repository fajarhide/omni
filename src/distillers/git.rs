use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};

pub struct GitDistiller;

impl Distiller for GitDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> String {
        if input.contains("diff --git") {
            distill_diff(segments, input)
        } else if input.contains("On branch") || input.contains("HEAD detached") {
            distill_status(input)
        } else {
            distill_log(segments, input)
        }
    }
}

fn distill_status(input: &str) -> String {
    let mut branch = String::new();
    let mut staged = Vec::new();
    let mut modified = Vec::new();
    let mut untracked = Vec::new();

    let mut state = "none";

    for line in input.lines() {
        if line.starts_with("On branch ") {
            branch = line.replace("On branch ", "").trim().to_string();
        } else if line.contains("Changes to be committed") {
            state = "staged";
        } else if line.contains("Changes not staged for commit") {
            state = "modified";
        } else if line.contains("Untracked files:") {
            state = "untracked";
        } else if line.starts_with('\t') || line.starts_with("  ") {
            let file = line.trim().to_string();
            let clean = if file.starts_with("modified:") {
                file.replace("modified:", "").trim().to_string()
            } else if file.starts_with("new file:") {
                file.replace("new file:", "").trim().to_string()
            } else if file.starts_with("deleted:") {
                file.replace("deleted:", "").trim().to_string()
            } else if file.starts_with("renamed:") {
                file.replace("renamed:", "").trim().to_string()
            } else {
                file
            };

            if clean.is_empty() || clean.starts_with("(use") {
                continue;
            }

            match state {
                "staged" => staged.push(clean),
                "modified" => modified.push(clean),
                "untracked" => untracked.push(clean),
                _ => {}
            }
        }
    }

    let mut out = format!(
        "git: on {} | staged:{} mod:{} untracked:{}",
        branch,
        staged.len(),
        modified.len(),
        untracked.len()
    );

    let top_staged = staged
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if !top_staged.is_empty() {
        out.push_str(&format!("\nStaged: {}", top_staged));
    }

    let top_mod = modified
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if !top_mod.is_empty() {
        out.push_str(&format!("\nModified: {}", top_mod));
    }

    out
}

fn distill_diff(segments: &[OutputSegment], _input: &str) -> String {
    let mut out = String::new();
    let mut added = 0;
    let mut removed = 0;
    let mut files = std::collections::HashSet::new();

    for seg in segments {
        if seg.content.starts_with("diff --git") {
            if let Some(file) = seg
                .content
                .lines()
                .next()
                .and_then(|l| l.split(' ').next_back())
            {
                files.insert(file.to_string());
                out.push_str(&format!("{}\n", file)); // Just output the filename instead of whole header
            }
            continue;
        }

        if seg.tier == SignalTier::Noise {
            continue;
        }

        let mut hunk_out = String::new();
        // Since all hunks contain "@@ -", their tier is Important (0.7).
        // To achieve >60% reduction, we only keep context lines if specifically boosted by session context (context_score > 0).
        let keep_context = seg.context_score > 0.0 || seg.tier == SignalTier::Critical;

        for line in seg.content.lines() {
            if line.starts_with("@@ ") {
                hunk_out.push_str(line);
                hunk_out.push('\n');
            } else if line.starts_with('+') && !line.starts_with("+++") {
                added += 1;
                hunk_out.push_str(line);
                hunk_out.push('\n');
            } else if line.starts_with('-') && !line.starts_with("---") {
                removed += 1;
                hunk_out.push_str(line);
                hunk_out.push('\n');
            } else if keep_context
                && !line.starts_with("+++")
                && !line.starts_with("---")
                && !line.starts_with("index")
            {
                hunk_out.push_str(line);
                hunk_out.push('\n');
            }
        }
        out.push_str(&hunk_out);
    }

    let summary = format!(
        "git diff: {} files changed, {}+, {}-",
        files.len(),
        added,
        removed
    );
    format!("{}\n{}", summary, out.trim())
}

fn distill_log(segments: &[OutputSegment], _input: &str) -> String {
    let mut out = String::new();
    for seg in segments {
        // Look for common commit hash patterns
        for line in seg.content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line.starts_with("commit ") {
                let hash: String = line.replace("commit ", "").chars().take(7).collect();
                out.push_str(&hash);
                out.push(' ');
            } else if crate::distillers::git::RE_GIT_LOG_HASH.is_match(line) {
                // `--oneline`: the hash and the subject share one line. Taking
                // `chars().take(7)` kept the hash and threw the subject away —
                // the only part a reader wanted — and `push(' ')` then joined
                // every commit into a wall of hashes reported as an ~89% saving
                // (#107). The hash is the cheap part; the subject is the signal.
                // Keep the line whole. (The `commit <sha>` branch above still
                // handles verbose logs, so this only ever fires on oneline.)
                out.push_str(line);
                out.push('\n');
            } else if !line.starts_with("Author:")
                && !line.starts_with("Date:")
                && !line.starts_with("Merge:")
            {
                out.push_str(line);
                out.push('\n');
            }
        }
    }

    let result = out.trim().to_string();
    if result.is_empty() && !segments.is_empty() {
        // Last resort: take first 5 lines
        segments[0]
            .content
            .lines()
            .take(5)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        result
    }
}

pub static RE_GIT_LOG_HASH: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[a-f0-9]{7,40} ").unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    fn one_segment(content: &str) -> Vec<OutputSegment> {
        vec![OutputSegment {
            content: content.to_string(),
            tier: SignalTier::Important,
            base_score: 0.8,
            context_score: 0.0,
            line_range: (1, content.lines().count().max(1)),
        }]
    }

    /// #107. Each `--oneline` entry carries its subject on the same line as the
    /// hash; the distiller kept 7 chars and dropped the rest, joining every
    /// commit into a wall of hashes reported as an ~89% saving. Assert the
    /// subjects survive — the hash alone is close to worthless to a reader.
    #[test]
    fn oneline_keeps_every_commit_subject() {
        let input = "\
a370713 Wordmark ForgePod: bobot 900 asli, italic (#72)
93db32e feat: idea length limit
1017f0e fix: success token hardcoded hex";
        let out = distill_log(&one_segment(input), input);

        assert!(
            out.contains("Wordmark ForgePod"),
            "subject dropped: {out:?}"
        );
        assert!(
            out.contains("feat: idea length limit"),
            "subject dropped: {out:?}"
        );
        assert!(
            out.contains("fix: success token hardcoded hex"),
            "subject dropped: {out:?}"
        );
        // One line per commit, not a single space-joined run of hashes.
        assert_eq!(
            out.lines().count(),
            3,
            "commits joined onto one line: {out:?}"
        );
    }

    /// Verbose `git log` keeps the `commit <sha>` handling untouched: the subject
    /// still arrives on its own indented line and must survive.
    #[test]
    fn verbose_log_still_keeps_the_subject() {
        let input = "\
commit a370713abc1234567890abcdef1234567890abcd
Author: Someone <s@example.com>
Date:   Mon Mar 20 10:30:00 2026 +0700

    feat: add the thing";
        let out = distill_log(&one_segment(input), input);

        assert!(
            out.contains("feat: add the thing"),
            "subject dropped: {out:?}"
        );
        assert!(!out.contains("Author:"), "kept noise: {out:?}");
    }
}
