//! ccusage wrapper — subprocess-based Claude Code spending analytics.
//!
//! Provides graceful degradation when the `ccusage` npm package is not
//! installed, and caches results at `~/.omni/ccusage_cache.json` with a
//! 15-minute TTL to avoid shelling out on every invocation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::omni_home;

// ─── Pricing weights (Feb 2026 API pricing) ────────────────

/// Output tokens cost 5× the input price.
const WEIGHT_OUTPUT: f64 = 5.0;
/// Cache-creation tokens cost 1.25× the input price.
const WEIGHT_CACHE_CREATE: f64 = 1.25;
/// Cache-read tokens cost 0.1× the input price.
const WEIGHT_CACHE_READ: f64 = 0.1;

/// Cache time-to-live in seconds (15 minutes).
const CACHE_TTL_SECS: u64 = 15 * 60;

/// Fallback cost-per-token ($3.00 per 1M tokens) used when ccusage
/// is available but hasn't recorded any usage yet.
const FALLBACK_CPT: f64 = 3.0 / 1_000_000.0;

// ─── Types ─────────────────────────────────────────────────

/// Raw token / cost metrics returned by the `ccusage` CLI.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CcusageMetrics {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
}

/// Aggregated economics for a time period (Today / This Week / All Time).
#[derive(Debug, Clone, Serialize)]
pub struct PeriodEconomics {
    /// Human label, e.g. "Today", "This Week", "All Time".
    pub label: String,

    // ── ccusage data (None when ccusage is unavailable) ─────
    pub cc_cost: Option<f64>,
    pub cc_input_tokens: Option<u64>,
    pub cc_output_tokens: Option<u64>,
    pub cc_cache_create_tokens: Option<u64>,
    pub cc_cache_read_tokens: Option<u64>,

    // ── OMNI savings data (from SQLite) ─────────────────────
    pub omni_commands: Option<usize>,
    pub omni_saved_bytes: Option<usize>,
    pub omni_reduction_pct: Option<f64>,

    // ── Computed (None when ccusage is unavailable) ─────────
    /// Weighted cost-per-token.
    pub weighted_input_cpt: Option<f64>,
    /// Estimated dollar savings = (omni_saved_bytes / 4) × weighted_cpt.
    pub dollar_saved: Option<f64>,
}

