//! Path utilities - single source of truth for all filesystem paths
//!
//! This module centralizes all path resolution for OMNI.
//! All paths are automatically OS-agnostic using cross-platform libraries.
//! No conditional compilation is needed here - `dirs` and `std::env::temp_dir`
//! handle platform differences automatically.

use dirs::home_dir;
use std::env;
use std::path::PathBuf;

/// Get OMNI home directory
///
/// Resolves automatically:
/// - `$OMNI_HOME` when set, used verbatim
/// - Linux/macOS: `~/.omni`
/// - Windows: `%USERPROFILE%\.omni`
///   Falls back to temp directory if home directory is not available
///
/// The override exists because there was none, and the test suite wrote into the
/// developer's live config as a result: `cargo test --all` left 11 auto-learned
/// filters in `~/.omni/filters/learned.toml`, one `make ci` added 31,458 bytes,
/// and the file had reached 1.8 MB of filters nobody asked for. `load_all_filters`
/// reads that directory on every hook, so the suite was quietly making the tool
/// slower for the person running it, and the filters then joined the `find()`
/// race and decided which signal claimed a command (#307).
///
/// Read through a `LazyLock` on purpose. Cargo runs tests in parallel in one
/// process, so a value read per call could change under a test that set it,
/// which is the process-state hazard this repo has already paid for once. Read
/// once, at first use, and the answer is the same for every caller.
#[inline]
pub fn omni_home() -> PathBuf {
    static HOME: std::sync::LazyLock<PathBuf> =
        std::sync::LazyLock::new(|| match env::var_os("OMNI_HOME") {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            _ => home_dir().unwrap_or_else(temp_dir).join(".omni"),
        });
    HOME.clone()
}

/// Get system temporary directory
///
/// Resolves automatically:
/// - Linux/macOS: `/tmp`
/// - Windows: `%TEMP%`
#[inline]
pub fn temp_dir() -> PathBuf {
    env::temp_dir()
}

/// Get path to OMNI SQLite database
#[inline]
pub fn database_path() -> PathBuf {
    omni_home().join("omni.db")
}

/// Get path to user defined filters directory
#[inline]
pub fn filters_directory() -> PathBuf {
    omni_home().join("filters")
}

/// Get path to trusted projects signature file
#[inline]
#[cfg_attr(test, allow(dead_code))]
pub fn trusted_projects_path() -> PathBuf {
    omni_home().join("trusted.json")
}

/// Get path to learned filters file
#[inline]
pub fn learned_filters_path() -> PathBuf {
    filters_directory().join("learned.toml")
}

/// Ensure OMNI home directory exists
/// Creates parent directories if they don't exist
pub fn ensure_omni_home() -> std::io::Result<()> {
    std::fs::create_dir_all(omni_home())?;
    std::fs::create_dir_all(filters_directory())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #307: without an override the suite wrote into the developer's live
    /// `~/.omni`. `.cargo/config.toml` points `OMNI_HOME` at the workspace's
    /// `target/`, so this fails if either half is removed: the config entry, or
    /// `omni_home`'s reading of it.
    #[test]
    fn a_test_run_never_resolves_to_the_developers_real_home() {
        let home = omni_home();
        assert!(
            home.ends_with("omni-home"),
            "OMNI_HOME is not in effect: a test run would write to {}",
            home.display()
        );

        let real = home_dir().map(|h| h.join(".omni"));
        assert_ne!(
            Some(home.clone()),
            real,
            "the suite must not share a home with the installed binary"
        );

        // Everything else derives from it, so one override covers the DB, the
        // filters and the transcripts alike.
        assert!(learned_filters_path().starts_with(&home));
        assert!(database_path().starts_with(&home));
    }
}
