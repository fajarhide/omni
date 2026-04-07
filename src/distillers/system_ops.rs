use crate::distillers::Distiller;
use crate::pipeline::{ContentType, OutputSegment, SignalTier};
use std::collections::BTreeMap;

pub struct SystemOpsDistiller;

impl Distiller for SystemOpsDistiller {
    fn content_type(&self) -> ContentType {
        ContentType::SystemOps
    }

    fn distill(&self, segments: &[OutputSegment], input: &str) -> String {
        let lines: Vec<&str> = input.lines().collect();
        if lines.is_empty() {
            return String::new();
        }

        // Dispatch based on content analysis
        if is_env_output(&lines) {
            distill_env_output(input)
        } else if is_ls_output(&lines) {
            distill_ls_output(input)
        } else if is_tree_output(&lines) {
            distill_tree_output(input)
        } else if is_find_output(&lines) {
            distill_find_output(input)
        } else if is_grep_output(&lines) {
            distill_grep_output(input)
        } else {
            distill_fallback(segments)
        }
    }
}

// ---------------------------------------------------------------------------
// Sensitive patterns for env redaction (Gate 6 — Security)
// ---------------------------------------------------------------------------

const SENSITIVE_PATTERNS: &[&str] = &[
    "SECRET",
    "TOKEN",
    "KEY",
    "PASSWORD",
    "PASS",
    "AUTH",
    "CRED",
    "API_",
    "AWS_",
    "GITHUB_",
    "ANTHROPIC_",
    "DATABASE_URL",
    "REDIS_URL",
    "MONGO_URL",
    "CLIENT_SECRET",
    "ACCESS_KEY",
    "OPENAI_",
    "GEMINI_",
    "PRIVATE_KEY",
];

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

fn is_grep_output(lines: &[&str]) -> bool {
    // grep/ripgrep: lines with "filepath:content" or "filepath:linenum:content"
    // Exclude lines that look like error output
    let grep_count = lines
        .iter()
        .filter(|l| {
            let l = l.trim();
            if l.is_empty() {
                return false;
            }
            // Must have a colon and NOT be a key=value pair
            if let Some(pos) = l.find(':') {
                // The part before the colon should look like a file path
                let before = &l[..pos];
                // Must not start with uppercase_key=value (that's env)
                !before.contains('=')
                    && !before.is_empty()
                    && (before.contains('/')
                        || before.contains('.')
                        || before.contains('\\'))
            } else {
                false
            }
        })
        .count();
    grep_count >= 3
}

fn is_ls_output(lines: &[&str]) -> bool {
    // ls -la: first line starts with "total N"
    let first = lines.first().map(|l| l.trim()).unwrap_or("");
    if first.starts_with("total ") {
        // Additional check: lines starting with permission string (drwx, -rw-, lrwx)
        let perm_count = lines.iter().skip(1).filter(|l| {
            let t = l.trim();
            t.starts_with("drwx")
                || t.starts_with("-rw")
                || t.starts_with("lrwx")
                || t.starts_with("d---")
                || t.starts_with("----")
                || t.starts_with("drw-")
                || t.starts_with("-r-")
                || t.starts_with("-r--")
        }).count();
        perm_count >= 1
    } else {
        false
    }
}

fn is_find_output(lines: &[&str]) -> bool {
    // find: 3+ lines starting with "./" or "/"
    let count = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("./") || (t.starts_with('/') && !t.contains(':'))
        })
        .count();
    count >= 3
}

fn is_tree_output(lines: &[&str]) -> bool {
    lines.iter().any(|l| l.contains("├──") || l.contains("└──"))
        || lines.iter().any(|l| {
            let t = l.trim();
            t.contains("directories") && t.contains("files")
        })
}

fn is_env_output(lines: &[&str]) -> bool {
    // env: 5+ lines of "UPPERCASE_KEY=value"
    let count = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            if let Some(pos) = t.find('=') {
                let key = &t[..pos];
                !key.is_empty()
                    && key
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    && key.chars().all(|c| c.is_alphanumeric() || c == '_')
            } else {
                false
            }
        })
        .count();
    count >= 5
}

// ---------------------------------------------------------------------------
// Grep/Ripgrep distiller
// ---------------------------------------------------------------------------

