use crate::distillers::Distiller;
use crate::pipeline::{ContentType, OutputSegment, SignalTier};
use std::collections::BTreeMap;

pub struct JsTsDistiller;

impl Distiller for JsTsDistiller {
    fn content_type(&self) -> ContentType {
        ContentType::JsTs
    }

    fn distill(&self, segments: &[OutputSegment], input: &str) -> String {
        let lines: Vec<&str> = input.lines().collect();

        // Dispatch based on content analysis
        if is_vitest_output(&lines) {
            distill_vitest(input)
        } else if is_tsc_output(&lines) {
            distill_tsc(input)
        } else if is_playwright_output(&lines) {
            distill_playwright(input)
        } else if is_eslint_output(&lines) {
            distill_eslint(input)
        } else if is_prettier_output(&lines) {
            distill_prettier(input)
        } else {
            distill_fallback(segments)
        }
    }
}

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

fn is_vitest_output(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        l.contains("vitest")
            || l.contains("VITE v")
            || l.contains("Test Files")
            || l.contains("Tests  ")
    })
}

fn is_tsc_output(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        l.contains("error TS")
            || l.contains("tsc --")
            || l.contains("Found errors")
            || l.contains("Found ") && l.contains(" error")
    })
}

fn is_playwright_output(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        l.contains("playwright")
            || l.contains("[chromium]")
            || l.contains("[firefox]")
            || l.contains("Running ") && l.contains(" tests")
    })
}

fn is_eslint_output(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        l.contains("eslint")
            || l.contains(" problems (")
            || (l.contains("error") && l.contains("@typescript-eslint"))
    })
}

fn is_prettier_output(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        l.contains("prettier")
            || l.contains("checking ") && l.contains(" files")
            || l.contains("reformatted ") && l.contains(" files")
    })
}

// ---------------------------------------------------------------------------
// vitest
// ---------------------------------------------------------------------------

fn distill_vitest(input: &str) -> String {
    let mut passed_tests = 0;
    let mut failed_tests = 0;
    let mut total_tests = 0;
    let mut has_summary = false;

    let mut failed_details: Vec<String> = Vec::new();

    let lines: Vec<&str> = input.lines().collect();

    // Attempt to parse formal summary first
    for line in &lines {
        let t = line.trim();
        if t.starts_with("Tests  ") {
            has_summary = true;
            // E.g., "Tests  3 failed | 48 passed (51)"
            let parts: Vec<&str> = t.split('|').collect();
            for part in parts {
                if part.contains("passed")
                    && let Some(num) = part.split_whitespace().find_map(|s| s.parse::<u32>().ok())
                {
                    passed_tests = num;
                }
                if part.contains("failed")
                    && let Some(num) = part.split_whitespace().find_map(|s| s.parse::<u32>().ok())
                {
                    failed_tests = num;
                }
            }
            // Parse total from "(51)" if present
            if let Some(start) = t.find('(')
                && let Some(end) = t[start..].find(')')
                && let Ok(num) = t[start + 1..start + end].trim().parse::<u32>()
            {
                total_tests = num;
            }
        }

        // Find failed tests: " ✗ src/services/__tests__/api.test.ts:47:12" or "   ✗ should handle rate limiting"
        // Look for deeper trace points
        if t.starts_with('❯') && t.contains(':') {
            let trace = t.trim_start_matches('❯').trim();
            // take basename:line
            if let Some(slash_idx) = trace.rfind('/') {
                let rest = &trace[slash_idx + 1..];
                // if it looks like file:line:col
                let mut parts = rest.split(':');
                if let Some(file) = parts.next()
                    && let Some(line) = parts.next()
                {
                    failed_details.push(format!("{}:{}", file, line));
                }
            } else {
                // fallback if no slash
                let mut parts = trace.split(':');
                if let Some(file) = parts.next()
                    && let Some(line) = parts.next()
                {
                    failed_details.push(format!("{}:{}", file, line));
                }
            }
        }
    }

    if !has_summary {
        // Fallback: count from lines
        for line in &lines {
            // Count passes and fails heuristically if no summary
            if line.contains(" ✓ ") {
                passed_tests += 1;
            }
            if *line != " ✗ " && line.contains(" ✗ ") && !line.contains("failed |") {
                failed_tests += 1;
            }
        }
        total_tests = passed_tests + failed_tests;
    }

    if total_tests == 0 {
        total_tests = passed_tests + failed_tests;
    }

    if failed_tests == 0 && failed_details.is_empty() {
        return format!("vitest: ✓ {}/{} passed", passed_tests, total_tests);
    }

    // Deduplicate failed_details
    let mut unique_fails = Vec::new();
    for f in failed_details {
        if !unique_fails.contains(&f) {
            unique_fails.push(f);
        }
    }

    let fail_count_display = if failed_tests > 0 {
        failed_tests
    } else {
        unique_fails.len() as u32
    };

    let mut out = format!(
        "vitest: ✓ {}/{} | ✗ {}",
        passed_tests, total_tests, fail_count_display
    );

    if !unique_fails.is_empty() {
        let shown: Vec<String> = unique_fails.into_iter().take(5).collect();
        out.push_str(&format!(" [{}]", shown.join(", ")));
        // Could add +N but spec just says show them. We show up to 5 implicitly.
    }

    out
}

