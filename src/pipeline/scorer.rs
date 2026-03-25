use crate::pipeline::{ContentType, OutputSegment, SessionState, SignalTier};

fn contains_any(trimmed: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|k| trimmed.contains(k))
}

pub fn classify_line(line: &str) -> SignalTier {
    let trimmed = line.trim();

    // Critical
    if contains_any(
        trimmed,
        &[
            "error[",
            "ERROR:",
            "Error:",
            "error TS",
            "FAILED",
            "FAIL:",
            "FAIL\t",
            "panic:",
            "Traceback (most recent",
            "exception:",
            "fatal:",
            "FATAL:",
            "✗",
            "× ",
        ],
    ) {
        return SignalTier::Critical;
    }

    // Important
    if contains_any(
        trimmed,
        &[
            "warning[",
            "WARNING:",
            "Warning:",
            "WARN:",
            "modified:",
            "deleted:",
            "new file:",
            "renamed:",
            "diff --git",
            "@@ -",
            "--- a/",
            "+++ b/",
            "test result:",
            "Tests:",
            "PASSED",
            "passed",
            "✓",
            "ok",
            "Successfully built",
            "Finished",
        ],
    ) {
        return SignalTier::Important;
    }

    // Noise patterns
    if contains_any(
        trimmed,
        &[
            "Compiling ",
            "Downloading ",
            "Fetching ",
            "Checking ",
            "Blocking waiting for ",
            "Locking ",
            "Downloaded ",
            "Unpacking ",
            "Installing ",
            "npm warn ",
            "[DEBUG]",
            "[TRACE]",
            "DEBUG:",
            "TRACE:",
            "Refreshing state",
        ],
    ) || trimmed.starts_with("DEBUG ")
        || trimmed.starts_with("TRACE ")
    {
        return SignalTier::Noise;
    }

    // Default context
    SignalTier::Context
}

pub fn score_line_with_context(
    line: &str,
    tier: SignalTier,
    session: Option<&SessionState>,
) -> f32 {
    let base = match tier {
        SignalTier::Critical => 0.9,
        SignalTier::Important => 0.7,
        SignalTier::Context => 0.4,
        SignalTier::Noise => 0.05,
    };

    let context_boost = session.map(|s| s.context_boost(line)).unwrap_or(0.0);

    (base + context_boost).clamp(0.0, 1.0)
}

fn split_into_hunks(input: &str) -> Vec<(String, usize, usize)> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut start_line = 1;
    let mut line_num = 1;

    for line in input.lines() {
        if line.starts_with("@@ ") || line.starts_with("diff --git") {
            if !current_chunk.is_empty() {
                chunks.push((current_chunk.clone(), start_line, line_num - 1));
                current_chunk.clear();
            }
            start_line = line_num;
        }

        if !current_chunk.is_empty() {
            current_chunk.push('\n');
        }
        current_chunk.push_str(line);
        line_num += 1;
    }

    if !current_chunk.is_empty() {
        chunks.push((current_chunk.clone(), start_line, line_num - 1));
    }

    chunks
}

