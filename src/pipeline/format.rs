//! Format sniffer — detects structured, machine-read payloads.
//!
//! Collapse and the distillers are lossy: they are safe on human-facing free text
//! but they corrupt anything a later step parses (`jq`, `json.load`, `kubectl apply`).
//! The sniffer is the gate that keeps those payloads out of the lossy path.
//!
//! Design principle: when a payload is *plausibly* structured, say so — a missed
//! compression is cheap, a corrupted payload is not. Detection is still positive
//! (a signal must be present), because treating unknown text as structured would
//! disable compression everywhere.

use std::fmt;
use std::sync::LazyLock;

use regex::Regex;

/// Above this size a full `serde_json` parse would blow the pipeline's latency
/// budget, so bracket-shape alone decides.
const MAX_PARSE_BYTES: usize = 1_000_000;

/// Number of NDJSON lines actually parsed before trusting the rest.
const NDJSON_SAMPLE_LINES: usize = 50;

/// Minimum rows before a consistent delimiter count means anything.
const MIN_DELIMITED_LINES: usize = 3;

/// A structured payload kind. Every variant must survive the pipeline verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Structured {
    Json,
    Ndjson,
    Yaml,
    Tsv,
    Csv,
}

impl Structured {
    pub fn as_str(&self) -> &'static str {
        match self {
            Structured::Json => "json",
            Structured::Ndjson => "ndjson",
            Structured::Yaml => "yaml",
            Structured::Tsv => "tsv",
            Structured::Csv => "csv",
        }
    }
}

impl fmt::Display for Structured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Label recorded on the passthrough event when the gate fires.
pub fn passthrough_reason(kind: Structured) -> String {
    format!("structured:{}", kind)
}

/// Classify `input`. `None` means plain text — safe to compress.
pub fn sniff(input: &str) -> Option<Structured> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // NDJSON sits between the two JSON checks: a `{…}\n{…}` stream is bracketed like
    // a JSON document but only parses line by line.
    sniff_json(trimmed)
        .or_else(|| sniff_ndjson(trimmed))
        .or_else(|| sniff_json_shaped(trimmed))
        .or_else(|| sniff_yaml(trimmed))
        .or_else(|| sniff_delimited(trimmed))
}

/// True when `input` must not be compressed.
pub fn is_structured(input: &str) -> bool {
    sniff(input).is_some()
}

/// Definitive JSON: a whole document that parses (or is too big to parse in budget).
fn sniff_json(trimmed: &str) -> Option<Structured> {
    if !is_bracketed(trimmed) {
        return None;
    }

    // Too large to parse in budget: bracket shape is the only evidence we can
    // afford, and it is enough to stay off the lossy path.
    if trimmed.len() > MAX_PARSE_BYTES {
        return Some(Structured::Json);
    }

    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .map(|_| Structured::Json)
}

/// Bracketed but unparseable — truncated or comment-bearing JSON. Unsure →
/// structured: compression cannot repair it, but it can make it worse.
fn sniff_json_shaped(trimmed: &str) -> Option<Structured> {
    if is_bracketed(trimmed) && has_json_field_syntax(trimmed) {
        return Some(Structured::Json);
    }
    None
}

fn is_bracketed(trimmed: &str) -> bool {
    let (Some(first), Some(last)) = (
        trimmed.as_bytes().first().copied(),
        trimmed.as_bytes().last().copied(),
    ) else {
        return false;
    };
    (first == b'{' && last == b'}') || (first == b'[' && last == b']')
}

/// Looks for `"key":` — the one bit of syntax free text almost never carries.
fn has_json_field_syntax(text: &str) -> bool {
    static JSON_FIELD: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#""[^"]*"\s*:"#).expect("valid json field regex"));
    JSON_FIELD.is_match(text)
}

fn sniff_ndjson(trimmed: &str) -> Option<Structured> {
    let lines: Vec<&str> = non_empty_lines(trimmed);
    if lines.len() < 2 {
        return None;
    }
    if !lines
        .iter()
        .all(|l| l.starts_with('{') || l.starts_with('['))
    {
        return None;
    }
    // Parsing every line of a huge stream is not worth the latency; a sample that
    // holds is strong enough evidence.
    if lines
        .iter()
        .take(NDJSON_SAMPLE_LINES)
        .all(|l| serde_json::from_str::<serde_json::Value>(l).is_ok())
    {
        return Some(Structured::Ndjson);
    }
    None
}

fn sniff_yaml(trimmed: &str) -> Option<Structured> {
    let lines: Vec<&str> = trimmed.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return None;
    }

    // A leading document marker is unambiguous.
    let first = lines[0].trim_end();
    if first == "---" || first.starts_with("--- ") {
        return Some(Structured::Yaml);
    }

    if lines.len() < 2 {
        return None;
    }

    // Otherwise every line must be YAML-shaped, and the document must show real
    // structure (nesting or a list). Free-text logs fail the first test: their
    // lines carry no `key:` at all.
    if !lines.iter().all(|l| is_yaml_shaped(l)) {
        return None;
    }
    let has_structure = lines
        .iter()
        .any(|l| l.starts_with(' ') || l.trim_start().starts_with("- "));
    if !has_structure {
        return None;
    }

    Some(Structured::Yaml)
}

fn is_yaml_shaped(line: &str) -> bool {
    static YAML_KEY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"^\s*(?:"[^"]*"|'[^']*'|[A-Za-z_][A-Za-z0-9_.\-/]*)\s*:(?:\s.*|)$"#)
            .expect("valid yaml key regex")
    });

    let trimmed = line.trim();
    if trimmed.starts_with('#') || trimmed == "---" || trimmed == "..." {
        return true;
    }
    if trimmed == "-" || trimmed.starts_with("- ") {
        return true;
    }
    YAML_KEY.is_match(line)
}

