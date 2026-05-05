pub fn distill_readfile(content: &str, filepath: &str) -> Option<String> {
    distill_readfile_with_context(content, filepath, 0)
}

/// `imported_by_count`: number of files that import this file (from graph).
/// When > 3, append a factual warning suggesting omni_context.
pub fn distill_readfile_with_context(
    content: &str,
    filepath: &str,
    imported_by_count: usize,
) -> Option<String> {
    let line_count = content.lines().count();
    if line_count < 50 {
        return None; // Small files pass through
    }

    let ext = std::path::Path::new(filepath)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

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
        out.push_str("None\n");
    } else {
        out.push_str(&imports);
    }
    out.push_str("\n--- Public API / Structure ---\n");
    if api.is_empty() {
        out.push_str("None\n");
    } else {
        out.push_str(&api);
    }
    out.push_str("\n--- Risk Markers (TODOs, panics) ---\n");
    if risk.is_empty() {
        out.push_str("None\n");
    } else {
        out.push_str(&risk);
    }

    out.push_str("\n... [Method bodies omitted. Use Read with offset/limit or Edit directly] ...");
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
        out.push_str("None\n");
    } else {
        out.push_str(&imports);
    }
    out.push_str("\n--- Public API / Structure ---\n");
    if api.is_empty() {
        out.push_str("None\n");
    } else {
        out.push_str(&api);
    }
    out.push_str("\n--- Risk Markers ---\n");
    if risk.is_empty() {
        out.push_str("None\n");
    } else {
        out.push_str(&risk);
    }

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
        out.push_str("None\n");
    } else {
        out.push_str(&imports);
    }
    out.push_str("\n--- Public API / Structure ---\n");
    if api.is_empty() {
        out.push_str("None\n");
    } else {
        out.push_str(&api);
    }
    out.push_str("\n--- Risk Markers ---\n");
    if risk.is_empty() {
        out.push_str("None\n");
    } else {
        out.push_str(&risk);
    }

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
