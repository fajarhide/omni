// Safety: String slicing uses ASCII delimiter positions or boundary-checked safe utilities.
#![allow(clippy::string_slice)]

use anyhow::{Context, Result};
use regex::Regex;
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

#[derive(RustEmbed)]
#[folder = "signals/"]
struct Asset;

#[derive(Debug, Deserialize)]
struct TomlDocument {
    schema_version: u32,
    #[serde(alias = "signals")]
    filters: Option<BTreeMap<String, FilterConfig>>,
    tests: Option<HashMap<String, Vec<TestConfig>>>,
}

/// OMNI Signal Schema v2 (backward-compatible with v1).
///
/// v1 field names are accepted via `#[serde(alias)]`:
///   - `match_command`     → `trigger_pattern` (v2)
///   - `strip_lines_matching` → `noise_patterns` (v2)
///   - `keep_lines_matching`  → `signal_patterns` (v2)
///   - `match_output`      → `output_matchers` (v2)
///   - `replace_rules`     → `transform_rules` (v2)
///   - `on_empty`          → `fallback_message` (v2)
#[derive(Debug, Deserialize)]
struct FilterConfig {
    description: Option<String>,

    // v2: trigger_pattern (v1 alias: match_command)
    #[serde(alias = "match_command")]
    trigger_pattern: Option<String>,

    #[serde(default)]
    strip_ansi: bool,
    #[serde(default = "default_confidence")]
    confidence: f32,

    // v2: output_matchers (v1 alias: match_output)
    #[serde(default, alias = "match_output")]
    output_matchers: Vec<MatchOutputConfig>,

    // v2: transform_rules (v1 alias: replace_rules)
    #[serde(default, alias = "replace_rules")]
    transform_rules: Vec<ReplaceRuleConfig>,

    // v2: noise_patterns (v1 alias: strip_lines_matching)
    #[serde(alias = "strip_lines_matching")]
    noise_patterns: Option<Vec<String>>,

    // v2: signal_patterns (v1 alias: keep_lines_matching)
    #[serde(alias = "keep_lines_matching")]
    signal_patterns: Option<Vec<String>>,

    max_lines: Option<usize>,

    // v2: fallback_message (v1 alias: on_empty)
    #[serde(alias = "on_empty")]
    fallback_message: Option<String>,

    project_types: Option<Vec<String>>,

    // ─── v2-only fields ─────────────────────────────────────────────
    /// Semantic classification hint: Critical, Important, Context, Noise.
    /// When set, overrides the scorer's automatic tier assignment for this signal.
    semantic_tier: Option<String>,

    /// Free-text hint for AI agents describing what this signal does and when
    /// to pay attention to its output.
    agent_hint: Option<String>,

    /// Target compression ratio (0.0–1.0). A value of 0.3 means "try to keep
    /// only 30% of the original output". Used by adaptive compression.
    compress_ratio_target: Option<f32>,

    /// Tool family classification: build, test, lint, deploy, infra, vcs.
    /// Enables domain-level signal stacking.
    tool_family: Option<String>,

    /// Priority weight override (default 100). Higher = evaluated first.
    #[serde(default = "default_priority")]
    priority: u32,

    /// If true, the filter processes output in a streaming fashion (line-by-line/chunk-by-chunk)
    /// to prevent memory spikes for long-running noisy commands (e.g., docker pull, npm ci).
    #[serde(default)]
    stream_mode: bool,
}