// ---------------------------------------------------------------------------
// TypeScript Compiler (TSC)
// ---------------------------------------------------------------------------

fn distill_tsc(input: &str) -> String {
    let mut by_file: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut total_errors = 0;

    for line in input.lines() {
        let t = line.trim();
        // Check for error line like "src/components/Button.tsx(10,5): error TS2741: Property 'onClick' is missing"
        // Or "error TS2307: Cannot find module './utils' in 'src/app.ts'"

        if let Some(ts_idx) = t.find("error TS") {
            total_errors += 1;

            // Try to extract file and line
            let mut file_display = String::new();
            let mut issue_display = String::new();

            // Format 1: file(line,col): error TS####...
            if ts_idx > 0 && t[..ts_idx].contains("): ") {
                let prefix = &t[..ts_idx];
                if let Some(paren_idx) = prefix.find('(') {
                    let file = prefix[..paren_idx].trim();
                    let basename = file.rsplit('/').next().unwrap_or(file);
                    file_display = basename.to_string();

                    let line_num = prefix[paren_idx + 1..].split(',').next().unwrap_or("");

                    // Extract TS code
                    let rest = &t[ts_idx..];
                    let ts_code = rest.split(':').next().unwrap_or("").replace("error ", "");

                    issue_display = format!("{}:l{}", ts_code, line_num);
                }
            } else {
                // Format 2: error TS####: ... in 'file.ts'
                let rest = &t[ts_idx..];
                let mut parts = rest.split(':');
                let ts_code = parts.next().unwrap_or("").replace("error ", "");

                if let Some(in_idx) = t.rfind(" in '") {
                    let file_part = &t[in_idx + 5..];
                    let file = file_part.trim_end_matches('\'');
                    let basename = file.rsplit('/').next().unwrap_or(file);
                    file_display = basename.to_string();
                    issue_display = ts_code;
                } else {
                    file_display = "unknown".to_string();
                    issue_display = ts_code;
                }
            }

            if !file_display.is_empty() {
                by_file.entry(file_display).or_default().push(issue_display);
            }
        } else if t.starts_with("Found ") && t.contains(" error") {
            // "Found 5 errors"
            if let Some(num) = t
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u32>().ok())
                && total_errors == 0
            {
                total_errors = num; // fallback if we couldn't parse individual lines
            }
        }
    }

    if total_errors == 0 {
        return "tsc: no errors".to_string();
    }

    let file_count = by_file.len();
    let mut out = format!("tsc: {} errors in {} files", total_errors, file_count);

    let mut sorted: Vec<(String, Vec<String>)> = by_file.into_iter().collect();
    // Sort by number of errors descending
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    for (file, issues) in sorted.iter().take(5) {
        let count = issues.len();
        let issues_str = issues.join(", ");
        let truncated = if issues_str.len() > 60 {
            format!("{}...", &issues_str[..57])
        } else {
            issues_str
        };
        out.push_str(&format!("\n  {}: {} errors [{}]", file, count, truncated));
    }

    if sorted.len() > 5 {
        out.push_str(&format!("\n  +{} more files", sorted.len() - 5));
    }

    out
}

