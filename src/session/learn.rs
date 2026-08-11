use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::LazyLock;

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

/// Lines that are never noise, however often they repeat.
///
/// A suggestion from here is framed as configuration to paste into
/// `~/.omni/signals/user.toml`, so a bad one outlives the session that produced
/// it and applies to every future command. Two classes have to be excluded by
/// construction rather than by frequency (#266):
///
/// * **OMNI's own channel markers.** Stripping `[stderr]` hides that a command
///   wrote to the error channel at all, which is worse than dropping output: the
///   reader gets no signal that anything was hidden.
/// * **Structural keys.** `spec:`, a lone brace, a markdown fence and a YAML
///   document separator are the shape of the document, not decoration. A filter
///   that removes them corrupts the manifest instead of tidying it.
fn is_never_noise(line: &str) -> bool {
    static BARE_KEY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^[A-Za-z_][A-Za-z0-9_.\-]*:$").expect("the bare-key regex is a literal")
    });

    let t = line.trim();

    if t.starts_with("[stderr]") || t.starts_with("[stdout]") || t.starts_with("[OMNI") {
        return true;
    }

    matches!(t, "{" | "}" | "[" | "]" | "---" | "...")
        || t.starts_with("```")
        || BARE_KEY.is_match(t)
}

pub fn detect_patterns(input: &str) -> Vec<PatternCandidate> {
    let mut frequency: HashMap<String, (usize, String)> = HashMap::new();
    let ansi_re = Regex::new(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])").unwrap();
    let num_re = Regex::new(r"\d+").unwrap();

    // 1. Split ke baris
    for line in input.lines() {
        let text = ansi_re.replace_all(line, "").to_string();
        let trimmed = text.trim();
        if trimmed.is_empty() || is_never_noise(trimmed) {
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
    candidates.sort_by_key(|a| std::cmp::Reverse(a.count));
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
        } else {
            // Safe fallback: match nothing rather than become a catch-all.
            toml.push_str("match_command = \"^$\"\n");
        }
    } else {
        // Safe fallback: match nothing rather than generate a skipped filter (doctor warnings).
        toml.push_str("match_command = \"^$\"\n");
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

pub fn queue_for_learn(input: &str, command: &str) {
    if input.len() <= 100 {
        return;
    }

    let input_clone = input.chars().take(5000).collect::<String>();
    let cmd = command.to_string();

    std::thread::spawn(move || {
        // Through `paths`, not around it. Resolving the directory here meant
        // `OMNI_HOME` did not cover the queue, so it was written to the real
        // `~/.omni` whatever the configuration said, and the writer and the
        // reader then disagreed about which home they were talking about (#312).
        let path = crate::paths::learn_queue_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }

        let entry = json!({
            "ts": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            "command": cmd,
            "sample": input_clone,
        });

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{}", entry);
        }

        // This thread used to call `cli::learn::run_learn` once the queue passed
        // 50 lines. `run_learn` is a user-facing printer: it writes to stdout
        // unconditionally, and in pipe mode stdout *is* the payload. On ubuntu
        // that put 149 bytes of `ℹ No learning data available yet.` on the end
        // of a git diff, so a 396 B passthrough was delivered as 545 B with
        // OMNI's console text inside the data (#312). It was green on macOS
        // every time, because the process usually exits before the thread runs,
        // which is the worst property a bug can have.
        //
        // Queueing stays; applying does not happen behind the user's back. The
        // filters it wrote unattended are the ones #307 measured accumulating to
        // 1.8 MB, and #266 had already narrowed the suggestions they are built
        // from. `omni learn --apply` still does it on request.
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queues_for_learn_non_blocking() {
        // Will fire the thread in the background
        queue_for_learn("x".repeat(300).as_str(), "make build");
    }

    #[test]
    fn generates_toml_with_numeric_placeholders() {
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