fn distill_grep_output(input: &str) -> String {
    let mut by_file: BTreeMap<String, u32> = BTreeMap::new();
    let mut total_matches = 0u32;
    let mut error_lines: Vec<String> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse "filepath:linenum:content" or "filepath:content"
        if let Some(colon_pos) = trimmed.find(':') {
            let filepath = &trimmed[..colon_pos];
            let content = &trimmed[colon_pos + 1..];

            // Skip if filepath doesn't look like a file
            if filepath.is_empty() || (!filepath.contains('/') && !filepath.contains('.') && !filepath.contains('\\')) {
                continue;
            }

            total_matches += 1;

            // Extract just the filename (basename)
            let basename = filepath
                .rsplit('/')
                .next()
                .unwrap_or(filepath);
            *by_file.entry(basename.to_string()).or_insert(0) += 1;

            // Check for error/panic lines — always show these
            let content_lower = content.to_lowercase();
            if (content_lower.contains("error")
                || content_lower.contains("panic")
                || content_lower.contains("fatal"))
                && error_lines.len() < 5
            {
                error_lines.push(trimmed.to_string());
            }
        }
    }

    if total_matches == 0 {
        return "grep: no matches".to_string();
    }

    let file_count = by_file.len();
    let mut out = format!("grep: {} matches in {} files", total_matches, file_count);

    // Sort by count descending
    let mut sorted: Vec<(String, u32)> = by_file.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let shown = sorted.iter().take(10);
    for (file, count) in shown {
        out.push_str(&format!("\n  {}: {} matches", file, count));
    }
    if sorted.len() > 10 {
        out.push_str(&format!("\n  +{} more files", sorted.len() - 10));
    }

    // Always show error lines
    if !error_lines.is_empty() {
        out.push_str("\nError matches:");
        for line in &error_lines {
            out.push_str(&format!("\n  {}", line));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// ls -la distiller
// ---------------------------------------------------------------------------

fn distill_ls_output(input: &str) -> String {
    let mut files = 0u32;
    let mut dirs = 0u32;
    let mut links = 0u32;
    let mut total = 0u32;
    let mut newest_file: Option<String> = None;

    for line in input.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        total += 1;

        if trimmed.starts_with('d') {
            dirs += 1;
        } else if trimmed.starts_with('l') {
            links += 1;
        } else if trimmed.starts_with('-') {
            files += 1;
        }

        // Track the last file listed (which is typically the newest in sorted output)
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 9 {
            // Last column(s) = filename - may include spaces if quoted
            let filename = parts[8..].join(" ");
            if !filename.starts_with('.') || filename.len() > 1 {
                newest_file = Some(filename);
            }
        }
    }

    let mut out = format!(
        "ls: {} items | {} files, {} dirs, {} links",
        total, files, dirs, links
    );

    if let Some(ref name) = newest_file {
        out.push_str(&format!(" | last: {}", name));
    }

    out
}

// ---------------------------------------------------------------------------
// find distiller
// ---------------------------------------------------------------------------

fn distill_find_output(input: &str) -> String {
    let mut by_ext: BTreeMap<String, u32> = BTreeMap::new();
    let mut total = 0u32;
    let mut samples: Vec<String> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        total += 1;

        // Extract extension
        let ext = if let Some(dot_pos) = trimmed.rfind('.') {
            let ext = &trimmed[dot_pos..];
            // Only track if it looks like a real extension (no slashes, reasonable length)
            if ext.len() <= 10 && !ext.contains('/') {
                ext.to_string()
            } else {
                "(no ext)".to_string()
            }
        } else {
            "(no ext)".to_string()
        };

        *by_ext.entry(ext).or_insert(0) += 1;

        // Collect first 3 samples
        if samples.len() < 3 {
            samples.push(trimmed.to_string());
        }
    }

    let mut out = format!("find: {} results", total);

    // Sort by count descending, show top extensions
    let mut sorted: Vec<(String, u32)> = by_ext.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let ext_strs: Vec<String> = sorted
        .iter()
        .take(6)
        .map(|(ext, n)| format!("{}: {}", ext, n))
        .collect();
    if !ext_strs.is_empty() {
        out.push_str(&format!("\n  {}", ext_strs.join(", ")));
    }

    if !samples.is_empty() {
        out.push_str("\n  samples:");
        for s in &samples {
            out.push_str(&format!("\n    {}", s));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// tree distiller
// ---------------------------------------------------------------------------

fn distill_tree_output(input: &str) -> String {
    // Look for summary line "N directories, M files"
    let summary_line = input.lines().find(|l| {
        let t = l.trim();
        t.contains("director") && t.contains("file")
    });

    // Collect top-level dirs (depth 1 — lines starting with ├── or └──)
    let top_dirs: Vec<&str> = input
        .lines()
        .filter(|l| {
            // Top-level items: "├── name" or "└── name" (no leading spaces before the box char)
            let t = l.trim_start();
            (t.starts_with("├── ") || t.starts_with("└── "))
                && !l.starts_with("│")
                && !l.starts_with("    ")
        })
        .filter_map(|l| {
            let t = l.trim_start();
            let name = t
                .trim_start_matches("├── ")
                .trim_start_matches("└── ");
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        })
        .collect();

    let mut out = if let Some(summary) = summary_line {
        format!("tree: {}", summary.trim())
    } else {
        let total = input.lines().count();
        format!("tree: {} entries", total)
    };

    if !top_dirs.is_empty() {
        let shown: Vec<&str> = top_dirs.iter().take(8).copied().collect();
        out.push_str(&format!("\n  top: {}", shown.join(", ")));
        if top_dirs.len() > 8 {
            out.push_str(&format!(" +{} more", top_dirs.len() - 8));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// env distiller (⚠️ SECURITY CRITICAL — Gate 6)
// ---------------------------------------------------------------------------

pub fn distill_env_output(input: &str) -> String {
    let mut total = 0u32;
    let mut redacted_count = 0u32;
    let mut by_prefix: BTreeMap<String, u32> = BTreeMap::new();
    let mut redacted_lines: Vec<String> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(eq_pos) = trimmed.find('=') {
            let key = &trimmed[..eq_pos];
            total += 1;

            // Check if sensitive
            let key_upper = key.to_uppercase();
            let is_sensitive = SENSITIVE_PATTERNS
                .iter()
                .any(|p| key_upper.contains(p));

            if is_sensitive {
                redacted_count += 1;
                redacted_lines.push(format!("{}=[REDACTED]", key));
            }

            // Group by prefix (first word before _ or full key if no _)
            let prefix = if let Some(underscore_pos) = key.find('_') {
                let p = &key[..underscore_pos];
                if p.is_empty() { key.to_string() } else { p.to_string() }
            } else {
                key.to_string()
            };
            *by_prefix.entry(prefix).or_insert(0) += 1;
        }
    }

    let mut out = format!(
        "env: {} vars | REDACTED: {} sensitive",
        total, redacted_count
    );

    // Sort by count descending, show top prefixes
    let mut sorted: Vec<(String, u32)> = by_prefix.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let prefix_strs: Vec<String> = sorted
        .iter()
        .take(8)
        .map(|(prefix, n)| format!("{}({})", prefix, n))
        .collect();
    if !prefix_strs.is_empty() {
        out.push_str(&format!("\n  {}", prefix_strs.join(" ")));
    }

    // Show redacted keys for transparency
    if !redacted_lines.is_empty() {
        out.push_str("\n  Sensitive:");
        for rl in redacted_lines.iter().take(10) {
            out.push_str(&format!("\n    {}", rl));
        }
        if redacted_lines.len() > 10 {
            out.push_str(&format!("\n    +{} more", redacted_lines.len() - 10));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Fallback: take max 30 lines from segments
// ---------------------------------------------------------------------------

fn distill_fallback(segments: &[OutputSegment]) -> String {
    let mut out = String::new();
    let mut line_count = 0;

    for seg in segments {
        if matches!(seg.tier, SignalTier::Critical | SignalTier::Important) {
            for line in seg.content.lines() {
                if line_count >= 30 {
                    break;
                }
                out.push_str(line);
                out.push('\n');
                line_count += 1;
            }
        }
        if line_count >= 30 {
            break;
        }
    }

    // If no critical/important found, take first 30 lines from any segment
    if out.trim().is_empty() {
        for seg in segments {
            for line in seg.content.lines() {
                if line_count >= 30 {
                    break;
                }
                out.push_str(line);
                out.push('\n');
                line_count += 1;
            }
            if line_count >= 30 {
                break;
            }
        }
    }

    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_redaction_removes_secrets() {
        let input = "ANTHROPIC_API_KEY=sk-ant-abc123\nHOME=/home/user\nGITHUB_TOKEN=ghp_secret";
        let result = distill_env_output(input);
        assert!(!result.contains("sk-ant-abc123"), "API key should be redacted");
        assert!(!result.contains("ghp_secret"), "GitHub token should be redacted");
        assert!(result.contains("[REDACTED]"), "Should contain [REDACTED] marker");
    }

    #[test]
    fn test_env_redaction_covers_all_sensitive_patterns() {
        let input = [
            "SECRET_KEY=mysecret",
            "TOKEN=mytoken",
            "API_KEY=myapikey",
            "PASSWORD=mypassword",
            "AUTH_TOKEN=myauth",
            "DATABASE_URL=postgres://secret",
            "AWS_SECRET_ACCESS_KEY=awssecret",
            "OPENAI_API_KEY=sk-abc",
            "GEMINI_API_KEY=gem-abc",
            "HOME=/home/user",
            "PATH=/usr/bin",
            "SHELL=/bin/zsh",
            "TERM=xterm",
            "EDITOR=vim",
        ]
        .join("\n");

        let result = distill_env_output(&input);
        assert!(!result.contains("mysecret"));
        assert!(!result.contains("mytoken"));
        assert!(!result.contains("myapikey"));
        assert!(!result.contains("mypassword"));
        assert!(!result.contains("myauth"));
        assert!(!result.contains("postgres://secret"));
        assert!(!result.contains("awssecret"));
        assert!(!result.contains("sk-abc"));
        assert!(!result.contains("gem-abc"));
    }

    #[test]
    fn test_grep_detection() {
        let lines = vec![
            "src/main.rs:10:fn main() {",
            "src/lib.rs:5:pub mod test;",
            "src/utils.rs:20:fn helper() {",
        ];
        assert!(is_grep_output(&lines));
    }

    #[test]
    fn test_ls_detection() {
        let lines = vec![
            "total 48",
            "drwxr-xr-x  5 user staff  160 Apr  5 10:00 .",
            "-rw-r--r--  1 user staff 1024 Apr  5 10:00 file.txt",
        ];
        assert!(is_ls_output(&lines));
    }

    #[test]
    fn test_find_detection() {
        let lines = vec![
            "./src/main.rs",
            "./src/lib.rs",
            "./src/utils.rs",
            "./Cargo.toml",
        ];
        assert!(is_find_output(&lines));
    }

    #[test]
    fn test_tree_detection() {
        let lines = vec![
            ".",
            "├── src",
            "│   ├── main.rs",
            "│   └── lib.rs",
            "└── Cargo.toml",
        ];
        assert!(is_tree_output(&lines));
    }

    #[test]
    fn test_env_detection() {
        let lines = vec![
            "HOME=/home/user",
            "PATH=/usr/bin",
            "SHELL=/bin/zsh",
            "TERM=xterm",
            "EDITOR=vim",
            "LANG=en_US.UTF-8",
        ];
        assert!(is_env_output(&lines));
    }

    #[test]
    fn test_grep_distill_groups_by_file() {
        let input = "src/main.rs:10:fn main() {\nsrc/main.rs:20:    println!(\"hello\");\nsrc/main.rs:30:}\nsrc/lib.rs:5:pub mod test;\nsrc/utils.rs:1:use std::io;";
        let result = distill_grep_output(input);
        assert!(result.contains("grep: 5 matches in 3 files"));
        assert!(result.contains("main.rs: 3 matches"));
    }

    #[test]
    fn test_grep_distill_shows_error_lines() {
        let input =
            "src/auth.rs:47:    return Err(AuthError::InvalidToken);\nsrc/db.rs:10:fn connect() {\nsrc/db.rs:20:fn query() {\nsrc/auth.rs:50:    panic!(\"fatal auth error\");";
        let result = distill_grep_output(input);
        assert!(result.contains("Error matches:"));
        assert!(result.contains("AuthError"));
    }
}
