pub mod classifier;
pub mod composer;
pub mod scorer;
pub mod toml_filter;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

// 1. Content type — hasil Stage 1 classifier
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContentType {
    GitDiff,
    GitStatus,
    GitLog,
    BuildOutput,    // cargo, npm, pip, make
    TestOutput,     // pytest, cargo test, vitest, go test
    InfraOutput,    // kubectl, terraform, helm, docker
    LogOutput,      // access log, error log, syslog
    TabularData,    // kubectl get pods (table format)
    StructuredData, // JSON output for CLI
    Unknown,
}

// 2. Signal tier — how important this segment is
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignalTier {
    Noise,     // Progress, compiling boring deps — drop
    Context,   // Supporting lines — include if space allows
    Important, // Warning, changed file — biasanya include
    Critical,  // Error, exception, FAILED — selalu include
}

// 3. Route — path distilasi
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Route {
    Keep,        // score >= 0.7, full distillation
    Soft,        // 0.3–0.69, labeled distillation
    Passthrough, // < 0.3, raw + learn trigger
    Rewind,      // aggressively compressed, stored in RewindStore
    Error,       // engine error, raw preserved
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Route::Keep => write!(f, "Keep"),
            Route::Soft => write!(f, "Soft"),
            Route::Passthrough => write!(f, "Passthrough"),
            Route::Rewind => write!(f, "Rewind"),
            Route::Error => write!(f, "Error"),
        }
    }
}

// Implement Display for ContentType (optional but useful for logging)
impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// Implement Display for SignalTier (optional but useful for logging)
impl fmt::Display for SignalTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// 4. Output segment
#[derive(Debug, Clone)]
pub struct OutputSegment {
    pub content: String,
    pub tier: SignalTier,
    pub base_score: f32,
    pub context_score: f32, // boost for session context
    pub line_range: (usize, usize),
}

impl OutputSegment {
    pub fn final_score(&self) -> f32 {
        (self.base_score + self.context_score).clamp(0.0, 1.0)
    }

    pub fn mentions(&self, path: &str) -> bool {
        self.content.contains(path)
    }

    pub fn is_diagnostic(&self) -> bool {
        matches!(self.tier, SignalTier::Critical | SignalTier::Important)
    }
}

// 5. Distillation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillResult {
    pub output: String,
    pub route: Route,
    pub filter_name: String,
    pub content_type: ContentType,
    pub score: f32,
    pub context_score: f32, // for session scorer
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub latency_ms: u64,
    pub rewind_hash: Option<String>, // if content is in RewindStore
    pub segments_kept: usize,
    pub segments_dropped: usize,
}

impl DistillResult {
    pub fn savings_pct(&self) -> f64 {
        if self.input_bytes == 0 {
            return 0.0;
        }
        (1.0 - self.output_bytes as f64 / self.input_bytes as f64) * 100.0
    }

    pub fn is_meaningful(&self) -> bool {
        // Return false if there is no significant compression (< 10%)
        self.output_bytes < (self.input_bytes as f64 * 0.90) as usize
    }
}

// 6. Session state (minimal for v0.5.0)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: String,
    pub started_at: i64,
    pub last_active: i64,

    // Inferred context
    pub inferred_task: Option<String>,   // "fix auth bug"
    pub inferred_domain: Option<String>, // "authentication"

    // Hot files (path → access count)
    pub hot_files: BTreeMap<String, u32>,

    // Recent errors to boost relevance
    pub active_errors: Vec<String>, // last 5 error messages

    // Command history
    pub command_count: u32,
    pub last_commands: Vec<String>, // last 20 commands
}

impl SessionState {
    pub fn new() -> Self {
        let id = format!("{}", chrono::Utc::now().timestamp_millis());
        let now = chrono::Utc::now().timestamp();
        Self {
            session_id: id,
            started_at: now,
            last_active: now,
            ..Default::default()
        }
    }

    // Score boost from session context for a text
    pub fn context_boost(&self, text: &str) -> f32 {
        let mut boost = 0.0f32;
        // Boost if mentioning hot file
        for (path, count) in &self.hot_files {
            if text.contains(path) {
                boost += 0.1 * (*count as f32 / 10.0).min(0.3);
            }
        }
        // Boost if mentioning active error
        for err in &self.active_errors {
            let err_short = &err[..err.len().min(30)];
            if text.contains(err_short) {
                boost += 0.25;
            }
        }
        boost.min(0.4)
    }