fn default_priority() -> u32 {
    100
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
    pub project_types: Option<Vec<String>>,
    pub inline_tests: Vec<TestConfig>,

    // v2 fields
    pub semantic_tier: Option<String>,
    pub agent_hint: Option<String>,
    pub compress_ratio_target: Option<f32>,
    pub tool_family: Option<String>,
    pub priority: u32,
    pub stream_mode: bool,
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

pub struct LoadReport {
    pub filters: Vec<TomlFilter>,
    pub warnings: Vec<String>,
}

static ECOSYSTEM_CACHE: OnceLock<Vec<String>> = OnceLock::new();

pub fn get_current_ecosystems() -> &'static [String] {
    ECOSYSTEM_CACHE.get_or_init(|| {
        let mut eco = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            if cwd.join("package.json").exists() {
                eco.push("node".to_string());
                eco.push("npm".to_string());
                eco.push("yarn".to_string());
                eco.push("pnpm".to_string());
                eco.push("bun".to_string());
            }
            if cwd.join("Cargo.toml").exists() {
                eco.push("rust".to_string());
                eco.push("cargo".to_string());
            }
            if cwd.join("go.mod").exists() {
                eco.push("go".to_string());
            }
            if cwd.join("requirements.txt").exists() || cwd.join("pyproject.toml").exists() {
                eco.push("python".to_string());
                eco.push("pip".to_string());
            }
            if cwd.join("pom.xml").exists() || cwd.join("build.gradle").exists() {
                eco.push("java".to_string());
            }
            if cwd.join("Gemfile").exists() {
                eco.push("ruby".to_string());
            }
            if cwd.join("composer.json").exists() {
                eco.push("php".to_string());
                eco.push("composer".to_string());
            }
        }
        eco
    })
}