fn sniff_delimited(trimmed: &str) -> Option<Structured> {
    let lines = non_empty_lines(trimmed);
    if lines.len() < MIN_DELIMITED_LINES {
        return None;
    }

    for (delim, kind) in [('\t', Structured::Tsv), (',', Structured::Csv)] {
        let expected = lines[0].matches(delim).count();
        if expected == 0 {
            continue;
        }
        if lines.iter().all(|l| l.matches(delim).count() == expected) {
            return Some(kind);
        }
    }

    None
}

fn non_empty_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_json_object() {
        assert_eq!(sniff(r#"{"a": 1, "b": [2, 3]}"#), Some(Structured::Json));
    }

    #[test]
    fn detects_json_array_of_similar_objects() {
        // The live repro: `az ... -o json` shaped output that collapse used to squash.
        let items: Vec<String> = (0..20)
            .map(|i| format!(r#"{{"name": "vm-{i}", "location": "eastus", "id": {i}}}"#))
            .collect();
        let payload = format!("[\n  {}\n]", items.join(",\n  "));
        assert_eq!(sniff(&payload), Some(Structured::Json));
    }

    #[test]
    fn detects_pretty_printed_json_with_newlines() {
        let payload = "{\n  \"kind\": \"List\",\n  \"items\": [\n    {\"n\": 1}\n  ]\n}";
        assert_eq!(sniff(payload), Some(Structured::Json));
    }

    #[test]
    fn treats_truncated_json_as_structured() {
        // Unsure → structured. Compressing a truncated payload cannot help it.
        let payload = r#"{"items": [{"name": "a"}, {"name": "b"#;
        assert_eq!(sniff(payload), None, "unbracketed tail is not JSON-shaped");

        let bracketed = r#"{"items": [{"name": "a"}, {"name": }"#;
        assert_eq!(sniff(bracketed), Some(Structured::Json));
    }

    #[test]
    fn detects_ndjson_stream() {
        let payload =
            "{\"ts\": 1, \"msg\": \"a\"}\n{\"ts\": 2, \"msg\": \"b\"}\n{\"ts\": 3, \"msg\": \"c\"}";
        assert_eq!(sniff(payload), Some(Structured::Ndjson));
    }

    #[test]
    fn detects_yaml_with_document_marker() {
        let payload = "---\napiVersion: v1\nkind: ConfigMap\ndata:\n  key: value\n";
        assert_eq!(sniff(payload), Some(Structured::Yaml));
    }

    #[test]
    fn detects_yaml_without_document_marker() {
        let payload = "apiVersion: apps/v1\nkind: Deployment\nspec:\n  replicas: 3\n";
        assert_eq!(sniff(payload), Some(Structured::Yaml));
    }

    #[test]
    fn detects_tsv_table() {
        let payload = "name\tstatus\tage\npod-a\tRunning\t1d\npod-b\tRunning\t2d\n";
        assert_eq!(sniff(payload), Some(Structured::Tsv));
    }

    #[test]
    fn detects_csv_table() {
        let payload = "name,status,age\npod-a,Running,1d\npod-b,Running,2d\n";
        assert_eq!(sniff(payload), Some(Structured::Csv));
    }

    #[test]
    fn treats_build_log_as_plain_text() {
        let payload = "   Compiling serde v1.0.203\n   Compiling omni v0.6.1\n\
                       error[E0308]: mismatched types\n  --> src/main.rs:3:5\n   |\n\
                       3  |     let x: i32 = \"s\";\n   |            ---   ^^^ expected i32\n\
                       error: could not compile `omni` due to 1 previous error\n";
        assert_eq!(sniff(payload), None);
    }

    #[test]
    fn treats_log_lines_with_colons_as_plain_text() {
        // `level: message` looks superficially like YAML — it must not gate.
        let payload = "INFO: starting server\nWARN: cache miss\nERROR: connection refused\n";
        assert_eq!(sniff(payload), None);
    }

    #[test]
    fn treats_git_status_as_plain_text() {
        let payload = "On branch main\nChanges not staged for commit:\n  \
                       modified:   src/main.rs\n  modified:   src/lib.rs\n";
        assert_eq!(sniff(payload), None);
    }

    #[test]
    fn treats_empty_input_as_plain_text() {
        assert_eq!(sniff(""), None);
        assert_eq!(sniff("   \n\n  "), None);
    }

    #[test]
    fn treats_prose_as_plain_text() {
        let payload = "The quick brown fox jumps over the lazy dog.\n\
                       Pack my box with five dozen liquor jugs.\n";
        assert_eq!(sniff(payload), None);
    }

    #[test]
    fn survives_binary_noise_without_panicking() {
        let payload = "\u{0}\u{1}\u{2}garbage\u{7f}\u{fffd}{[}]";
        let _ = sniff(payload);
    }

    #[test]
    fn oversized_bracketed_payload_short_circuits_to_json() {
        let filler = "x".repeat(MAX_PARSE_BYTES + 1);
        let payload = format!("{{{}}}", filler);
        assert_eq!(sniff(&payload), Some(Structured::Json));
    }

    #[test]
    fn reason_label_names_the_kind() {
        assert_eq!(passthrough_reason(Structured::Json), "structured:json");
        assert_eq!(passthrough_reason(Structured::Yaml), "structured:yaml");
    }
}
