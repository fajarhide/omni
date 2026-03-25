use crate::distillers::Distiller;
use crate::pipeline::{ContentType, OutputSegment, SignalTier};

pub struct GitDistiller;

impl Distiller for GitDistiller {
    fn content_type(&self) -> ContentType {
        ContentType::GitDiff
    }

    fn distill(&self, segments: &[OutputSegment], input: &str) -> String {
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
                let hash: String = line.chars().take(7).collect();
                out.push_str(&hash);
                out.push(' ');
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

lazy_static::lazy_static! {
    pub static ref RE_GIT_LOG_HASH: regex::Regex = regex::Regex::new(r"^[a-f0-9]{7,40} ").unwrap();
}