    pub fn add_hot_file(&mut self, path: &str) {
        *self.hot_files.entry(path.to_string()).or_insert(0) += 1;
    }

    pub fn add_error(&mut self, error: &str) {
        self.active_errors
            .insert(0, error[..error.len().min(200)].to_string());
        self.active_errors.truncate(5);
    }

    pub fn add_command(&mut self, cmd: &str) {
        self.command_count += 1;
        self.last_commands.insert(0, cmd.to_string());
        self.last_commands.truncate(20);
        self.last_active = chrono::Utc::now().timestamp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_display_formatting_correct() {
        assert_eq!(format!("{}", Route::Keep), "Keep");
        assert_eq!(format!("{}", Route::Soft), "Soft");
        assert_eq!(format!("{}", Route::Passthrough), "Passthrough");
        assert_eq!(format!("{}", Route::Rewind), "Rewind");
        assert_eq!(format!("{}", Route::Error), "Error");
    }

    #[test]
    fn test_distill_result_savings_pct_calculation() {
        let res = DistillResult {
            output: String::new(),
            route: Route::Keep,
            filter_name: String::new(),
            content_type: ContentType::Unknown,
            score: 0.0,
            context_score: 0.0,
            input_bytes: 100,
            output_bytes: 25,
            latency_ms: 0,
            rewind_hash: None,
            segments_kept: 0,
            segments_dropped: 0,
        };
        assert_eq!(res.savings_pct(), 75.0);

        let res_zero = DistillResult {
            input_bytes: 0,
            output_bytes: 0,
            ..res
        };
        assert_eq!(res_zero.savings_pct(), 0.0);
    }

    #[test]
    fn test_distill_result_is_meaningful_threshold() {
        let mut res = DistillResult {
            output: String::new(),
            route: Route::Keep,
            filter_name: String::new(),
            content_type: ContentType::Unknown,
            score: 0.0,
            context_score: 0.0,
            input_bytes: 100,
            output_bytes: 89, // > 10% savings (89 < 90)
            latency_ms: 0,
            rewind_hash: None,
            segments_kept: 0,
            segments_dropped: 0,
        };
        assert!(res.is_meaningful());

        res.output_bytes = 90; // Exactly 10% savings (90 < 90 is false)
        assert!(!res.is_meaningful());

        res.output_bytes = 95; // < 10% savings
        assert!(!res.is_meaningful());
    }

    #[test]
    fn test_session_state_context_boost_dengan_hot_files() {
        let mut state = SessionState::new();
        state.add_hot_file("src/main.rs");
        // base count is 1 => boost = 0.1 * min(1/10, 0.3) = 0.01

        let text = "Error in src/main.rs at line 10";
        assert!((state.context_boost(text) - 0.01).abs() < f32::EPSILON);

        for _ in 0..19 {
            state.add_hot_file("src/main.rs");
        }
        // count is 20 => boost = 0.1 * min(20/10, 0.3) = 0.03
        // Float precision might cause issues here, so we check with a small delta.
        assert!((state.context_boost(text) - 0.03).abs() < f32::EPSILON);
    }

    #[test]
    fn test_session_state_context_boost_dengan_active_errors() {
        let mut state = SessionState::new();
        state.add_error("expected identifier, found keyword `fn`");

        let text1 = "compiler output: expected identifier, found keyword `fn`";
        assert_eq!(state.context_boost(text1), 0.25);

        // Multiple matches are not additive for errors individually within the method loop unless there are multiple different errors matched.
        let text2 = "something else";
        assert_eq!(state.context_boost(text2), 0.0);
    }

    #[test]
    fn test_output_segment_final_score_clamp_0_1() {
        let seg1 = OutputSegment {
            content: "test".to_string(),
            tier: SignalTier::Noise,
            base_score: 0.8,
            context_score: 0.5,
            line_range: (0, 1),
        };
        assert_eq!(seg1.final_score(), 1.0);

        let seg2 = OutputSegment {
            content: "test".to_string(),
            tier: SignalTier::Noise,
            base_score: -0.5,
            context_score: 0.1,
            line_range: (0, 1),
        };
        assert_eq!(seg2.final_score(), 0.0);

        let seg3 = OutputSegment {
            content: "test".to_string(),
            tier: SignalTier::Noise,
            base_score: 0.4,
            context_score: 0.2,
            line_range: (0, 1),
        };
        // Use an epsilon check due to potential binary representation artifacts of f32 addition
        assert!((seg3.final_score() - 0.6).abs() < f32::EPSILON);
    }
}