// ─── On-disk cache entry ───────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
struct CacheEntry {
    unix_ts: u64,
    metrics: CcusageMetrics,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

// ─── Core helpers ──────────────────────────────────────────

/// Returns `true` when a working `ccusage` binary is on `$PATH`.
pub fn is_ccusage_available() -> bool {
    std::process::Command::new("ccusage")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Compute a weighted cost-per-token that accounts for the different
/// pricing tiers of input / output / cache tokens.
///
/// Returns `None` when the total weighted units are negligible (< 1).
pub fn compute_weighted_cpt(metrics: &CcusageMetrics) -> Option<f64> {
    let weighted_units = metrics.input_tokens as f64
        + WEIGHT_OUTPUT * metrics.output_tokens as f64
        + WEIGHT_CACHE_CREATE * metrics.cache_creation_tokens as f64
        + WEIGHT_CACHE_READ * metrics.cache_read_tokens as f64;
    if weighted_units < 1.0 {
        return None;
    }
    Some(metrics.total_cost / weighted_units)
}

// ─── Subprocess fetchers ───────────────────────────────────

/// Fetch metrics for a single day via `ccusage daily --date <DATE> --json`.
pub fn fetch_daily(date: &str) -> Option<CcusageMetrics> {
    let output = std::process::Command::new("ccusage")
        .args(["daily", "--date", date, "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    Some(CcusageMetrics {
        input_tokens: json["inputTokens"].as_u64().unwrap_or(0),
        output_tokens: json["outputTokens"].as_u64().unwrap_or(0),
        cache_creation_tokens: json["cacheCreationTokens"].as_u64().unwrap_or(0),
        cache_read_tokens: json["cacheReadTokens"].as_u64().unwrap_or(0),
        total_tokens: json["totalTokens"].as_u64().unwrap_or(0),
        total_cost: json["totalCost"].as_f64().unwrap_or(0.0),
    })
}

// ─── Cache layer ───────────────────────────────────────────

fn cache_path() -> PathBuf {
    omni_home().join("ccusage_cache.json")
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_cache() -> CacheFile {
    let path = cache_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_cache(cache: &CacheFile) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        &path,
        serde_json::to_string_pretty(cache).unwrap_or_default(),
    );
}

/// Fetch metrics for a period label (e.g. `"today"`, `"week"`, `"all"`),
/// honouring a 15-minute disk cache.
///
/// Returns `None` when:
/// - `ccusage` is not installed (graceful degradation).
/// - The subprocess fails for any reason.
pub fn fetch_with_cache(since_label: &str) -> Option<CcusageMetrics> {
    // Fast-path: no binary → don't bother touching the cache.
    if !is_ccusage_available() {
        return None;
    }

    let now = now_unix();
    let mut cache = read_cache();

    // Check for a non-expired hit.
    if let Some(entry) = cache.entries.get(since_label)
        && now.saturating_sub(entry.unix_ts) < CACHE_TTL_SECS
    {
        return Some(entry.metrics.clone());
    }

    // Cache miss or stale — shell out to ccusage.
    let metrics = if since_label.eq_ignore_ascii_case("today") {
        let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
        fetch_daily(&today_str)?
    } else {
        let output = std::process::Command::new("ccusage")
            .args(["daily", "--since", since_label, "--json"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        CcusageMetrics {
            input_tokens: json["inputTokens"].as_u64().unwrap_or(0),
            output_tokens: json["outputTokens"].as_u64().unwrap_or(0),
            cache_creation_tokens: json["cacheCreationTokens"].as_u64().unwrap_or(0),
            cache_read_tokens: json["cacheReadTokens"].as_u64().unwrap_or(0),
            total_tokens: json["totalTokens"].as_u64().unwrap_or(0),
            total_cost: json["totalCost"].as_f64().unwrap_or(0.0),
        }
    };

    // Persist to cache.
    cache.entries.insert(
        since_label.to_string(),
        CacheEntry {
            unix_ts: now,
            metrics: metrics.clone(),
        },
    );
    write_cache(&cache);

    Some(metrics)
}

// ─── Economics builder ─────────────────────────────────────

/// Build a [`PeriodEconomics`] from OMNI savings data and optional ccusage
/// metrics.  When `cc_metrics` is `None` (ccusage not installed), the
/// CC-related and dollar-saved fields gracefully degrade to `None`.
pub fn build_period_economics(
    label: &str,
    omni_commands: usize,
    omni_saved_bytes: usize,
    omni_reduction_pct: f64,
    cc_metrics: Option<CcusageMetrics>,
) -> PeriodEconomics {
    let saved_tokens = omni_saved_bytes / 4;

    let (weighted_cpt, dollar_saved) = if let Some(m) = cc_metrics.as_ref() {
        // If we have actual usage, compute the weighted CPT.
        if let Some(cpt) = compute_weighted_cpt(m) {
            (Some(cpt), Some(saved_tokens as f64 * cpt))
        } else {
            // Available but no usage yet - use fallback CPT so user sees SOMETHING.
            (Some(FALLBACK_CPT), Some(saved_tokens as f64 * FALLBACK_CPT))
        }
    } else {
        // Binary completely missing.
        (None, None)
    };

    PeriodEconomics {
        label: label.to_string(),
        cc_cost: cc_metrics.as_ref().map(|m| m.total_cost),
        cc_input_tokens: cc_metrics.as_ref().map(|m| m.input_tokens),
        cc_output_tokens: cc_metrics.as_ref().map(|m| m.output_tokens),
        cc_cache_create_tokens: cc_metrics.as_ref().map(|m| m.cache_creation_tokens),
        cc_cache_read_tokens: cc_metrics.as_ref().map(|m| m.cache_read_tokens),
        omni_commands: Some(omni_commands),
        omni_saved_bytes: Some(omni_saved_bytes),
        omni_reduction_pct: Some(omni_reduction_pct),
        weighted_input_cpt: weighted_cpt,
        dollar_saved,
    }
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_weighted_cpt_normal() {
        let m = CcusageMetrics {
            input_tokens: 1000,
            output_tokens: 200,
            cache_creation_tokens: 500,
            cache_read_tokens: 1000,
            total_tokens: 2700,
            total_cost: 0.005,
        };
        let cpt = compute_weighted_cpt(&m);
        assert!(cpt.is_some());
        // weighted = 1000 + 5×200 + 1.25×500 + 0.1×1000
        //          = 1000 + 1000  + 625      + 100
        //          = 2725
        // cpt = 0.005 / 2725 ≈ 0.00000183
        let cpt_val = cpt.unwrap();
        assert!(cpt_val > 0.0 && cpt_val < 0.001);
    }

    #[test]
    fn test_compute_weighted_cpt_zero_tokens() {
        let m = CcusageMetrics {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            total_tokens: 0,
            total_cost: 0.0,
        };
        assert!(compute_weighted_cpt(&m).is_none());
    }

    #[test]
    fn test_graceful_degradation_tanpa_ccusage() {
        let period = build_period_economics("Today", 10, 50000, 89.0, None);
        assert!(period.cc_cost.is_none());
        assert!(period.dollar_saved.is_none());
        assert_eq!(period.omni_commands, Some(10));
        assert_eq!(period.omni_saved_bytes, Some(50000));
        assert!(period.omni_reduction_pct == Some(89.0));
        // Must not crash when ccusage is unavailable.
    }

    #[test]
    fn test_build_period_economics_with_metrics() {
        let m = CcusageMetrics {
            input_tokens: 1000,
            output_tokens: 200,
            cache_creation_tokens: 500,
            cache_read_tokens: 1000,
            total_tokens: 2700,
            total_cost: 0.005,
        };
        let period = build_period_economics("This Week", 42, 100_000, 75.5, Some(m));

        assert_eq!(period.label, "This Week");
        assert!(period.cc_cost.is_some());
        assert!(period.weighted_input_cpt.is_some());
        assert!(period.dollar_saved.is_some());
        assert!(period.dollar_saved.unwrap() > 0.0);
    }
}
