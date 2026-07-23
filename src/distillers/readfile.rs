pub fn distill_readfile(content: &str, filepath: &str) -> Option<String> {
    distill_readfile_with_context(content, filepath, 0)
}

const MIN_DISTILL_TOKENS: usize = 2000;

/// `imported_by_count`: number of files that import this file (from graph).
/// When > 3, append a factual warning suggesting omni_context.
pub fn distill_readfile_with_context(
    content: &str,
    filepath: &str,
    imported_by_count: usize,
) -> Option<String> {
    let line_count = content.lines().count();
    let ext = std::path::Path::new(filepath)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let hint = content_hint_for_extension(ext);
    let estimated_tokens = crate::util::token_estimate::estimate_tokens(content.len(), hint);
    if estimated_tokens < MIN_DISTILL_TOKENS {
        return None; // Below token threshold, passthrough
    }

    let distilled = match ext {
        "rs" => distill_rust_file(content),
        "py" => distill_python_file(content),
        "ts" | "tsx" | "js" | "jsx" => distill_js_ts_file(content),
        "go" => distill_go_file(content),
        "java" | "kt" => distill_java_file(content),
        "json" => distill_json_file(content),
        "toml" | "yaml" | "yml" => distill_config_file(content, ext),
        "log" | "txt" => distill_log_file(content),
        _ => distill_unknown_file(content),
    };

    // Only return if meaningful compression achieved
    if distilled.len() < content.len() * 8 / 10 {
        let mut out = format!(
            "[OMNI ReadFile: {} → distilled ({} lines)]\n{}",
            filepath, line_count, distilled
        );
        // Phase 6: factual guard — file has many dependents
        if imported_by_count > 3 {
            out.push_str(&format!(
                "\n[OMNI Guard: {} is imported by {} files — changes here may have wide impact. Call omni_context(\"{}\") for full dependency map.]",
                filepath, imported_by_count, filepath
            ));
        }
        Some(out)
    } else {
        None
    }
}

/// What a code distiller must say about the lines it dropped (#176).
///
/// These distillers keep selected lines — imports, signatures, risk markers —
/// and discard everything else, function bodies included. The never-drop
/// invariant requires the output to say so with a count; a skeleton returned
/// silently is data loss wearing a compression badge, the #111 shape.
///
/// `distill_rust_file` was the only one that said anything, and it said it
/// without a number. The other four returned the skeleton and left the reader
/// no way to tell the bodies had ever existed: a 24,999 B Python file came back
/// as 3,275 B of repeated signatures, 86.9% reported as a win, with the business
/// rule it was read for deleted and unmentioned.
fn omitted_note(total_lines: usize, kept_lines: usize) -> String {
    let omitted = total_lines.saturating_sub(kept_lines);
    if omitted == 0 {
        return String::new();
    }
    format!(
        "\n\n... [{omitted} of {total_lines} lines omitted — bodies and comments not shown. \
         Re-read with offset/limit for the full file.] ..."
    )
}

/// The scan behind every `--- … ---` section runs over the **whole** file, not
/// over the lines kept, so an empty section means "absent from the file" rather
/// than "absent from what you can see". A bare `None` next to a visibly
/// truncated body cannot convey which, so it says which.
const NONE_IN_FULL_FILE: &str = "None in the full file\n";

fn content_hint_for_extension(ext: &str) -> crate::util::token_estimate::ContentHint {
    match ext {
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go" | "java" | "kt" | "c" | "cpp" | "h"
        | "hpp" | "cs" | "php" | "ruby" | "rb" => crate::util::token_estimate::ContentHint::Code,
        "json" | "toml" | "yaml" | "yml" => crate::util::token_estimate::ContentHint::Json,
        "log" => crate::util::token_estimate::ContentHint::BuildLog,
        "md" | "txt" => crate::util::token_estimate::ContentHint::Prose,
        _ => crate::util::token_estimate::ContentHint::Mixed,
    }
}