pub fn score_segments(
    input: &str,
    content_type: &ContentType,
    session: Option<&SessionState>,
) -> Vec<OutputSegment> {
    let mut segments = Vec::new();

    match content_type {
        ContentType::GitDiff => {
            let hunks = split_into_hunks(input);
            for (content, start_line, end_line) in hunks {
                let tier = classify_line(&content);
                let base_score = match tier {
                    SignalTier::Critical => 0.9,
                    SignalTier::Important => 0.7,
                    SignalTier::Context => 0.4,
                    SignalTier::Noise => 0.05,
                };
                let context_score = session.map(|s| s.context_boost(&content)).unwrap_or(0.0);

                segments.push(OutputSegment {
                    content,
                    tier,
                    base_score,
                    context_score,
                    line_range: (start_line, end_line),
                });
            }
        }
        ContentType::TestOutput => {
            let mut current_chunk = String::new();
            let mut start_line = 1;
            let mut line_num = 1;

            for line in input.lines() {
                if line.starts_with("test ")
                    || line.starts_with("--- FAIL")
                    || line.starts_with("--- PASS")
                    || line.starts_with('✓')
                    || line.starts_with('✗')
                    || line.contains("test result:")
                {
                    if !current_chunk.is_empty() {
                        let tier = classify_line(&current_chunk);
                        let base_score = match tier {
                            SignalTier::Critical => 0.9,
                            SignalTier::Important => 0.7,
                            SignalTier::Context => 0.4,
                            SignalTier::Noise => 0.05,
                        };
                        let context_score = session
                            .map(|s| s.context_boost(&current_chunk))
                            .unwrap_or(0.0);

                        segments.push(OutputSegment {
                            content: current_chunk.clone(),
                            tier,
                            base_score,
                            context_score,
                            line_range: (start_line, line_num - 1),
                        });
                        current_chunk.clear();
                    }
                    start_line = line_num;
                }
                if !current_chunk.is_empty() {
                    current_chunk.push('\n');
                }
                current_chunk.push_str(line);
                line_num += 1;
            }

            if !current_chunk.is_empty() {
                let tier = classify_line(&current_chunk);
                let base_score = match tier {
                    SignalTier::Critical => 0.9,
                    SignalTier::Important => 0.7,
                    SignalTier::Context => 0.4,
                    SignalTier::Noise => 0.05,
                };
                let context_score = session
                    .map(|s| s.context_boost(&current_chunk))
                    .unwrap_or(0.0);

                segments.push(OutputSegment {
                    content: current_chunk,
                    tier,
                    base_score,
                    context_score,
                    line_range: (start_line, line_num - 1),
                });
            }
        }
        _ => {
            // Segment per line
            let mut line_num = 1;
            for line in input.lines() {
                let tier = classify_line(line);
                let base_score = match tier {
                    SignalTier::Critical => 0.9,
                    SignalTier::Important => 0.7,
                    SignalTier::Context => 0.4,
                    SignalTier::Noise => 0.05,
                };
                let context_score = session.map(|s| s.context_boost(line)).unwrap_or(0.0);

                segments.push(OutputSegment {
                    content: line.to_string(),
                    tier,
                    base_score,
                    context_score,
                    line_range: (line_num, line_num),
                });
                line_num += 1;
            }
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_line_error_variants_to_critical() {
        assert_eq!(classify_line("error[123]: bad loop"), SignalTier::Critical);
        assert_eq!(classify_line("fatal: ref is broken"), SignalTier::Critical);
        assert_eq!(classify_line("FAILED test_parse"), SignalTier::Critical);
        assert_eq!(classify_line("✗ fail"), SignalTier::Critical);
        assert_eq!(
            classify_line("error TS2307: Cannot find module"),
            SignalTier::Critical
        );
        assert_eq!(
            classify_line("FAIL\tgithub.com/user/pkg\t0.123s"),
            SignalTier::Critical
        );
    }

    #[test]
    fn test_classify_line_warning_variants_to_important() {
        assert_eq!(
            classify_line("warning[E123]: unused import"),
            SignalTier::Important
        );
        assert_eq!(classify_line("diff --git a/b"), SignalTier::Important);
        assert_eq!(classify_line("✓ success"), SignalTier::Important);
    }

    #[test]
    fn test_classify_line_compiling_lines_to_noise() {
        assert_eq!(classify_line("Compiling omni v0.1"), SignalTier::Noise);
        assert_eq!(classify_line("Downloading crates"), SignalTier::Noise);
    }

    #[test]
    fn test_classify_line_default_to_context() {
        assert_eq!(classify_line("fn main() {"), SignalTier::Context);
        assert_eq!(classify_line("println!(\"hello\");"), SignalTier::Context);
    }

    #[test]
    fn test_score_line_with_context_tanpa_session() {
        let score = score_line_with_context("error:", SignalTier::Critical, None);
        assert_eq!(score, 0.9);

        let score = score_line_with_context("normal", SignalTier::Context, None);
        assert_eq!(score, 0.4);
    }

    #[test]
    fn test_score_line_with_context_dengan_session_boost_hot_file() {
        let mut session = SessionState::new();
        for _ in 0..5 {
            session.add_hot_file("src/main.rs");
        }
        let score = score_line_with_context("at src/main.rs", SignalTier::Context, Some(&session));
        // context boost should be > 0
        assert!(score > 0.4);
    }

    #[test]
    fn test_score_line_with_context_dengan_session_boost_active_error() {
        let mut session = SessionState::new();
        session.add_error("missing semicolon");
        let score = score_line_with_context(
            "compiler says missing semicolon",
            SignalTier::Context,
            Some(&session),
        );
        assert!(score > 0.4); // exact is 0.4 + 0.25 => 0.65
    }

    #[test]
    fn test_score_segments_returns_correct_count() {
        let input = "line 1\nline 2\nline 3";
        let segments = score_segments(input, &ContentType::Unknown, None);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].line_range, (1, 1));
        assert_eq!(segments[2].line_range, (3, 3));
    }

    #[test]
    fn test_score_segments_git_diff_split_by_hunk() {
        let diff = "diff --git a/file.txt b/file.txt\nindex 1234..5678\n@@ -1,3 +1,4 @@\n line1\n line2\n@@ -10,2 +11,3 @@\n line10\n line11";
        let segments = score_segments(diff, &ContentType::GitDiff, None);

        assert_eq!(segments.len(), 3);
        // Header
        assert!(segments[0].content.starts_with("diff --git"));
        assert_eq!(segments[0].line_range, (1, 2));
        // Hunk 1
        assert!(segments[1].content.starts_with("@@ -1,3"));
        assert_eq!(segments[1].line_range, (3, 5));
        // Hunk 2
        assert!(segments[2].content.starts_with("@@ -10,2"));
        assert_eq!(segments[2].line_range, (6, 8));
    }

    #[test]
    fn test_context_boost_tidak_exceed_0_4() {
        let mut session = SessionState::new();
        for _ in 0..50 {
            session.add_hot_file("src/main.rs");
        }
        session.add_error("E0432");
        session.add_error("missing semicolon");

        let boost = session.context_boost("src/main.rs has missing semicolon and E0432");
        assert!(boost <= 0.4);
    }
}
