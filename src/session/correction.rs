use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ErrorType {
    UnknownFlag,
    CommandNotFound,
    WrongSyntax,
    WrongPath,
    MissingArg,
    PermissionDenied,
    Other(String),
}

impl ErrorType {
    pub fn as_str(&self) -> &str {
        match self {
            ErrorType::UnknownFlag => "Unknown Flag",
            ErrorType::CommandNotFound => "Command Not Found",
            ErrorType::WrongSyntax => "Wrong Syntax",
            ErrorType::WrongPath => "Wrong Path",
            ErrorType::MissingArg => "Missing Argument",
            ErrorType::PermissionDenied => "Permission Denied",
            ErrorType::Other(s) => s,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionPair {
    pub wrong_command: String,
    pub right_command: String,
    pub error_output: String,
    pub error_type: ErrorType,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionRule {
    pub wrong_pattern: String,
    pub right_pattern: String,
    pub error_type: ErrorType,
    pub occurrences: usize,
    pub base_command: String,
    pub example_error: String,
}

lazy_static! {
    static ref UNKNOWN_FLAG_RE: Regex = Regex::new(
        r"(?i)(unexpected argument|unknown (option|flag)|unrecognized (option|flag)|invalid (option|flag))"
    ).unwrap();

    static ref CMD_NOT_FOUND_RE: Regex = Regex::new(
        r"(?i)(command not found|not recognized as an internal|no such file or directory.*command)"
    ).unwrap();

    static ref WRONG_SYNTAX_RE: Regex = Regex::new(
        r"(?i)(unexpected syntax|invalid syntax|syntax error|parse error|expected.*but found)"
    ).unwrap();

    static ref WRONG_PATH_RE: Regex = Regex::new(
        r"(?i)(no such file or directory|cannot find the path|file not found)"
    ).unwrap();

    static ref MISSING_ARG_RE: Regex = Regex::new(
        r"(?i)(requires a value|requires an argument|missing (required )?argument|expected.*argument)"
    ).unwrap();

    static ref PERMISSION_DENIED_RE: Regex = Regex::new(
        r"(?i)(permission denied|access denied|not permitted)"
    ).unwrap();

    // User rejection patterns - NOT actual errors
    static ref USER_REJECTION_RE: Regex = Regex::new(
        r"(?i)(user (doesn't want|declined|rejected|cancelled)|operation (cancelled|aborted) by user)"
    ).unwrap();
}

/// Filters out user rejections - requires actual error-indicating content
pub fn is_command_error(output: &str) -> bool {
    // Reject if it's a user rejection
    if USER_REJECTION_RE.is_match(output) {
        return false;
    }

    // Must contain error-indicating content
    let output_lower = output.to_lowercase();
    output_lower.contains("error")
        || output_lower.contains("failed")
        || output_lower.contains("unknown")
        || output_lower.contains("invalid")
        || output_lower.contains("not found")
        || output_lower.contains("permission denied")
        || output_lower.contains("cannot")
}

pub fn classify_error(output: &str) -> ErrorType {
    if UNKNOWN_FLAG_RE.is_match(output) {
        ErrorType::UnknownFlag
    } else if CMD_NOT_FOUND_RE.is_match(output) {
        ErrorType::CommandNotFound
    } else if MISSING_ARG_RE.is_match(output) {
        ErrorType::MissingArg
    } else if PERMISSION_DENIED_RE.is_match(output) {
        ErrorType::PermissionDenied
    } else if WRONG_SYNTAX_RE.is_match(output) {
        ErrorType::WrongSyntax
    } else if WRONG_PATH_RE.is_match(output) {
        ErrorType::WrongPath
    } else {
        ErrorType::Other("General Error".to_string())
    }
}

/// Represents a command with its execution result for correction detection
pub struct CommandExecution {
    pub command: String,
    pub is_error: bool,
    pub output: String,
}

const CORRECTION_WINDOW: usize = 3;
const MIN_CONFIDENCE: f64 = 0.6;

/// Extract base command (first 1-2 tokens, stripping env prefixes)
pub fn extract_base_command(cmd: &str) -> String {
    let trimmed = cmd.trim();

    // Strip common env prefixes
    let stripped = trimmed
        .strip_prefix("RUST_BACKTRACE=1 ")
        .or_else(|| trimmed.strip_prefix("NODE_ENV=production "))
        .or_else(|| trimmed.strip_prefix("DEBUG=* "))
        .unwrap_or(trimmed);

    // Get first 1-2 tokens
    let parts: Vec<&str> = stripped.split_whitespace().collect();
    match parts.len() {
        0 => String::new(),
        1 => parts[0].to_string(),
        _ => {
            // Check if second part is a common sub-command
            let common_subs = [
                "commit", "push", "pull", "checkout", "install", "run", "test", "build",
            ];
            if common_subs.contains(&parts[1]) {
                format!("{} {}", parts[0], parts[1])
            } else {
                parts[0].to_string()
            }
        }
    }
}

/// Calculate similarity between two commands using Jaccard similarity
/// Same base command = 0.5 base score + up to 0.5 from argument similarity
pub fn command_similarity(a: &str, b: &str) -> f64 {
    let base_a = extract_base_command(a);
    let base_b = extract_base_command(b);

    if base_a != base_b {
        // Even if base command differs, they might be very similar (typo in base)
        let dist = strsim::levenshtein(&base_a, &base_b);
        if dist > 2 {
            return 0.0;
        }
        // Base typo similarity
        0.4
    } else {
        // Same base command
        // Extract args (everything after base command)
        let args_a: std::collections::HashSet<&str> = a
            .strip_prefix(&base_a)
            .unwrap_or("")
            .split_whitespace()
            .collect();

        let args_b: std::collections::HashSet<&str> = b
            .strip_prefix(&base_b)
            .unwrap_or("")
            .split_whitespace()
            .collect();

        if args_a.is_empty() && args_b.is_empty() {
            return 1.0; // Identical commands
        }

        let intersection = args_a.intersection(&args_b).count();
        let union = args_a.union(&args_b).count();

        if union == 0 {
            return 0.5; // Same base, no args
        }

        // 0.5 for same base + up to 0.5 for arg similarity
        0.5 + (intersection as f64 / union as f64) * 0.5
    }
}

/// Check if error is a compilation/test error (TDD cycle, not CLI correction)
fn is_tdd_cycle_error(output: &str) -> bool {
    // Compilation errors
    if output.contains("error[E")
        || output.contains("aborting due to")
        || output.contains("unresolved import")
    {
        return true;
    }

    // Test failures
    if output.contains("test result: FAILED") || output.contains("tests failed") {
        return true;
    }

    false
}

/// Check if commands differ only by path (exploration, not correction)
fn differs_only_by_path(a: &str, b: &str) -> bool {
    let base_a = extract_base_command(a);
    let base_b = extract_base_command(b);

    if base_a != base_b {
        return false;
    }

    // Simple heuristic: if similarity is relatively high (>0.6) but not identical,
    // likely just path differences
    let sim = command_similarity(a, b);
    sim > 0.6 && sim < 1.0
}

pub fn find_corrections(commands: &[CommandExecution]) -> Vec<CorrectionPair> {
    let mut corrections = Vec::new();

    for i in 0..commands.len() {
        let cmd = &commands[i];

        // Must be an actual error
        if !is_command_error(&cmd.output) && !cmd.is_error {
            continue;
        }

        let error_type = classify_error(&cmd.output);

        // Skip TDD cycle errors
        if is_tdd_cycle_error(&cmd.output) {
            continue;
        }

        // Look ahead for correction within CORRECTION_WINDOW
        for candidate in commands.iter().skip(i + 1).take(CORRECTION_WINDOW) {
            let similarity = command_similarity(&cmd.command, &candidate.command);

            // Must meet minimum similarity
            if similarity < 0.4 {
                continue;
            }

            // Skip if only path differs (exploration)
            if differs_only_by_path(&cmd.command, &candidate.command) {
                continue;
            }

            // Skip if identical commands (same error repeated)
            if cmd.command == candidate.command {
                continue;
            }

            // Calculate confidence
            let mut confidence = similarity;

            // Boost confidence if correction succeeded
            if !is_command_error(&candidate.output) && !candidate.is_error {
                confidence = (confidence + 0.2).min(1.0);
            }

            // Must meet minimum confidence
            if confidence < MIN_CONFIDENCE {
                continue;
            }

            // Found a correction!
            corrections.push(CorrectionPair {
                wrong_command: cmd.command.clone(),
                right_command: candidate.command.clone(),
                error_output: cmd.output.chars().take(500).collect(),
                error_type: error_type.clone(),
                confidence,
            });

            // Take first match only
            break;
        }
    }

    corrections
}

/// Extract the specific token that changed between wrong and right commands
fn extract_diff_token(wrong: &str, right: &str) -> String {
    let wrong_parts: std::collections::HashSet<&str> = wrong.split_whitespace().collect();
    let right_parts: std::collections::HashSet<&str> = right.split_whitespace().collect();

    // Find tokens in wrong but not in right (removed)
    let removed: Vec<&str> = wrong_parts.difference(&right_parts).copied().collect();

    // Find tokens in right but not in wrong (added)
    let added: Vec<&str> = right_parts.difference(&wrong_parts).copied().collect();

    // Return the most distinctive change
    if !removed.is_empty() && !added.is_empty() {
        format!("{} → {}", removed[0], added[0])
    } else if !removed.is_empty() {
        format!("removed {}", removed[0])
    } else if !added.is_empty() {
        format!("added {}", added[0])
    } else {
        "unknown".to_string()
    }
}

pub fn deduplicate_corrections(pairs: Vec<CorrectionPair>) -> Vec<CorrectionRule> {
    use std::collections::HashMap;

    let mut groups: HashMap<(String, String, String), Vec<CorrectionPair>> = HashMap::new();

    // Group by (base_command, error_type, diff_token)
    for pair in pairs {
        let base = extract_base_command(&pair.wrong_command);
        let error_type_str = pair.error_type.as_str().to_string();
        let diff_token = extract_diff_token(&pair.wrong_command, &pair.right_command);

        let key = (base, error_type_str, diff_token);
        groups.entry(key).or_default().push(pair);
    }

    // For each group, keep the best confidence example
    let mut rules = Vec::new();
    for ((base_command, _error_type_str, _diff_token), mut group) in groups {
        // Sort by confidence descending
        group.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let best = &group[0];
        let occurrences = group.len();

        rules.push(CorrectionRule {
            wrong_pattern: best.wrong_command.clone(),
            right_pattern: best.right_command.clone(),
            error_type: best.error_type.clone(),
            occurrences,
            base_command,
            example_error: best.error_output.clone(),
        });
    }

    // Sort by occurrences descending (most common mistakes first)
    rules.sort_by(|a, b| b.occurrences.cmp(&a.occurrences));

    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_command_error_content() {
        assert!(is_command_error("error: unknown flag"));
        assert!(is_command_error("bash: gti: command not found"));
        assert!(!is_command_error("success: all tests passed"));
    }

    #[test]
    fn test_is_command_error_filters_user_rejection() {
        assert!(!is_command_error("Operation cancelled by user"));
        assert!(is_command_error("error: permission denied"));
    }

    #[test]
    fn test_classify_error_types() {
        assert_eq!(
            classify_error("unexpected argument '--foo'"),
            ErrorType::UnknownFlag
        );
        assert_eq!(
            classify_error("command not found: gti"),
            ErrorType::CommandNotFound
        );
        assert_eq!(
            classify_error("No such file or directory"),
            ErrorType::WrongPath
        );
    }

    #[test]
    fn test_extract_base_command() {
        assert_eq!(extract_base_command("git commit -m 'fix'"), "git commit");
        assert_eq!(extract_base_command("cargo test --lib"), "cargo test");
        assert_eq!(extract_base_command("ls -la"), "ls");
        assert_eq!(
            extract_base_command("RUST_BACKTRACE=1 cargo test"),
            "cargo test"
        );
    }

    #[test]
    fn test_command_similarity() {
        assert_eq!(command_similarity("git commit", "git commit"), 1.0);
        assert_eq!(command_similarity("git status", "npm install"), 0.0);
        // Typo in base
        assert_eq!(command_similarity("gti status", "git status"), 0.4);
        // Same base, different args
        let sim = command_similarity("git commit -m 'a'", "git commit -m 'b'");
        assert!(sim > 0.5 && sim < 1.0);
    }

    #[test]
    fn test_find_corrections_logic() {
        let commands = vec![
            CommandExecution {
                command: "git commit --ammend".to_string(),
                is_error: true,
                output: "error: unexpected argument '--ammend'".to_string(),
            },
            CommandExecution {
                command: "git commit --amend".to_string(),
                is_error: false,
                output: "[main abc123] fix".to_string(),
            },
        ];

        let corrections = find_corrections(&commands);
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].wrong_command, "git commit --ammend");
        assert_eq!(corrections[0].right_command, "git commit --amend");
    }

    #[test]
    fn test_deduplicate_corrections() {
        let pairs = vec![
            CorrectionPair {
                wrong_command: "git committ".to_string(),
                right_command: "git commit".to_string(),
                error_output: "command not found".to_string(),
                error_type: ErrorType::CommandNotFound,
                confidence: 0.8,
            },
            CorrectionPair {
                wrong_command: "git committ -m 'a'".to_string(),
                right_command: "git commit -m 'a'".to_string(),
                error_output: "command not found".to_string(),
                error_type: ErrorType::CommandNotFound,
                confidence: 0.9,
            },
        ];

        let rules = deduplicate_corrections(pairs);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].occurrences, 2);
    }

    #[test]
    fn test_is_tdd_cycle_error() {
        assert!(is_tdd_cycle_error("error[E0425]: cannot find value `x`"));
        assert!(is_tdd_cycle_error(
            "test result: FAILED. 10 passed; 1 failed"
        ));
    }

    #[test]
    fn test_differs_only_by_path() {
        // High similarity but different paths
        assert!(differs_only_by_path(
            "cat /very/long/path/to/file1.txt --flag --opt",
            "cat /very/long/path/to/file2.txt --flag --opt"
        ));
        assert!(!differs_only_by_path("git commit", "git push"));
    }
}
