use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DiagnosisCategory {
    Success,
    Suboptimal,
    FailedSignalDropped,
    Passthrough,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceDiagnosis {
    pub category: DiagnosisCategory,
    pub compression_ratio: f32,
    pub dropped_critical_lines: usize,
    pub feedback_notes: Vec<String>,
}

/// Analyzes an execution trace to determine the quality of the distillation.
pub fn analyze_trace(raw_input: &str, distilled_output: &str, _command: &str) -> TraceDiagnosis {
    let raw_len = raw_input.len().max(1) as f32;
    let dist_len = distilled_output.len() as f32;
    let compression_ratio = 1.0 - (dist_len / raw_len);

    let mut feedback_notes = Vec::new();
    let mut dropped_critical_lines = 0;

    // Detect critical signals in raw input
    let critical_indicators = [
        "error:",
        "error[",
        "FAIL:",
        "panic:",
        "fatal:",
        "Exception:",
        "Traceback",
    ];
    let mut raw_critical = Vec::new();
    for line in raw_input.lines() {
        if critical_indicators.iter().any(|&ind| line.contains(ind)) {
            raw_critical.push(line.trim());
        }
    }

    // F-09: Check if critical signals exist in distilled output
    // Use a 40-char fingerprint prefix to handle formatting changes
    for crit in raw_critical {
        let check_len = crit.len().min(40); // first 40 chars as fingerprint
        let fingerprint = &crit[..check_len];
        if !distilled_output.contains(fingerprint) {
            dropped_critical_lines += 1;
            feedback_notes.push(format!(
                "Critical signal may be dropped: '{}'...",
                &crit[..crit.len().min(60)]
            ));
        }
    }

    let category = if dropped_critical_lines > 0 {
        feedback_notes.push(format!(
            "Dropped {} critical error/fail lines",
            dropped_critical_lines
        ));
        DiagnosisCategory::FailedSignalDropped
    } else if compression_ratio < 0.1 && raw_input.len() > 1000 {
        feedback_notes.push("Passthrough: Very low compression on large trace".to_string());
        DiagnosisCategory::Passthrough
    } else if compression_ratio > 0.2 || raw_input.len() < 1000 {
        DiagnosisCategory::Success
    } else {
        feedback_notes.push("Suboptimal: Low compression ratio".to_string());
        DiagnosisCategory::Suboptimal
    };

    TraceDiagnosis {
        category,
        compression_ratio,
        dropped_critical_lines,
        feedback_notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_trace_success() {
        let raw = "Building...\nerror: missing semicolon\naborted.";
        let distilled = "error: missing semicolon";
        let diag = analyze_trace(raw, distilled, "cargo build");
        assert_eq!(diag.category, DiagnosisCategory::Success);
        assert_eq!(diag.dropped_critical_lines, 0);
    }

    #[test]
    fn test_analyze_trace_signal_dropped() {
        let raw = "Building...\nerror[E001]: bad type\nnoise\nmore noise";
        let distilled = "Building...\nnoise\nmore noise";
        let diag = analyze_trace(raw, distilled, "cargo build");
        assert_eq!(diag.category, DiagnosisCategory::FailedSignalDropped);
        assert_eq!(diag.dropped_critical_lines, 1);
    }

    #[test]
    fn test_analyze_trace_passthrough() {
        let raw = "a".repeat(1500);
        let distilled = "a".repeat(1500);
        let diag = analyze_trace(&raw, &distilled, "cat file.txt");
        assert_eq!(diag.category, DiagnosisCategory::Passthrough);
    }

    #[test]
    fn test_analyze_trace_detects_dropped_error() {
        let raw = "error[E0308]: mismatched types\n  --> src/main.rs:42\nnormal output\n";
        let distilled = "Build output processed\nnormal output\n"; // error missing!

        let diagnosis = analyze_trace(raw, distilled, "cargo build");
        assert_eq!(diagnosis.category, DiagnosisCategory::FailedSignalDropped);
        assert!(diagnosis.dropped_critical_lines > 0);
        assert!(
            diagnosis
                .feedback_notes
                .iter()
                .any(|n| n.contains("Critical signal")),
            "Must include feedback about dropped signal"
        );
    }

    #[test]
    fn test_analyze_trace_ok_when_error_preserved() {
        let raw = "error[E0308]: mismatched types\n  --> src/main.rs:42\n";
        let distilled = "Build: 1 errors, 0 warnings\nerror[E0308]: mismatched types\n";

        let diagnosis = analyze_trace(raw, distilled, "cargo build");
        assert_eq!(diagnosis.dropped_critical_lines, 0);
        assert_ne!(diagnosis.category, DiagnosisCategory::FailedSignalDropped);
    }
}