fn distill_rust_file(content: &str) -> String {
    let mut out = String::new();
    out.push_str("--- Imports ---\n");
    let mut imports = String::new();
    let mut api = String::new();
    let mut risk = String::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let num = i + 1;
        if trimmed.starts_with("use ") || trimmed.starts_with("pub mod ") {
            imports.push_str(&format!("{} | {}\n", num, line));
        } else if trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub async fn ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("pub trait ")
            || trimmed.starts_with("impl ")
        {
            api.push_str(&format!("{} | {}\n", num, line));
        } else if trimmed.contains("todo!")
            || trimmed.contains("unimplemented!")
            || trimmed.contains("panic!")
            || trimmed.contains("FIXME")
            || trimmed.contains("TODO")
        {
            risk.push_str(&format!("{} | {}\n", num, line));
        }
    }

    if imports.is_empty() {
        out.push_str(NONE_IN_FULL_FILE);
    } else {
        out.push_str(&imports);
    }
    out.push_str("\n--- Public API / Structure ---\n");
    if api.is_empty() {
        out.push_str(NONE_IN_FULL_FILE);
    } else {
        out.push_str(&api);
    }
    out.push_str("\n--- Risk Markers (TODOs, panics) ---\n");
    if risk.is_empty() {
        out.push_str(NONE_IN_FULL_FILE);
    } else {
        out.push_str(&risk);
    }

    let kept = imports.lines().count() + api.lines().count() + risk.lines().count();
    out.push_str(&omitted_note(content.lines().count(), kept));
    out.trim().to_string()
}

fn distill_python_file(content: &str) -> String {
    let mut out = String::new();
    out.push_str("--- Imports ---\n");
    let mut imports = String::new();
    let mut api = String::new();
    let mut risk = String::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let num = i + 1;
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            imports.push_str(&format!("{} | {}\n", num, line));
        } else if trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with('@')
        {
            api.push_str(&format!("{} | {}\n", num, line));
        } else if trimmed.contains("TODO")
            || trimmed.contains("FIXME")
            || trimmed.contains("NotImplementedError")
        {
            risk.push_str(&format!("{} | {}\n", num, line));
        }
    }
    if imports.is_empty() {
        out.push_str(NONE_IN_FULL_FILE);
    } else {
        out.push_str(&imports);
    }
    out.push_str("\n--- Public API / Structure ---\n");
    if api.is_empty() {
        out.push_str(NONE_IN_FULL_FILE);
    } else {
        out.push_str(&api);
    }
    out.push_str("\n--- Risk Markers ---\n");
    if risk.is_empty() {
        out.push_str(NONE_IN_FULL_FILE);
    } else {
        out.push_str(&risk);
    }

    let kept = imports.lines().count() + api.lines().count() + risk.lines().count();
    out.push_str(&omitted_note(content.lines().count(), kept));
    out.trim().to_string()
}

fn distill_js_ts_file(content: &str) -> String {
    let mut out = String::new();
    out.push_str("--- Imports ---\n");
    let mut imports = String::new();
    let mut api = String::new();
    let mut risk = String::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let num = i + 1;
        if trimmed.starts_with("import ") {
            imports.push_str(&format!("{} | {}\n", num, line));
        } else if trimmed.starts_with("export ")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("interface ")
            || trimmed.starts_with("type ")
            || (trimmed.starts_with("const ") && trimmed.contains("=>"))
        {
            api.push_str(&format!("{} | {}\n", num, line));
        } else if trimmed.contains("TODO")
            || trimmed.contains("FIXME")
            || trimmed.contains("console.error")
        {
            risk.push_str(&format!("{} | {}\n", num, line));
        }
    }
    if imports.is_empty() {
        out.push_str(NONE_IN_FULL_FILE);
    } else {
        out.push_str(&imports);
    }
    out.push_str("\n--- Public API / Structure ---\n");
    if api.is_empty() {
        out.push_str(NONE_IN_FULL_FILE);
    } else {
        out.push_str(&api);
    }
    out.push_str("\n--- Risk Markers ---\n");
    if risk.is_empty() {
        out.push_str(NONE_IN_FULL_FILE);
    } else {
        out.push_str(&risk);
    }

    let kept = imports.lines().count() + api.lines().count() + risk.lines().count();
    out.push_str(&omitted_note(content.lines().count(), kept));
    out.trim().to_string()
}

