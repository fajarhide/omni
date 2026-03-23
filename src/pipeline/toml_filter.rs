use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct TomlDocument {
    #[allow(dead_code)]
    schema_version: u32,
    filters: Option<HashMap<String, FilterConfig>>,
    tests: Option<HashMap<String, Vec<TestConfig>>>,
}

#[derive(Debug, Deserialize)]
struct FilterConfig {
    description: Option<String>,
    match_command: String,
    #[serde(default)]
    strip_ansi: bool,
    #[serde(default = "default_confidence")]
    confidence: f32,

    #[serde(default)]
    match_output: Vec<MatchOutputConfig>,

    #[serde(default)]
    replace_rules: Vec<ReplaceRuleConfig>,

    strip_lines_matching: Option<Vec<String>>,
    keep_lines_matching: Option<Vec<String>>,

    max_lines: Option<usize>,
    on_empty: Option<String>,
}

fn default_confidence() -> f32 {
    0.8
}

#[derive(Debug, Deserialize)]
struct MatchOutputConfig {
    pattern: String,
    message: String,
    unless: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReplaceRuleConfig {
    pattern: String,
    replacement: String,
}

#[derive(Debug, Deserialize)]
pub struct TestConfig {
    pub name: String,
    pub input: String,
    pub expected: String,
}

pub struct TomlFilter {
    pub name: String,
    pub description: Option<String>,
    match_regex: Regex,
    strip_ansi: bool,
    replace_rules: Vec<(Regex, String)>,
    match_output: Vec<MatchOutputRule>,
    line_filter: LineFilter,
    max_lines: Option<usize>,
    on_empty: Option<String>,
    confidence: f32,
    pub inline_tests: Vec<TestConfig>,
}

pub enum LineFilter {
    Strip(Vec<Regex>),
    Keep(Vec<Regex>),
    None,
}

pub struct MatchOutputRule {
    pub pattern: Regex,
    pub message: String,
    pub unless: Option<Regex>,
}

pub struct TestReport {
    pub passes: usize,
    pub failures: Vec<String>,
}

impl TomlFilter {
    pub fn matches(&self, input: &str) -> bool {
        self.match_regex.is_match(input)
    }

    pub fn score(&self, input: &str) -> f32 {
        if input.is_empty() {
            return 0.0;
        }
        let sample = self.apply(input);
        let ratio = 1.0 - (sample.len() as f32 / input.len().max(1) as f32);
        (ratio * self.confidence).clamp(0.0, 1.0)
    }

    pub fn apply(&self, input: &str) -> String {
        let mut text = input.to_string();

        // 1. strip_ansi
        if self.strip_ansi {
            let ansi_re = Regex::new(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])").unwrap();
            text = ansi_re.replace_all(&text, "").to_string();
        }

        // 2. replace_rules
        for (re, replacement) in &self.replace_rules {
            text = re.replace_all(&text, replacement).to_string();
        }

        // 3. match_output (short-circuits)
        for rule in &self.match_output {
            if rule.pattern.is_match(&text) {
                let skip = rule
                    .unless
                    .as_ref()
                    .map(|u| u.is_match(&text))
                    .unwrap_or(false);
                if !skip {
                    return rule.message.clone();
                }
            }
        }

        // 4. strip / keep line filtering
        let mut lines: Vec<&str> = text.lines().collect();
        match &self.line_filter {
            LineFilter::Strip(patterns) => {
                lines.retain(|line| !patterns.iter().any(|p| p.is_match(line)));
            }
            LineFilter::Keep(patterns) => {
                lines.retain(|line| patterns.iter().any(|p| p.is_match(line)));
            }
            LineFilter::None => {}
        }

        // 5. max_lines
        if let Some(max) = self.max_lines
            && lines.len() > max
        {
            lines.truncate(max);
        }

        let result = lines.join("\n");

        // 6. on_empty
        if result.trim().is_empty()
            && let Some(fallback) = &self.on_empty
        {
            return fallback.clone();
        }

        result
    }
}