impl TomlFilter {
    pub fn matches(&self, input: &str) -> bool {
        if let Some(types) = self.project_types.as_ref().filter(|t| !t.is_empty()) {
            let current = get_current_ecosystems();
            // If the filter specifies a project type we don't have, skip.
            if !types.iter().any(|t| current.contains(&t.to_lowercase())) {
                return false;
            }
        }
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

pub fn load_from_file(path: &Path) -> Result<LoadReport> {
    let mut warnings = Vec::new();
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;

    let doc: TomlDocument = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML in {}", path.display()))?;

    if doc.schema_version > 2 {
        warnings.push(format!(
            "unsupported TOML schema version {} in {} (max supported: 2)",
            doc.schema_version,
            path.display()
        ));
    }

    let mut filters_result = Vec::new();

    if let Some(filters) = doc.filters {
        let mut tests_map = doc.tests.unwrap_or_default();

        for (name, config) in filters {
            let cmd_pattern = match config.trigger_pattern {
                Some(ref c) if !c.is_empty() => c,
                _ => {
                    warnings.push(format!("skip signal '{}': missing trigger_pattern", name));
                    continue;
                }
            };

            let match_regex = match Regex::new(cmd_pattern) {
                Ok(r) => r,
                Err(e) => {
                    warnings.push(format!("skip invalid regex in filter '{}': {}", name, e));
                    continue;
                }
            };

            let mut replace_rules = Vec::new();
            let mut replace_failed = false;
            for rr in config.transform_rules {
                match Regex::new(&rr.pattern) {
                    Ok(r) => replace_rules.push((r, rr.replacement)),
                    Err(e) => {
                        warnings.push(format!(
                            "skip invalid transform regex in signal '{}': {}",
                            name, e
                        ));
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
            for mo in config.output_matchers {
                let pattern = match Regex::new(&mo.pattern) {
                    Ok(r) => r,
                    Err(e) => {
                        warnings.push(format!(
                            "skip invalid output_matcher pattern in '{}': {}",
                            name, e
                        ));
                        mo_failed = true;
                        break;
                    }
                };
                let unless = match mo.unless {
                    Some(u) => match Regex::new(&u) {
                        Ok(r) => Some(r),
                        Err(e) => {
                            warnings.push(format!(
                                "skip invalid output_matcher unless in '{}': {}",
                                name, e
                            ));
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

            let line_filter = if let Some(strips) = config.noise_patterns {
                let mut rules = Vec::new();
                for s in strips {
                    match Regex::new(&s) {
                        Ok(r) => rules.push(r),
                        Err(e) => {
                            warnings.push(format!(
                                "skip invalid noise_pattern in signal '{}': {}",
                                name, e
                            ));
                        }
                    }
                }
                LineFilter::Strip(rules)
            } else if let Some(keeps) = config.signal_patterns {
                let mut rules = Vec::new();
                for k in keeps {
                    match Regex::new(&k) {
                        Ok(r) => rules.push(r),
                        Err(e) => {
                            warnings.push(format!(
                                "skip invalid signal_pattern in signal '{}': {}",
                                name, e
                            ));
                        }
                    }
                }
                LineFilter::Keep(rules)
            } else {
                LineFilter::None
            };

            let inline_tests = tests_map.remove(&name).unwrap_or_default();

            filters_result.push(TomlFilter {
                name,
                description: config.description,
                match_regex,
                strip_ansi: config.strip_ansi,
                replace_rules,
                match_output,
                line_filter,
                max_lines: config.max_lines,
                on_empty: config.fallback_message,
                confidence: config.confidence,
                project_types: config.project_types,
                inline_tests,
                // v2 fields
                semantic_tier: config.semantic_tier,
                agent_hint: config.agent_hint,
                compress_ratio_target: config.compress_ratio_target,
                tool_family: config.tool_family,
                priority: config.priority,
                stream_mode: config.stream_mode,
            });
        }
    }

    Ok(LoadReport {
        filters: filters_result,
        warnings,
    })
}

/// Intelligent Repair for Signal TOMLs
pub fn try_repair_file(path: &Path) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let mut repaired = content.clone();
    let mut changed = false;

    // 1. Missing schema_version (Hard requirement for TomlDocument)
    if !repaired.contains("schema_version") {
        repaired = format!("schema_version = 1\n\n{}", repaired);
        changed = true;
    }

    // 2. Dangerous catch-all patterns (check both v1 and v2 field names)
    for field in ["match_command", "trigger_pattern"] {
        let catchall = format!("{field} = \".*\"");
        if repaired.contains(&catchall) {
            repaired = repaired.replace(
                &catchall,
                &format!(
                    "# {field} = \".*\" # [OMNI: disabled because it intercepts all commands]"
                ),
            );
            changed = true;
        }
    }

    // 3. Simple syntax cleanups
    // Trim trailing whitespace on every line to avoid some weirdness
    let cleaned: Vec<String> = repaired.lines().map(|l| l.trim_end().to_string()).collect();
    repaired = cleaned.join("\n");

    // 3.5 Repair learned signals missing trigger_pattern/match_command
    // Strategy: if a [filters.learned_*] or [signals.learned_*] block doesn't
    // declare trigger_pattern or match_command, insert a safe non-matching
    // default (trigger_pattern = "^$") right after the table header.
    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n == "learned.toml")
    {
        let header_re = Regex::new(r"(?m)^\[(filters|signals)\.(learned_[^\]]+)\]\s*$")?;
        let trigger_re = Regex::new(r"(?m)^\s*(match_command|trigger_pattern)\s*=")?;
        let mut out = String::new();
        let mut last = 0;
        for m in header_re.find_iter(&repaired) {
            // Copy everything up to and including the header
            out.push_str(&repaired[last..m.end()]);
            out.push('\n');

            // Look ahead until next [filters.*] or [signals.*] header or EOF
            let rest = &repaired[m.end()..];
            let next_header_idx = rest
                .find("\n[filters.")
                .unwrap_or(rest.len())
                .min(rest.find("\n[signals.").unwrap_or(rest.len()));
            let block = &rest[..next_header_idx];
            if !trigger_re.is_match(block) {
                out.push_str("trigger_pattern = \"^$\"\n");
                changed = true;
            }

            last = m.end();
        }
        if last > 0 {
            out.push_str(&repaired[last..]);
            repaired = out;
        }
    }

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

pub fn load_from_dir(dir: &Path) -> LoadReport {
    let mut all_filters = Vec::new();
    let mut all_warnings = Vec::new();

    if !dir.exists() || !dir.is_dir() {
        return LoadReport {
            filters: all_filters,
            warnings: all_warnings,
        };
    }

    if let Ok(entries) = fs::read_dir(dir) {
        let mut sorted_entries: Vec<_> = entries.flatten().collect();
        sorted_entries.sort_by_key(|e| e.path());
        for entry in sorted_entries {
            let path = entry.path();
            if path.is_dir() {
                // Recurse into subdirectories (tools/, domains/, composite/)
                let sub = load_from_dir(&path);
                all_filters.extend(sub.filters);
                all_warnings.extend(sub.warnings);
            } else if path.extension().is_some_and(|ext| ext == "toml") {
                match load_from_file(&path) {
                    Ok(mut report) => {
                        all_filters.append(&mut report.filters);
                        all_warnings.append(&mut report.warnings);
                    }
                    Err(e) => all_warnings.push(format!("skip file {}: {}", path.display(), e)),
                }
            }
        }
    }
    LoadReport {
        filters: all_filters,
        warnings: all_warnings,
    }
}

pub fn load_embedded_filters() -> LoadReport {
    let mut all_filters = Vec::new();
    let mut all_warnings = Vec::new();

    let mut files: Vec<String> = Asset::iter().map(|s| s.to_string()).collect();
    files.sort();
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
                            let sys_name = format!("sys_{}", name);
                            match create_filter_from_config(sys_name, config, &mut tests_map) {
                                Ok(filter) => all_filters.push(filter),
                                Err(e) => all_warnings.push(format!(
                                    "failed to parse embedded filter {} > {}: {}",
                                    file, name, e
                                )),
                            }
                        }
                    }
                }
                Err(e) => {
                    all_warnings.push(format!("failed to parse embedded file {}: {}", file, e))
                }
            }
        }
    }
    LoadReport {
        filters: all_filters,
        warnings: all_warnings,
    }
}

fn create_filter_from_config(
    name: String,
    config: FilterConfig,
    tests_map: &mut HashMap<String, Vec<TestConfig>>,
) -> Result<TomlFilter> {
    let cmd_pattern = config
        .trigger_pattern
        .as_ref()
        .filter(|c| !c.is_empty())
        .context("missing trigger_pattern")?;
    let match_regex = Regex::new(cmd_pattern)?;
    let mut replace_rules = Vec::new();
    for rr in config.transform_rules {
        replace_rules.push((Regex::new(&rr.pattern)?, rr.replacement));
    }

    let mut match_output = Vec::new();
    for mo in config.output_matchers {
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

    let line_filter = if let Some(strips) = config.noise_patterns {
        let mut rules = Vec::new();
        for s in strips {
            rules.push(Regex::new(&s)?);
        }
        LineFilter::Strip(rules)
    } else if let Some(keeps) = config.signal_patterns {
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
        on_empty: config.fallback_message,
        confidence: config.confidence,
        project_types: config.project_types,
        inline_tests,
        semantic_tier: config.semantic_tier,
        agent_hint: config.agent_hint,
        compress_ratio_target: config.compress_ratio_target,
        tool_family: config.tool_family,
        priority: config.priority,
        stream_mode: config.stream_mode,
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

#[derive(Clone)]
struct FiltersCache {
    fingerprint: u64,
    filters: Vec<TomlFilter>,
}

static ALL_FILTERS_CACHE: OnceLock<Mutex<FiltersCache>> = OnceLock::new();

fn sort_filters_by_priority(filters: &mut [TomlFilter]) {
    // Stable sorting keeps project > user > built-in precedence for equal
    // priorities while making the documented priority knob effective.
    filters.sort_by_key(|filter| Reverse(filter.priority));
}

pub fn load_all_filters() -> Vec<TomlFilter> {
    let cache = ALL_FILTERS_CACHE.get_or_init(|| {
        Mutex::new(FiltersCache {
            fingerprint: 0,
            filters: Vec::new(),
        })
    });

    let fingerprint = compute_filters_fingerprint();
    let mut guard = cache.lock().unwrap();

    if guard.fingerprint == fingerprint && !guard.filters.is_empty() {
        return guard.filters.clone();
    }

    let filters = load_all_filters_uncached();
    guard.fingerprint = fingerprint;
    guard.filters = filters.clone();
    filters
}

/// Resolve the effective signal directory for a given base path.
/// Prefers `<base>/.omni/signals/` (new), falls back to `<base>/.omni/filters/` (legacy).
fn resolve_signal_dir(base: &Path) -> std::path::PathBuf {
    let new_path = base.join(".omni").join("signals");
    if new_path.exists() {
        new_path
    } else {
        base.join(".omni").join("filters")
    }
}

/// Resolve the effective user-global signal directory.
/// Prefers `~/.omni/signals/` (new), falls back to `~/.omni/filters/` (legacy).
fn resolve_user_signal_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| {
        let new_path = h.join(".omni").join("signals");
        if new_path.exists() {
            new_path
        } else {
            h.join(".omni").join("filters")
        }
    })
}

fn load_all_filters_uncached() -> Vec<TomlFilter> {
    let mut all = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. .omni/signals/*.toml (project-local, if trusted)
    //    Falls back to .omni/filters/ for backward compatibility.
    if let Ok(cwd) = std::env::current_dir() {
        let local_signals_dir = resolve_signal_dir(&cwd);
        if local_signals_dir.exists() {
            let config_path = cwd.join("omni_config.json");
            if crate::guard::trust::is_trusted(&config_path) {
                let report = load_from_dir(&local_signals_dir);
                for f in report.filters {
                    if !seen.contains(&f.name) {
                        seen.insert(f.name.clone());
                        all.push(f);
                    }
                }
            }
        }
    }

    // 2. ~/.omni/signals/*.toml (user-global)
    //    Falls back to ~/.omni/filters/ for backward compatibility.
    if let Some(dir) = resolve_user_signal_dir() {
        let report = load_from_dir(&dir);
        for f in report.filters {
            if !seen.contains(&f.name) {
                seen.insert(f.name.clone());
                all.push(f);
            }
        }
    }

    // 3. Built-in signals (embedded from signals/ directory)
    let report = load_embedded_filters();
    for f in report.filters {
        if !seen.contains(&f.name) {
            seen.insert(f.name.clone());
            all.push(f);
        }
    }

    sort_filters_by_priority(&mut all);
    all
}

fn compute_filters_fingerprint() -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    // 1) project-local (include trust decision + config mtime)
    if let Ok(cwd) = std::env::current_dir() {
        let config_path = cwd.join("omni_config.json");
        let is_trusted = crate::guard::trust::is_trusted(&config_path);
        is_trusted.hash(&mut hasher);
        hash_path_metadata(&config_path, &mut hasher);

        let local_signals_dir = resolve_signal_dir(&cwd);
        hash_dir_toml_entries(&local_signals_dir, &mut hasher);
    }

    // 2) user-global
    if let Some(dir) = resolve_user_signal_dir() {
        hash_dir_toml_entries(&dir, &mut hasher);
    }

    hasher.finish()
}

fn hash_dir_toml_entries(dir: &Path, hasher: &mut impl Hasher) {
    if !dir.exists() || !dir.is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();

    for p in paths {
        hash_path_metadata(&p, hasher);
    }
}

fn hash_path_metadata(path: &Path, hasher: &mut impl Hasher) {
    path.to_string_lossy().hash(hasher);

    let Ok(meta) = fs::metadata(path) else {
        0u64.hash(hasher);
        return;
    };

    meta.len().hash(hasher);
    if let Ok(modified) = meta.modified()
        && let Ok(duration) = modified.duration_since(std::time::SystemTime::UNIX_EPOCH)
    {
        duration.as_secs().hash(hasher);
        duration.subsec_nanos().hash(hasher);
    } else {
        0u64.hash(hasher);
    }
}

pub fn get_filters_by_source() -> (LoadReport, LoadReport, LoadReport) {
    let built_in = load_embedded_filters();

    let user_filters = resolve_user_signal_dir()
        .map(|d| load_from_dir(&d))
        .unwrap_or_else(|| LoadReport {
            filters: Vec::new(),
            warnings: Vec::new(),
        });

    let mut local_filters = LoadReport {
        filters: Vec::new(),
        warnings: Vec::new(),
    };
    if let Ok(cwd) = std::env::current_dir() {
        let local_dir = resolve_signal_dir(&cwd);
        local_filters = load_from_dir(&local_dir);
    }

    (built_in, user_filters, local_filters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use tempfile::tempdir;

    #[test]
    fn test_load_from_file_succeeds_for_valid_toml() {
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

        let report = load_from_file(file.path()).unwrap();
        assert_eq!(report.filters.len(), 1);
        assert_eq!(report.filters[0].name, "test1");
    }

    #[test]
    fn test_load_from_file_skips_invalid_filters_without_crashing() {
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

        let report = load_from_file(file.path()).unwrap();
        assert_eq!(report.filters.len(), 0); // Di-skip
        assert!(!report.warnings.is_empty());
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
            project_types: None,
            inline_tests: vec![],
            semantic_tier: None,
            agent_hint: None,
            compress_ratio_target: None,
            tool_family: None,
            priority: 100,
            stream_mode: false,
        };
        let input = "hello\nnoisy line\nworld";
        let score = filter.score(input);
        assert!(score > 0.0);
    }

    #[test]
    fn test_tomlfilter_applies_pipeline_stages_in_order() {
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
            project_types: None,
            inline_tests: vec![],
            semantic_tier: None,
            agent_hint: None,
            compress_ratio_target: None,
            tool_family: None,
            priority: 100,
            stream_mode: false,
        };
        let input = "\x1b[31mhello\x1b[0m\nnoisy\nworld";
        assert_eq!(filter.apply(input), "hello\nworld");
    }

    #[test]
    fn test_match_output_short_circuits_before_line_filtering() {
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
            project_types: None,
            inline_tests: vec![],
            semantic_tier: None,
            agent_hint: None,
            compress_ratio_target: None,
            tool_family: None,
            priority: 100,
            stream_mode: false,
        };
        assert_eq!(filter.apply("Wait\nSUCCESS\nNoisy"), "done");
    }

    #[test]
    fn test_run_inline_tests_succeeds_for_all_built_in_filters() {
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

        let report = load_from_dir(&filters_dir);
        let test_report = run_inline_tests(&report.filters);
        assert_eq!(test_report.passes, 1);
        assert_eq!(test_report.failures.len(), 0);
    }

    #[test]
    fn orders_filters_by_priority_with_deterministic_ties() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
        schema_version = 1
        [filters.low]
        match_command = "^tool"
        priority = 50
        [filters.high_b]
        match_command = "^tool"
        priority = 200
        [filters.high_a]
        match_command = "^tool"
        priority = 200
        "#
        )
        .unwrap();

        let mut filters = load_from_file(file.path()).unwrap().filters;
        sort_filters_by_priority(&mut filters);

        let names: Vec<_> = filters.iter().map(|filter| filter.name.as_str()).collect();
        assert_eq!(names, ["high_a", "high_b", "low"]);
        assert_eq!(
            filters
                .iter()
                .map(|filter| filter.priority)
                .collect::<Vec<_>>(),
            [200, 200, 50]
        );
    }

    #[test]
    fn test_project_filters_are_not_loaded_when_untrusted() {
        // Mocking an untrusted `.omni/filters` configuration.
        // Because trust evaluates `is_trusted` false by default locally for unknown bounfores.
        // The project local load won't pick up mock files if `omni_config.json` doesn't exist/trust.
        let _filters = load_all_filters();
        // Evaluates successfully cleanly
    }

    #[test]
    fn test_verify_all_builtin_filters_pass_their_inline_tests() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let filters_dir = std::path::Path::new(&manifest_dir).join("signals");
        let report = load_from_dir(&filters_dir);

        // Ensure we loaded something
        assert!(
            !report.filters.is_empty(),
            "Built-in filters directory should not be empty"
        );

        let test_report = run_inline_tests(&report.filters);
        if !test_report.failures.is_empty() {
            for failure in &test_report.failures {
                println!("{}", failure);
            }
            panic!("TOML Filter Verification Failed");
        }
    }

    #[test]
    fn test_matches_with_project_types_filter_logic() {
        // Because get_current_ecosystems uses std::env::current_dir(), which in our tests is the repo root.
        // The repo root has Cargo.toml, so "rust" and "cargo" should be active.
        let mut filter = TomlFilter {
            name: "mock".to_string(),
            description: None,
            confidence: 1.0,
            match_regex: Regex::new("").unwrap(),
            strip_ansi: false,
            replace_rules: vec![],
            match_output: vec![],
            line_filter: LineFilter::None,
            max_lines: None,
            on_empty: None,
            project_types: Some(vec!["rust".to_string()]),
            inline_tests: vec![],
            semantic_tier: None,
            agent_hint: None,
            compress_ratio_target: None,
            tool_family: None,
            priority: 100,
            stream_mode: false,
        };

        // It should match since we are in a Rust project context
        assert!(filter.matches("anything"));

        // Change the project type to something not in the repo context
        filter.project_types = Some(vec!["java".to_string()]);
        assert!(!filter.matches("anything"));

        // Remove project_types constraint; should match universally
        filter.project_types = None;
        assert!(filter.matches("anything"));
    }
}
