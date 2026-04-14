use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};

pub struct DatabaseDistiller;

impl Distiller for DatabaseDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> String {
        // Detect apakah ini query result, error, atau migration output
        if input.contains("ERROR:") || input.contains("FATAL:") || input.contains("error:") {
            distill_db_error(input)
        } else if input.contains("rows)") || input.contains("row)") || looks_like_table(input) {
            distill_query_result(input)
        } else {
            distill_db_generic(segments, input)
        }
    }
}

fn distill_db_error(input: &str) -> String {
    let mut errors: Vec<String> = vec![];
    let mut hint: Option<String> = None;
    let mut position: Option<String> = None;

    for line in input.lines() {
        let l = line.trim();
        let l_lower = l.to_lowercase();
        // Semantic matching (resilient to timestamp prefixes, container logs, etc)
        if l_lower.contains("error:") || l_lower.contains("fatal:") || l_lower.contains("error ") {
            errors.push(l.to_string());
        } else if l_lower.contains("hint:") || l_lower.contains("detail:") {
            hint = Some(l.to_string());
        } else if l_lower.contains("line ") || l_lower.contains("position:") {
            position = Some(l.to_string());
        }
    }

    let mut out = format!("DB Error ({} found):\n", errors.len());
    for e in errors.iter().take(3) {
        out.push_str(e);
        out.push('\n');
    }
    if let Some(p) = position {
        out.push_str(&p);
        out.push('\n');
    }
    if let Some(h) = hint {
        out.push_str(&h);
        out.push('\n');
    }
    out.trim().to_string()
}

fn distill_query_result(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let total = lines.len();

    // Cari baris "N rows"
    let row_summary = lines
        .iter()
        .rev()
        .take(5)
        .find(|l| l.contains("row") && (l.contains('(') || l.trim().parse::<usize>().is_ok()))
        .map(|l| l.trim().to_string());

    // Header (kolom) biasanya baris pertama non-empty
    let header = lines
        .iter()
        .find(|l| {
            let lt = l.trim();
            !lt.is_empty() && !lt.contains("---") && !lt.contains("(") && !lt.contains("rows")
        })
        .map(|l| l.trim().to_string());

    let mut out = String::new();
    if let Some(h) = &header {
        out.push_str(&format!("Query result columns: {}\n", h));
    }
    if let Some(summary) = row_summary {
        out.push_str(&format!("Result: {}\n", summary));
    } else {
        out.push_str(&format!("Result: {} lines output\n", total));
    }
    // Show first 3 data rows as sample
    let data_rows: Vec<&str> = lines
        .iter()
        .filter(|l| {
            let lt = l.trim();
            !lt.is_empty() && !lt.contains("---") && !lt.contains("(") && !lt.contains("rows")
        })
        .skip(1) // skip header
        .take(3)
        .copied()
        .collect();
    if !data_rows.is_empty() {
        out.push_str("Sample rows:\n");
        for row in &data_rows {
            out.push_str(row);
            out.push('\n');
        }
        if total > data_rows.len() + 2 {
            out.push_str(&format!(
                "  ... [{} more rows]\n",
                total - data_rows.len() - 2
            ));
        }
    }
    out.trim().to_string()
}

fn distill_db_generic(segments: &[OutputSegment], _input: &str) -> String {
    let errors: Vec<&str> = segments
        .iter()
        .filter(|s| s.tier == SignalTier::Critical)
        .map(|s| s.content.as_str())
        .collect();
    if errors.is_empty() {
        format!("DB: ok ({} lines output)", segments.len())
    } else {
        format!(
            "DB errors: {}\n{}",
            errors.len(),
            errors
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

fn looks_like_table(input: &str) -> bool {
    input
        .lines()
        .take(5)
        .any(|l| l.contains(" | ") || l.contains("---") || l.contains("+--"))
}
