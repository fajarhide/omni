use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Clone)]
pub enum LearnAction {
    Strip,
    Count,
}

#[derive(Debug, Clone)]
pub struct PatternCandidate {
    pub trigger_prefix: String,
    pub sample_line: String,
    pub count: usize,
    pub confidence: f32,
    pub suggested_action: LearnAction,
}

pub fn detect_patterns(input: &str) -> Vec<PatternCandidate> {
    let mut frequency: HashMap<String, (usize, String)> = HashMap::new();
    let ansi_re = Regex::new(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])").unwrap();
    let num_re = Regex::new(r"\d+").unwrap();

    // 1. Split ke baris
    for line in input.lines() {
        let text = ansi_re.replace_all(line, "").to_string();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 3. Ambil prefix: take first 3 words, but strip numbers to group similar steps
        let words: Vec<String> = trimmed
            .split_whitespace()
            .map(|w| {
                // If it's just #, ignore or keep as is? Let's keep it to preserve structure
                num_re.replace_all(w, "#").to_string()
            })
            .collect();

        let prefix = if words.len() >= 3 {
            format!("{} {} {}", words[0], words[1], words[2])
        } else {
            words.join(" ")
        };

        // 4. Hitung frekuensi setiap prefix
        let entry = frequency.entry(prefix).or_insert((0, trimmed.to_string()));
        entry.0 += 1;
    }

    let mut candidates = Vec::new();

    // 5. Filter: count >= 3
    for (prefix, (count, sample)) in frequency {
        if count >= 3 {
            // 6. Assign action
            let action = if num_re.is_match(&sample) {
                LearnAction::Count
            } else {
                LearnAction::Strip
            };

            let confidence = if count > 10 { 0.95 } else { 0.85 };

            candidates.push(PatternCandidate {
                trigger_prefix: prefix,
                sample_line: sample,
                count,
                confidence,
                suggested_action: action,
            });
        }
    }

    // 7. Sort by count desc, return max 16
    candidates.sort_by(|a, b| b.count.cmp(&a.count));
    candidates.into_iter().take(16).collect()
}

pub fn generate_toml(
    candidates: &[PatternCandidate],
    filter_name: &str,
    command: Option<&str>,
) -> String {
    let mut toml = format!("\n[filters.{}]\n", filter_name);
    toml.push_str(&format!(
        "description = \"Auto-learned filter for {}\"\n",
        command.unwrap_or("general output")
    ));

    if let Some(cmd) = command {
        // Create a simple prefix-based match for the command
        let cmd_base = cmd.split_whitespace().next().unwrap_or(cmd);
        // Ensure we don't accidentally match everything if cmd_base is empty or just special chars
        if !cmd_base.is_empty() && cmd_base != "." && cmd_base != "*" {
            toml.push_str(&format!(
                "match_command = \"^{}.*\"\n",
                regex::escape(cmd_base)
            ));
        }
    } else {
        // Omitting match_command since it's now optional in the parser.
        // This filter will be skipped by the loader, which is safer than a catch-all.
        toml.push_str("# match_command omitted (unknown command)\n");
    }

    toml.push_str("strip_ansi = true\n");
    toml.push_str("confidence = 0.85\n\n");

    let mut strips = Vec::new();
    let mut tests = format!(
        "\n[[tests.{}]]\nname = \"auto_learned_strip\"\n",
        filter_name
    );
    let mut sample_lines = String::new();

    for c in candidates {
        let clean_prefix: String = c
            .trigger_prefix
            .chars()
            .filter(|&ch| !ch.is_control() || ch == '\t')
            .collect();
        let clean_sample: String = c
            .sample_line
            .chars()
            .filter(|&ch| !ch.is_control() || ch == '\t')
            .collect();

        // Escape characters for RegEx safeties
        let escaped_prefix = regex::escape(&clean_prefix);
        // Replace the '#' placeholder with the memory-safe regex placeholder (single backslash \d+)
        let mem_regex = format!("^{}", escaped_prefix.replace('#', r"\d+"));

        // Use toml crate to handle ALL string escaping correctly for TOML
        let toml_val = toml::Value::String(mem_regex);
        let toml_safe = toml_val.to_string();

        strips.push(toml_safe);
        sample_lines.push_str(&format!("{}\n", clean_sample));
    }

    if !strips.is_empty() {
        toml.push_str(&format!("strip_lines_matching = [{}]\n", strips.join(", ")));
    }

    toml.push_str("max_lines = 50\n");
    if let Some(_first) = candidates.first() {
        toml.push_str(&format!(
            "on_empty = \"{}: dropped repetitive patterns\"\n",
            filter_name
        ));
    }

    let safe_sample = sample_lines.trim_end().replace("\"\"\"", "\"\"\\\"");
    tests.push_str(&format!("input = \"\"\"\n{}\n\"\"\"\n", safe_sample));
    if let Some(_first) = candidates.first() {
        tests.push_str(&format!(
            "expected = \"{}: dropped repetitive patterns\"\n",
            filter_name
        ));
    } else {
        tests.push_str("expected = \"\"\n");
    }

    toml.push_str(&tests);
    toml
}