// ---------------------------------------------------------------------------
// Playwright
// ---------------------------------------------------------------------------

fn distill_playwright(input: &str) -> String {
    let mut passed = 0;
    let mut failed = 0;

    let mut fail_info: Vec<String> = Vec::new();

    let lines: Vec<&str> = input.lines().collect();

    // Collect specific failures
    for line in &lines {
        let t = line.trim();
        // Look for: ✗  9 [chromium] › tests/login.spec.ts:20:1 › submits valid credentials (5.0s)
        if t.starts_with('✗') && t.contains(" › ") {
            // Extract file:line and test name
            let parts: Vec<&str> = t.split(" › ").collect();
            if parts.len() >= 3 {
                let file_path = parts[1]; // tests/login.spec.ts:20:1
                let test_name = parts[2].split(" (").next().unwrap_or(parts[2]);

                // Keep just file basename and line
                let mut display_file = file_path.to_string();
                if let Some(slash_idx) = file_path.rfind('/') {
                    display_file = file_path[slash_idx + 1..].to_string();
                }

                // Strip the final column if any, e.g. login.spec.ts:20:1 -> login.spec.ts:20
                if display_file.matches(':').count() == 2
                    && let Some(last_colon) = display_file.rfind(':')
                {
                    display_file = display_file[..last_colon].to_string();
                }

                fail_info.push(format!("{}:{}", test_name, display_file));
            }
        }

        // Parse summary line: "  2 failed" or "  22 passed (45.2s)"
        if t.ends_with("passed") || (t.contains("passed (") && t.ends_with(")")) {
            if let Some(num) = t
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u32>().ok())
            {
                passed = num;
            }
        } else if (t.ends_with("failed") || (t.contains("failed (") && t.ends_with(")")))
            && let Some(num) = t
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u32>().ok())
        {
            failed = num;
        }
    }

    // If we missed summary but have individual lines
    if passed == 0 && failed == 0 {
        for line in &lines {
            if line.contains(" ✓ ") {
                passed += 1;
            }
            if line.contains(" ✗ ") {
                failed += 1;
            }
        }
    }

    let total = passed + failed;

    if failed == 0 {
        return format!("playwright: ✓ {}/{} passed", passed, total);
    }

    let mut out = format!("playwright: ✓ {}/{} | ✗ {}", passed, total, failed);

    if !fail_info.is_empty() {
        let shown: Vec<String> = fail_info.into_iter().take(3).collect();
        out.push_str(&format!(" [{}]", shown.join(", ")));
    }

    out
}

// ---------------------------------------------------------------------------
// ESLint
// ---------------------------------------------------------------------------