fn distill_go_file(content: &str) -> String {
    let mut out = String::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("func ")
            || trimmed.starts_with("type ")
            || trimmed.starts_with("var ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("package ")
            || trimmed.starts_with("import")
        {
            out.push_str(&format!("{} | {}\n", i + 1, line));
        }
    }
    if out.is_empty() {
        distill_unknown_file(content)
    } else {
        let kept = out.lines().count();
        out.push_str(&omitted_note(content.lines().count(), kept));
        out.trim().to_string()
    }
}

fn distill_java_file(content: &str) -> String {
    let mut out = String::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if (trimmed.contains("class ")
            || trimmed.contains("interface ")
            || trimmed.contains("public ")
            || trimmed.contains("private ")
            || trimmed.contains("protected ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("package "))
            && !trimmed.starts_with("//")
            && !trimmed.is_empty()
        {
            out.push_str(&format!("{} | {}\n", i + 1, line));
        }
    }
    if out.is_empty() {
        distill_unknown_file(content)
    } else {
        let kept = out.lines().count();
        out.push_str(&omitted_note(content.lines().count(), kept));
        out.trim().to_string()
    }
}

fn distill_json_file(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total <= 30 {
        return content.trim().to_string();
    }
    let head: Vec<&str> = lines.iter().take(15).copied().collect();
    format!(
        "{}\n... [{} more lines — full JSON omitted]",
        head.join("\n"),
        total - 15
    )
}

fn distill_config_file(content: &str, ext: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total <= 40 {
        return content.trim().to_string();
    }
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if (ext == "toml"
            && (trimmed.starts_with('[')
                || (!trimmed.starts_with('#')
                    && trimmed.contains('=')
                    && !trimmed.starts_with(' '))))
            || (matches!(ext, "yaml" | "yml")
                && !trimmed.starts_with(' ')
                && !trimmed.starts_with('#')
                && trimmed.ends_with(':'))
        {
            out.push_str(&format!("{} | {}\n", i + 1, line));
        }
    }
    if out.is_empty() {
        distill_unknown_file(content)
    } else {
        format!("[Config structure — {} lines total]\n{}", total, out.trim())
    }
}

fn distill_log_file(content: &str) -> String {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut error_lines: Vec<String> = vec![];
    for (i, line) in content.lines().enumerate() {
        let l = line.to_lowercase();
        if l.contains("error") || l.contains("fatal") || l.contains("panic") {
            errors += 1;
            error_lines.push(format!("{} | {}", i + 1, line));
        } else if l.contains("warn") {
            warnings += 1;
        }
    }
    let total = content.lines().count();
    let mut out = format!(
        "Log: {} errors, {} warnings ({} total lines)\n",
        errors, warnings, total
    );
    for err in error_lines.iter().take(10) {
        out.push_str(err);
        out.push('\n');
    }
    if errors > 10 {
        out.push_str(&format!("... [{} more error lines]\n", errors - 10));
    }
    out.trim().to_string()
}