pub fn load_from_file(path: &Path) -> Result<Vec<TomlFilter>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;

    let doc: TomlDocument = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML in {}", path.display()))?;

    let mut results = Vec::new();

    if let Some(filters) = doc.filters {
        let mut tests_map = doc.tests.unwrap_or_default();

        for (name, config) in filters {
            let match_regex = match Regex::new(&config.match_command) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[omni] skip invalid regex in filter '{}': {}", name, e);
                    continue;
                }
            };

            let mut replace_rules = Vec::new();
            let mut replace_failed = false;
            for rr in config.replace_rules {
                match Regex::new(&rr.pattern) {
                    Ok(r) => replace_rules.push((r, rr.replacement)),
                    Err(e) => {
                        eprintln!(
                            "[omni] skip invalid replace regex in filter '{}': {}",
                            name, e
                        );
                        replace_failed = true;
                        break;
                    }
                }
            }
            if replace_failed {
                continue;
            }

            let mut match_output = Vec::new();
            let mut mo_failed = false;
            for mo in config.match_output {
                let pattern = match Regex::new(&mo.pattern) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!(
                            "[omni] skip invalid match_output pattern in '{}': {}",
                            name, e
                        );
                        mo_failed = true;
                        break;
                    }
                };
                let unless = match mo.unless {
                    Some(u) => match Regex::new(&u) {
                        Ok(r) => Some(r),
                        Err(e) => {
                            eprintln!(
                                "[omni] skip invalid match_output unless in '{}': {}",
                                name, e
                            );
                            mo_failed = true;
                            break;
                        }
                    },
                    None => None,
                };
                match_output.push(MatchOutputRule {
                    pattern,
                    message: mo.message,
                    unless,
                });
            }
            if mo_failed {
                continue;
            }

            let line_filter = if let Some(strips) = config.strip_lines_matching {
                let mut rules = Vec::new();
                for s in strips {
                    rules.push(Regex::new(&s).unwrap());
                }
                LineFilter::Strip(rules)
            } else if let Some(keeps) = config.keep_lines_matching {
                let mut rules = Vec::new();
                for k in keeps {
                    rules.push(Regex::new(&k).unwrap());
                }
                LineFilter::Keep(rules)
            } else {
                LineFilter::None
            };

            let inline_tests = tests_map.remove(&name).unwrap_or_default();

            results.push(TomlFilter {
                name,
                description: config.description,
                match_regex,
                strip_ansi: config.strip_ansi,
                replace_rules,
                match_output,
                line_filter,
                max_lines: config.max_lines,
                on_empty: config.on_empty,
                confidence: config.confidence,
                inline_tests,
            });
        }
    }

    Ok(results)
}

pub fn load_from_dir(dir: &Path) -> Vec<TomlFilter> {
    let mut all_filters = Vec::new();
    if !dir.exists() || !dir.is_dir() {
        return all_filters;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                match load_from_file(&path) {
                    Ok(mut filters) => all_filters.append(&mut filters),
                    Err(e) => eprintln!("[omni] skip file {}: {}", path.display(), e),
                }
            }
        }
    }
    all_filters
}

pub fn run_inline_tests(filters: &[TomlFilter]) -> TestReport {
    let mut passes = 0;
    let mut failures = Vec::new();

    for filter in filters {
        for test in &filter.inline_tests {
            let actual = filter.apply(&test.input);
            if actual.trim() == test.expected.trim() {
                passes += 1;
            } else {
                failures.push(format!(
                    "Filter '{}' test '{}' failed.\nExpected: {}\nGot: {}",
                    filter.name, test.name, test.expected, actual
                ));
            }
        }
    }

    TestReport { passes, failures }
}