fn distill_eslint(input: &str) -> String {
    let mut total_errors = 0;
    let mut total_warnings = 0;
    let mut by_rule: BTreeMap<String, u32> = BTreeMap::new();
    let mut files_affected: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in input.lines() {
        let t = line.trim();

        // Skip empty or summary lines
        if t.is_empty() || t.starts_with("✖") || t.starts_with("Checking") {
            // But still parse summary counts
            if t.contains("problems (") {
                if let Some(err_idx) = t.find(" errors") {
                    if let Some(start) = t[..err_idx].rfind('(') {
                        if let Ok(n) = t[start + 1..err_idx].trim().parse::<u32>() {
                            total_errors = n;
                        }
                    } else if let Some(start) = t[..err_idx].rfind(' ')
                        && let Ok(n) = t[start + 1..err_idx].trim().parse::<u32>()
                    {
                        total_errors = n;
                    }
                } else if let Some(err_idx) = t.find(" error")
                    && let Some(start) = t[..err_idx].rfind('(')
                    && let Ok(n) = t[start + 1..err_idx].trim().parse::<u32>()
                {
                    total_errors = n;
                }

                if let Some(warn_idx) = t.find(" warnings") {
                    if let Some(start) = t[..warn_idx].rfind(" ")
                        && let Ok(n) = t[start + 1..warn_idx].trim().parse::<u32>()
                    {
                        total_warnings = n;
                    }
                } else if let Some(warn_idx) = t.find(" warning")
                    && let Some(start) = t[..warn_idx].rfind(", ")
                    && let Ok(n) = t[start + 2..warn_idx].trim().parse::<u32>()
                {
                    total_warnings = n;
                }
            }
            continue;
        }

        // Standard formatter grouping (file path on its own line)
        if !t.contains(" error ")
            && !t.contains(" warning ")
            && (t.contains('/') || t.contains('\\'))
            && !t.contains(' ')
        {
            files_affected.insert(t.to_string());
        }

        // Inline formatter (file:line:col error ...)
        if let Some(colon_idx) = t.find(':')
            && (t.contains(" error ") || t.contains(" warning "))
        {
            let file_path = &t[..colon_idx];
            if file_path.contains('/') || file_path.contains('.') || file_path.contains('\\') {
                files_affected.insert(file_path.to_string());
            }
        }

        // Parse individual rules: "src/index.ts:10:15 error Unexpected console statement @typescript-eslint/no-console"
        if t.contains(" error ") || t.contains(" warning ") {
            let parts = t.split_whitespace();
            if let Some(last) = parts.last()
                && (last.contains('/') || last.contains('-'))
            {
                // Looks like a rule name
                *by_rule.entry(last.to_string()).or_insert(0) += 1;
            }
        }
    }

    if total_errors == 0 && total_warnings == 0 {
        return "eslint: no problems found".to_string();
    }

    let mut out = format!(
        "eslint: {} errors, {} warnings in {} files",
        total_errors,
        total_warnings,
        files_affected.len()
    );

    if !by_rule.is_empty() {
        let mut sorted: Vec<(String, u32)> = by_rule.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        out.push_str("\n  top rules: ");
        let rules_str: Vec<String> = sorted
            .iter()
            .take(3)
            .map(|(r, c)| format!("{}: {}", r, c))
            .collect();
        out.push_str(&rules_str.join(", "));
    }

    out
}

// ---------------------------------------------------------------------------
// Prettier
// ---------------------------------------------------------------------------

fn distill_prettier(input: &str) -> String {
    let mut reformatted = 0;
    let mut unchanged = 0;

    for line in input.lines() {
        let t = line.trim();
        // Look for summary lines
        if t.contains("reformatted ")
            && t.contains(" files")
            && let Some(num) = t.split_whitespace().find_map(|s| s.parse::<u32>().ok())
        {
            reformatted = num;
        }
        // In some output, it mentions "unchanged"
        if t.contains(" unchanged")
            && let Some(num) = t.split_whitespace().find_map(|s| s.parse::<u32>().ok())
        {
            unchanged = num;
        }
    }

    format!(
        "prettier: {} files reformatted, {} unchanged",
        reformatted, unchanged
    )
}

// ---------------------------------------------------------------------------
// Fallback
// ---------------------------------------------------------------------------

fn distill_fallback(segments: &[OutputSegment]) -> String {
    let mut out = String::new();
    let mut lines = 0;

    for seg in segments {
        if matches!(seg.tier, SignalTier::Critical | SignalTier::Important) {
            for line in seg.content.lines() {
                if lines >= 30 {
                    break;
                }
                out.push_str(line);
                out.push('\n');
                lines += 1;
            }
        }
        if lines >= 30 {
            break;
        }
    }

    if out.trim().is_empty() {
        for seg in segments.iter().take(10) {
            out.push_str(&seg.content);
            out.push('\n');
        }
    }

    out.trim().to_string()
}