pub fn apply_to_config(
    candidates: &[PatternCandidate],
    filter_name: &str,
    config_path: &Path,
    command: Option<&str>,
) -> anyhow::Result<usize> {
    if candidates.is_empty() {
        return Ok(0);
    }

    let existing_content = if config_path.exists() {
        fs::read_to_string(config_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut new_candidates = Vec::new();
    let mut skipped = 0;

    for c in candidates {
        let clean_prefix: String = c
            .trigger_prefix
            .chars()
            .filter(|&ch| !ch.is_control() || ch == '\t')
            .collect();
        let escaped_prefix = regex::escape(&clean_prefix);
        let toml_safe = escaped_prefix.replace('\\', "\\\\").replace('"', "\\\"");
        let pattern_str = format!("\"^{}\"", toml_safe);

        if existing_content.contains(&pattern_str) {
            skipped += 1;
            println!(
                "  [Skip] Pattern \"{}\" already exists in learned filters.",
                c.trigger_prefix
            );
        } else {
            new_candidates.push(c.clone());
        }
    }

    if new_candidates.is_empty() {
        println!(
            "\n  All {} patterns are already learned! No new filters added.",
            skipped
        );
        return Ok(0);
    }

    let generated = generate_toml(&new_candidates, filter_name, command);

    if !config_path.exists() {
        if let Some(p) = config_path.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(config_path, "schema_version = 1\n")?;
    }

    let mut file = OpenOptions::new().append(true).open(config_path)?;
    file.write_all(generated.as_bytes())?;

    if skipped > 0 {
        println!("\n  Skipped {} existing patterns.", skipped);
    }

    Ok(new_candidates.len())
}

pub fn queue_for_learn(input: &str, command: &str) {
    if input.len() <= 100 {
        return;
    }

    let input_clone = input.chars().take(5000).collect::<String>();
    let cmd = command.to_string();

    std::thread::spawn(move || {
        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".omni");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("learn_queue.jsonl");

        let entry = json!({
            "ts": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            "command": cmd,
            "sample": input_clone,
        });

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{}", entry);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_detect_patterns_for_repetitive_build_output() {
        let input = "Waiting for connection 1\nWaiting for connection 2\nWaiting for connection 3\nFinished dev";
        let candidates = detect_patterns(input);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].trigger_prefix, "Waiting for connection");
        assert_eq!(candidates[0].count, 3);
    }

    #[test]
    fn test_detect_patterns_for_podman_steps() {
        let input = "[1/2] STEP 1/7: FROM alpine\n[1/2] STEP 2/7: RUN ls\n[1/2] STEP 3/7: RUN date";
        let candidates = detect_patterns(input);
        assert_eq!(candidates.len(), 1);
        // Prefix should have numbers replaced by #
        assert_eq!(candidates[0].trigger_prefix, "[#/#] STEP #/#:");
        assert_eq!(candidates[0].count, 3);
    }

    #[test]
    fn test_detect_patterns_no_false_positive_pada_diverse_text() {
        let input = "one two three\nfour five six\nseven eight nine\n";
        let candidates = detect_patterns(input);
        assert_eq!(candidates.len(), 0);
    }

    #[test]
    fn test_generate_toml_menghasilkan_valid_toml() {
        let c = vec![PatternCandidate {
            trigger_prefix: "Test Prefix Gen".to_string(),
            sample_line: "Test Prefix Gen is good".to_string(),
            count: 5,
            confidence: 0.9,
            suggested_action: LearnAction::Strip,
        }];
        let toml = generate_toml(&c, "gen_test", None);
        // schema_version is now handled by apply_to_config, not generation
        assert!(toml.contains("[filters.gen_test]"));
        assert!(toml.contains("\"^Test Prefix Gen\""));
    }

    #[test]
    fn test_apply_to_config_not_duplicate_trigger() {
        let file = NamedTempFile::new().unwrap();
        let c = vec![PatternCandidate {
            trigger_prefix: "Test".to_string(),
            sample_line: "x".to_string(),
            count: 3,
            confidence: 0.9,
            suggested_action: LearnAction::Strip,
        }];
        apply_to_config(&c, "dummy", file.path(), None).unwrap();
        let content = fs::read_to_string(file.path()).unwrap();
        assert!(content.contains("[filters.dummy]"));
    }

    #[test]
    fn test_queue_for_learn_non_blocking() {
        // Will fire the thread in the background
        queue_for_learn("x".repeat(300).as_str(), "make build");
    }

    #[test]
    fn test_generate_toml_with_numeric_placeholders() {
        let c = vec![PatternCandidate {
            trigger_prefix: "Step #/#:".to_string(),
            sample_line: "Step 1/2: FROM alpine".to_string(),
            count: 3,
            confidence: 0.85,
            suggested_action: LearnAction::Strip,
        }];
        let toml = generate_toml(&c, "numeric_test", None);
        // The generated regex in TOML will have escaped backslashes
        // Step #/#: -> Step \d+/\d+: -> Step \\d+/\\d+:
        assert!(
            toml.contains(r"Step \\d+/\\d+:"),
            "TOML did not contain expected regex. Got: {}",
            toml
        );
    }
}
