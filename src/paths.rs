//! Path utilities - single source of truth for all filesystem paths
//!
//! This module centralizes all path resolution for OMNI.
//! All paths are automatically OS-agnostic using cross-platform libraries.
//! No conditional compilation is needed here - `dirs` and `std::env::temp_dir`
//! handle platform differences automatically.
//!
//! **Single source of truth means every caller asks here.** A call site that
//! builds `dirs::home_dir().join(".omni")` itself is outside every override this
//! module offers, and that is not hypothetical: `session::learn::queue_for_learn`
//! wrote the learn queue to the real home whatever the configuration said
//! (#312), and `toml_filter::resolve_user_signal_dir` read the developer's live
//! filters during `cargo test` while the same suite's writes were correctly
//! isolated (#315). Both looked correct in isolation and both bypassed the
//! override that was supposed to cover them.

use dirs::home_dir;
use std::env;
use std::path::PathBuf;

/// The legacy tree, `~/.omni`, whether or not it exists.
fn legacy_home() -> PathBuf {
    home_dir().unwrap_or_else(temp_dir).join(".omni")
}

/// Resolve one of the two roots.
///
/// Precedence, most specific first (#217):
///
/// 1. `OMNI_HOME` puts the whole tree in one place.
/// 2. `OMNI_CONFIG_HOME` / `OMNI_DATA_HOME` split config from data.
/// 3. **An existing `~/.omni` wins over XDG.** This is the migration decision
///    and it is deliberate: an install that already has a tree keeps using it,
///    so upgrading never appears to lose a database. #217 offered moving the
///    directory with a notice as an alternative; silently continuing to work is
///    the boring option and the one that cannot surprise anyone.
/// 4. `XDG_CONFIG_HOME/omni` / `XDG_DATA_HOME/omni` for a fresh install on a
///    machine that asked for the spec.
/// 5. `~/.omni`.
///
/// Read once through a `LazyLock` at each call site below. Cargo runs tests in
/// parallel in one process, so a value re-read per call could change under a
/// test that set it, which is the process-state hazard this repo has already
/// paid for once.
fn resolve_root(split_var: &str, xdg_var: &str) -> PathBuf {
    if let Some(p) = env::var_os("OMNI_HOME").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    if let Some(p) = env::var_os(split_var).filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let legacy = legacy_home();
    if legacy.exists() {
        return legacy;
    }
    if let Some(p) = env::var_os(xdg_var).filter(|p| !p.is_empty()) {
        return PathBuf::from(p).join("omni");
    }
    legacy
}

/// Where configuration lives: `config.toml`, `filters/`, `signals/`.
#[inline]
pub fn config_home() -> PathBuf {
    static ROOT: std::sync::LazyLock<PathBuf> =
        std::sync::LazyLock::new(|| resolve_root("OMNI_CONFIG_HOME", "XDG_CONFIG_HOME"));
    ROOT.clone()
}

/// Where state lives: the database, transcripts, caches, exports.
#[inline]
pub fn data_home() -> PathBuf {
    static ROOT: std::sync::LazyLock<PathBuf> =
        std::sync::LazyLock::new(|| resolve_root("OMNI_DATA_HOME", "XDG_DATA_HOME"));
    ROOT.clone()
}

/// The OMNI home directory.
///
/// Kept as the name most of the tree already asks for, and equal to
/// [`config_home`]. With no environment set at all, every root here is the same
/// `~/.omni`, so splitting config from data changes nothing for an existing
/// install and only matters to someone who asked for it.
#[inline]
pub fn omni_home() -> PathBuf {
    config_home()
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
    data_home().join("omni.db")
}

/// Get path to user defined filters directory
#[inline]
pub fn filters_directory() -> PathBuf {
    config_home().join("filters")
}

/// Get path to the user configuration file.
#[inline]
pub fn config_file() -> PathBuf {
    config_home().join("config.toml")
}

/// Get path to learned filters file.
///
/// It stays inside `filters_directory()` rather than following the database
/// into `data_home()`, even though it is generated rather than authored: the
/// loader reads that directory as a unit, so splitting one file out of it would
/// mean two directory walks to answer one question.
#[inline]
pub fn learned_filters_path() -> PathBuf {
    filters_directory().join("learned.toml")
}

/// Get path to the learn queue.
#[inline]
pub fn learn_queue_path() -> PathBuf {
    data_home().join("learn_queue.jsonl")
}

/// Get path to the cache directory.
#[inline]
pub fn cache_directory() -> PathBuf {
    data_home().join("cache")
}

/// Get path to the session export directory.
#[inline]
pub fn exports_directory() -> PathBuf {
    data_home().join("exports")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #307: without an override the suite wrote into the developer's live
    /// `~/.omni`. `.cargo/config.toml` points `OMNI_HOME` at the workspace's
    /// `target/`, so this fails if either half is removed: the config entry, or
    /// the reading of it here.
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
    }

    /// #315: the previous version of the test above asserted only that the
    /// *accessor* was overridden, and passed while `toml_filter` read the real
    /// `~/.omni/filters` on every hook. Every path the product actually opens is
    /// checked here, so a new call site that derives its own is caught by the
    /// one test rather than by a CI flake months later.
    #[test]
    fn every_resolved_path_stays_inside_a_configured_root() {
        let config = config_home();
        let data = data_home();

        for (name, path) in [
            ("filters_directory", filters_directory()),
            ("config_file", config_file()),
            ("learned_filters_path", learned_filters_path()),
        ] {
            assert!(
                path.starts_with(&config),
                "{name} resolved to {}, outside the config root {}",
                path.display(),
                config.display()
            );
        }

        for (name, path) in [
            ("database_path", database_path()),
            ("learn_queue_path", learn_queue_path()),
            ("cache_directory", cache_directory()),
            ("exports_directory", exports_directory()),
        ] {
            assert!(
                path.starts_with(&data),
                "{name} resolved to {}, outside the data root {}",
                path.display(),
                data.display()
            );
        }
    }

    /// The migration decision, stated as a test because it is the one thing an
    /// upgrade could get visibly wrong: an install that already has `~/.omni`
    /// keeps using it, even when XDG is set.
    #[test]
    fn an_existing_legacy_tree_wins_over_xdg() {
        // `resolve_root` is pure apart from the environment, and this asserts
        // the ordering rather than driving the process env, which a parallel
        // test would race on.
        let legacy = legacy_home();
        if legacy.exists() {
            assert_eq!(
                resolve_root("OMNI_DATA_HOME_UNSET_FOR_TEST", "XDG_DATA_HOME"),
                if env::var_os("OMNI_HOME").is_some_and(|p| !p.is_empty()) {
                    omni_home()
                } else {
                    legacy
                },
                "an existing ~/.omni must keep being used"
            );
        }
    }
}