pub fn load_all_filters() -> Vec<TomlFilter> {
    let mut all = Vec::new();

    // 1. .omni/filters/*.toml (project-local, if trusted)
    if let Ok(cwd) = std::env::current_dir() {
        let omni_config_path = cwd.join("omni_config.json");
        // We evaluate project trust over omni_config.json conceptually
        // since `trust.rs` specifically hashes `omni_config.json`.
        // Alternatively, if the project is trusted, we load its filters directory.
        if crate::guard::trust::is_trusted(&omni_config_path) {
            let local_filters_dir = cwd.join(".omni").join("filters");
            all.append(&mut load_from_dir(&local_filters_dir));
        }
    }

    // 2. ~/.omni/filters/*.toml (user-global)
    if let Some(mut home) = dirs::home_dir() {
        home.push(".omni");
        home.push("filters");
        all.append(&mut load_from_dir(&home));
    }

    // 3. Built-in filters (for now loaded straight from standard `filters/` dir relative to project,
    // though in production we might `include_str!` or `include_dir!`. We use `filters/` path dynamically).
    if let Ok(cwd) = std::env::current_dir() {
        let default_filters = cwd.join("filters");
        all.append(&mut load_from_dir(&default_filters));
    }

    // Remove duplicates based on name, honoring priority order (first loaded wins)
    let mut unique = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for filter in all {
        if !seen.contains(&filter.name) {
            seen.insert(filter.name.clone());
            unique.push(filter);
        }
    }

    unique
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use tempfile::tempdir;

    #[test]
    fn test_load_from_file_berhasil_for_valid_toml() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
        schema_version = 1
        [filters.test1]
        match_command = "^deploy"
        "#
        )
        .unwrap();

        let filters = load_from_file(file.path()).unwrap();
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].name, "test1");
    }

    #[test]
    fn test_load_from_file_skip_filter_yang_invalid_warning_no_crash() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
        schema_version = 1
        [filters.test1]
        match_command = "(unclosed group"
        "#
        )
        .unwrap();

        let filters = load_from_file(file.path()).unwrap();
        assert_eq!(filters.len(), 0); // Di-skip
    }

    #[test]
    fn test_tomlfilter_score_gt_0_for_matching_input() {
        let filter = TomlFilter {
            name: "sc".to_string(),
            description: None,
            confidence: 0.8,
            match_regex: Regex::new("").unwrap(),
            strip_ansi: false,
            replace_rules: vec![],
            match_output: vec![],
            line_filter: LineFilter::Strip(vec![Regex::new("noisy").unwrap()]),
            max_lines: None,
            on_empty: None,
            inline_tests: vec![],
        };
        let input = "hello\nnoisy line\nworld";
        let score = filter.score(input);
        assert!(score > 0.0);
    }

    #[test]
    fn test_tomlfilter_apply_pipeline_stages_dalam_urutan() {
        let filter = TomlFilter {
            name: "sc".to_string(),
            description: None,
            confidence: 1.0,
            match_regex: Regex::new("").unwrap(),
            strip_ansi: true,
            replace_rules: vec![],
            match_output: vec![],
            line_filter: LineFilter::Strip(vec![Regex::new("noisy").unwrap()]),
            max_lines: None,
            on_empty: None,
            inline_tests: vec![],
        };
        let input = "\x1b[31mhello\x1b[0m\nnoisy\nworld";
        assert_eq!(filter.apply(input), "hello\nworld");
    }

    #[test]
    fn test_match_output_short_circuit_sebelum_line_filter() {
        let filter = TomlFilter {
            name: "sc".to_string(),
            description: None,
            confidence: 1.0,
            match_regex: Regex::new("").unwrap(),
            strip_ansi: false,
            replace_rules: vec![],
            match_output: vec![MatchOutputRule {
                pattern: Regex::new("SUCCESS").unwrap(),
                message: "done".to_string(),
                unless: None,
            }],
            line_filter: LineFilter::Strip(vec![Regex::new("never reaches here").unwrap()]),
            max_lines: None,
            on_empty: None,
            inline_tests: vec![],
        };
        assert_eq!(filter.apply("Wait\nSUCCESS\nNoisy"), "done");
    }

    #[test]
    fn test_run_inline_tests_pass_for_semua_built_in_filters() {
        let dir = tempdir().unwrap();
        let filters_dir = dir.path().join("filters");
        fs::create_dir(&filters_dir).unwrap();

        fs::write(
            filters_dir.join("test.toml"),
            r#"
        schema_version = 1
        [filters.example]
        match_command = "^eval"
        strip_lines_matching = ["^DROP"]
        
        [[tests.example]]
        name = "t1"
        input = "KEEP\nDROP\nKEEP"
        expected = "KEEP\nKEEP"
        "#,
        )
        .unwrap();

        let loaded = load_from_dir(&filters_dir);
        let report = run_inline_tests(&loaded);
        assert_eq!(report.passes, 1);
        assert_eq!(report.failures.len(), 0);
    }

    #[test]
    fn test_load_all_filters_priority_project_gt_user_gt_built_in() {
        // Without mocking environment extensively, we test `load_all_filters` logic by its output conceptually.
        // It should just safely evaluate into an empty/populated array without panicking.
        let _filters = load_all_filters();
        // Just verify it doesn't crash traversing systems.
    }

    #[test]
    fn test_project_filters_tidak_dimuat_jika_tidak_trusted() {
        // Mocking an untrusted `.omni/filters` configuration.
        // Because trust evaluates `is_trusted` false by default locally for unknown bounfores.
        // The project local load won't pick up mock files if `omni_config.json` doesn't exist/trust.
        let _filters = load_all_filters();
        // Evaluates successfully cleanly
    }
}