fn distill_unknown_file(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total <= 30 {
        return content.trim().to_string();
    }
    let head: Vec<String> = lines
        .iter()
        .take(15)
        .enumerate()
        .map(|(i, l)| format!("{} | {}", i + 1, l))
        .collect();
    let tail: Vec<String> = lines
        .iter()
        .enumerate()
        .rev()
        .take(5)
        .map(|(i, l)| format!("{} | {}", i + 1, l))
        .collect();
    let tail_rev: Vec<String> = tail.into_iter().rev().collect();
    format!(
        "--- HEAD ({} total lines) ---\n{}\n... [{} lines omitted] ...\n--- TAIL ---\n{}",
        total,
        head.join("\n"),
        total - 20,
        tail_rev.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readfile_passthrough_when_below_token_threshold() {
        let content = "pub fn a() {}\n";
        assert!(distill_readfile(content, "src/lib.rs").is_none());
    }

    /// Enough repeated bodies to clear MIN_DISTILL_TOKENS in any language.
    fn bulky(unit: &str) -> String {
        unit.repeat(200)
    }

    /// #176: every code distiller drops the lines it did not select, so every
    /// one of them must say how many went. Four of the five said nothing, and a
    /// silently returned skeleton is the #111 never-drop violation.
    #[test]
    fn every_code_distiller_reports_how_many_lines_it_dropped() {
        let cases = [
            (
                "billing.py",
                bulky(
                    "def process(o):\n    if o.total > 1000:\n        o.discount(0.1)\n    return o\n",
                ),
            ),
            (
                "billing.ts",
                bulky(
                    "export function process(o) {\n  if (o.total > 1000) {\n    o.discount(0.1);\n  }\n}\n",
                ),
            ),
            (
                "billing.go",
                bulky(
                    "func Process(o Order) Order {\n\tif o.Total > 1000 {\n\t\to.Discount(0.1)\n\t}\n\treturn o\n}\n",
                ),
            ),
            (
                "Billing.java",
                bulky(
                    "public Order process(Order o) {\n    if (o.total > 1000) {\n        o.discount(0.1);\n    }\n    return o;\n}\n",
                ),
            ),
            (
                "billing.rs",
                bulky(
                    "pub fn process(o: Order) -> Order {\n    if o.total > 1000 {\n        o.discount(0.1);\n    }\n    o\n}\n",
                ),
            ),
        ];

        for (path, content) in cases {
            let out = distill_readfile(&content, path)
                .unwrap_or_else(|| panic!("{path} should distill at this size"));
            assert!(
                out.contains("lines omitted"),
                "{path} dropped lines without saying so:\n{out}"
            );
        }
    }

    /// The count is the point — "output was truncated" does not let a reader
    /// judge whether to re-read, a number does.
    #[test]
    fn states_the_omitted_line_count_against_the_file_total() {
        let content = bulky("def f(o):\n    return o.total * 2\n");
        let total = content.lines().count();

        let out = distill_readfile(&content, "a.py").unwrap();

        assert!(
            out.contains(&format!("of {total} lines omitted")),
            "expected a count against {total} total, got:\n{out}"
        );
    }

    #[test]
    fn says_nothing_when_no_lines_were_dropped() {
        assert_eq!(omitted_note(10, 10), "");
        assert_eq!(omitted_note(10, 99), "", "kept > total must not underflow");
        assert_eq!(omitted_note(0, 0), "");
    }

    /// The section scans the whole file, including the lines it then drops, so
    /// an empty section means absent from the file — not merely absent from
    /// what is shown. Next to a visibly truncated body a bare `None` cannot
    /// convey which.
    #[test]
    fn qualifies_an_empty_section_as_covering_the_whole_file() {
        let content = bulky("def f(o):\n    return o.total * 2\n");

        let out = distill_readfile(&content, "a.py").unwrap();

        assert!(out.contains("--- Risk Markers ---"));
        assert!(out.contains("None in the full file"), "got:\n{out}");
    }

    #[test]
    fn readfile_distills_when_above_token_threshold_even_if_few_lines() {
        let mut content = String::from("pub fn a() {}\n");
        for _ in 0..9 {
            content.push_str("// ");
            content.push_str(&"a".repeat(3000));
            content.push('\n');
        }

        let out = distill_readfile(&content, "src/lib.rs");
        assert!(out.is_some());
    }
}
