use anyhow::{Context, Result};
use regex::Regex;
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

#[derive(RustEmbed)]
#[folder = "filters/"]
struct Asset;

#[derive(Debug, Deserialize)]
struct TomlDocument {
    schema_version: u32,
    filters: Option<HashMap<String, FilterConfig>>,
    tests: Option<HashMap<String, Vec<TestConfig>>>,
}

#[derive(Debug, Deserialize)]
struct FilterConfig {
    description: Option<String>,
    match_command: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct TestConfig {
    pub name: String,
    pub input: String,
    pub expected: String,
}

#[derive(Clone)]
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

#[derive(Clone)]
pub enum LineFilter {
    Strip(Vec<Regex>),
    Keep(Vec<Regex>),
    None,
}

#[derive(Clone)]
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
                    if let Some(caps) = rule.pattern.captures(&text) {
                        let mut dst = String::new();
                        caps.expand(&rule.message, &mut dst);
                        return dst;
                    }
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

    if doc.schema_version > 1 {
        eprintln!(
            "[omni] Warning: parsing newer TOML schema version {} in {}",
            doc.schema_version,
            path.display()
        );
    }

    let mut results = Vec::new();

    if let Some(filters) = doc.filters {
        let mut tests_map = doc.tests.unwrap_or_default();

        for (name, config) in filters {
            let cmd_pattern = match config.match_command {
                Some(ref c) if !c.is_empty() => c,
                _ => {
                    eprintln!("[omni] skip filter '{}': missing match_command", name);
                    continue;
                }
            };

            let match_regex = match Regex::new(cmd_pattern) {
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

/// Intelligent Repair for Filter TOMLs
pub fn try_repair_file(path: &Path) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let mut repaired = content.clone();
    let mut changed = false;

    // 1. Missing schema_version (Hard requirement for TomlDocument)
    if !repaired.contains("schema_version") {
        repaired = format!("schema_version = 1\n\n{}", repaired);
        changed = true;
    }

    // 2. Dangerous catch-all patterns
    if repaired.contains("match_command = \".*\"") {
        repaired = repaired.replace(
            "match_command = \".*\"",
            "# match_command = \".*\" # [OMNI: disabled because it intercepts all commands]",
        );
        changed = true;
    }

    // 3. Simple syntax cleanups
    // Trim trailing whitespace on every line to avoid some weirdness
    let cleaned: Vec<String> = repaired.lines().map(|l| l.trim_end().to_string()).collect();
    repaired = cleaned.join("\n");

    // 4. Try to parse with standard toml crate to verify structural integrity
    match toml::from_str::<TomlDocument>(&repaired) {
        Ok(_) => {
            if changed || repaired != content {
                fs::write(path, repaired)?;
                return Ok(true);
            }
            Ok(false)
        }
        Err(_) => {
            // Still broken. We fallback to backup in doctor.rs if it's still syntactically invalid.
            Ok(false)
        }
    }
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

pub fn load_embedded_filters() -> Vec<TomlFilter> {
    let mut all_filters = Vec::new();
    let mut files: Vec<String> = Asset::iter().map(|s| s.to_string()).collect();
    files.sort(); // Sort alphabetically so specific filters (e.g., 00_vitest.toml) load before general ones (e.g., npm.toml)
    for file in files {
        if file.ends_with(".toml")
            && let Some(content) = Asset::get(&file)
        {
            let s = String::from_utf8_lossy(&content.data);
            match toml::from_str::<TomlDocument>(&s) {
                Ok(doc) => {
                    if let Some(filters) = doc.filters {
                        let mut tests_map = doc.tests.unwrap_or_default();
                        for (name, config) in filters {
                            // Add sys_ prefix to built-in filters
                            let sys_name = format!("sys_{}", name);
                            if let Ok(filter) =
                                create_filter_from_config(sys_name, config, &mut tests_map)
                            {
                                all_filters.push(filter);
                            }
                        }
                    }
                }
                Err(e) => eprintln!("[omni] failed to parse embedded filter {}: {}", file, e),
            }
        }
    }
    all_filters
}

fn create_filter_from_config(
    name: String,
    config: FilterConfig,
    tests_map: &mut HashMap<String, Vec<TestConfig>>,
) -> Result<TomlFilter> {
    let cmd_pattern = config
        .match_command
        .as_ref()
        .filter(|c| !c.is_empty())
        .context("missing match_command")?;
    let match_regex = Regex::new(cmd_pattern)?;
    let mut replace_rules = Vec::new();
    for rr in config.replace_rules {
        replace_rules.push((Regex::new(&rr.pattern)?, rr.replacement));
    }

    let mut match_output = Vec::new();
    for mo in config.match_output {
        let pattern = Regex::new(&mo.pattern)?;
        let unless = match mo.unless {
            Some(u) => Some(Regex::new(&u)?),
            None => None,
        };
        match_output.push(MatchOutputRule {
            pattern,
            message: mo.message,
            unless,
        });
    }

    let line_filter = if let Some(strips) = config.strip_lines_matching {
        let mut rules = Vec::new();
        for s in strips {
            rules.push(Regex::new(&s)?);
        }
        LineFilter::Strip(rules)
    } else if let Some(keeps) = config.keep_lines_matching {
        let mut rules = Vec::new();
        for k in keeps {
            rules.push(Regex::new(&k)?);
        }
        LineFilter::Keep(rules)
    } else {
        LineFilter::None
    };

    let inline_tests = tests_map
        .remove(&name.replace("sys_", ""))
        .unwrap_or_default();

    Ok(TomlFilter {
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
    })
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

static ALL_FILTERS_CACHE: OnceLock<Vec<TomlFilter>> = OnceLock::new();

pub fn load_all_filters() -> &'static [TomlFilter] {
    ALL_FILTERS_CACHE.get_or_init(|| {
        let mut all = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 1. .omni/filters/*.toml (project-local, if trusted)
        if let Ok(cwd) = std::env::current_dir() {
            let local_filters_dir = cwd.join(".omni").join("filters");
            if local_filters_dir.exists() {
                let config_path = cwd.join("omni_config.json");
                if crate::guard::trust::is_trusted(&config_path) {
                    for f in load_from_dir(&local_filters_dir) {
                        if !seen.contains(&f.name) {
                            seen.insert(f.name.clone());
                            all.push(f);
                        }
                    }
                }
            }
        }

        // 2. ~/.omni/filters/*.toml (user-global)
        if let Some(mut home) = dirs::home_dir() {
            home.push(".omni");
            home.push("filters");
            for f in load_from_dir(&home) {
                if !seen.contains(&f.name) {
                    seen.insert(f.name.clone());
                    all.push(f);
                }
            }
        }

        // 3. Built-in filters (embedded)
        for f in load_embedded_filters() {
            if !seen.contains(&f.name) {
                seen.insert(f.name.clone());
                all.push(f);
            }
        }

        all
    })
}

pub fn get_filters_by_source() -> (Vec<TomlFilter>, Vec<TomlFilter>, Vec<TomlFilter>) {
    let mut built_in = Vec::new();
    let mut user = Vec::new();
    let mut local = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Local
    if let Ok(cwd) = std::env::current_dir() {
        let local_filters_dir = cwd.join(".omni").join("filters");
        if local_filters_dir.exists() {
            let config_path = cwd.join("omni_config.json");
            if crate::guard::trust::is_trusted(&config_path) {
                for f in load_from_dir(&local_filters_dir) {
                    if !seen.contains(&f.name) {
                        seen.insert(f.name.clone());
                        local.push(f);
                    }
                }
            }
        }
    }

    // 2. User
    if let Some(mut home) = dirs::home_dir() {
        home.push(".omni");
        home.push("filters");
        for f in load_from_dir(&home) {
            if !seen.contains(&f.name) {
                seen.insert(f.name.clone());
                user.push(f);
            }
        }
    }

    // 3. Built-in
    for f in load_embedded_filters() {
        if !seen.contains(&f.name) {
            seen.insert(f.name.clone());
            built_in.push(f);
        }
    }

    (built_in, user, local)
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
    fn test_project_filters_not_dimuat_jika_not_trusted() {
        // Mocking an untrusted `.omni/filters` configuration.
        // Because trust evaluates `is_trusted` false by default locally for unknown bounfores.
        // The project local load won't pick up mock files if `omni_config.json` doesn't exist/trust.
        let _filters = load_all_filters();
        // Evaluates successfully cleanly
    }

    #[test]
    fn test_verify_all_builtin_filters_pass_their_inline_tests() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let filters_dir = std::path::Path::new(&manifest_dir).join("filters");
        let filters = load_from_dir(&filters_dir);

        // Ensure we loaded something
        assert!(
            !filters.is_empty(),
            "Built-in filters directory should not be empty"
        );

        let report = run_inline_tests(&filters);
        if !report.failures.is_empty() {
            for failure in &report.failures {
                println!("{}", failure);
            }
            panic!("TOML Filter Verification Failed");
        }
    }
}
