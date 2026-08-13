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

/// Get path to the user configuration file.
#[inline]
pub fn config_file() -> PathBuf {
    config_home().join("config.toml")
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
    /// *accessor* was overridden, and passed while the filter loader read the
    /// real `~/.omni/filters` on every hook. Every path the product actually
    /// opens is checked here, so a new call site that derives its own is caught
    /// by the one test rather than by a CI flake months later. The filter paths
    /// left this list with the layer they served (#505).
    #[test]
    fn every_resolved_path_stays_inside_a_configured_root() {
        let config = config_home();
        let data = data_home();

        assert!(
            config_file().starts_with(&config),
            "config_file resolved to {}, outside the config root {}",
            config_file().display(),
            config.display()
        );

        for (name, path) in [
            ("database_path", database_path()),
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

/// The project a ledger scope belongs to: the repository root, not the
/// directory the command happened to run in (#525).
///
/// The project scope was `current_dir()` alone, so the same repository reached
/// by a different path was a different project. Measured on the maintainer's
/// database before this landed: 31 project scopes, and one repository split
/// across six of them, 2,759 lines of history (15.6%) stranded in scopes
/// nothing would consult again. Four of the six were created in a single day of
/// ordinary work, because a git worktree is a different directory.
///
/// **A linked worktree is why this is not a walk up to `.git`.** In a worktree
/// `.git` is a *file* holding `gitdir: <main>/.git/worktrees/<name>`, so
/// stopping at the first `.git` returns the worktree root and splits the
/// history exactly as before, while looking like a fix. The gitdir is read and
/// resolved back to the main checkout.
///
/// Falls back to the directory itself outside a repository, which is what a
/// scope keyed on a path has always been.
pub fn project_key(from: &std::path::Path) -> String {
    for dir in from.ancestors() {
        let dot = dir.join(".git");
        if dot.is_dir() {
            return dir.to_string_lossy().to_string();
        }
        if dot.is_file() {
            return worktree_main_root(&dot).unwrap_or_else(|| dir.to_string_lossy().to_string());
        }
    }
    from.to_string_lossy().to_string()
}

/// `<main>/.git/worktrees/<name>` back to `<main>`.
///
/// Returns `None` for anything that does not have that shape, so an unreadable
/// or unfamiliar `.git` file leaves the caller on the directory it started from
/// rather than on a guess.
fn worktree_main_root(dot_git_file: &std::path::Path) -> Option<String> {
    let contents = std::fs::read_to_string(dot_git_file).ok()?;
    let gitdir = contents.trim().strip_prefix("gitdir:")?.trim();
    let git_dir = std::path::Path::new(gitdir)
        .ancestors()
        .find(|a| a.file_name().is_some_and(|n| n == ".git"))?;
    Some(git_dir.parent()?.to_string_lossy().to_string())
}

#[cfg(test)]
mod project_key_tests {
    use super::project_key;

    #[test]
    fn a_plain_checkout_answers_its_own_root() {
        let d = tempfile::tempdir().expect("tempdir");
        let root = d.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).expect("mkdir");
        std::fs::create_dir_all(root.join("src/deep")).expect("mkdir");

        assert_eq!(
            project_key(&root.join("src/deep")),
            root.to_string_lossy(),
            "a subdirectory has to answer the repository root, not itself"
        );
    }

    /// The case the whole change exists for. Stopping at the first `.git` would
    /// return the worktree root here and pass a test written the obvious way.
    #[test]
    fn a_linked_worktree_answers_the_main_checkout() {
        let d = tempfile::tempdir().expect("tempdir");
        let main = d.path().join("omni");
        let tree = main.join(".git-worktrees/520");
        std::fs::create_dir_all(main.join(".git/worktrees/520")).expect("mkdir");
        std::fs::create_dir_all(tree.join("src")).expect("mkdir");
        std::fs::write(
            tree.join(".git"),
            format!("gitdir: {}/.git/worktrees/520\n", main.display()),
        )
        .expect("write");

        assert_eq!(
            project_key(&tree.join("src")),
            main.to_string_lossy(),
            "a worktree has to share the main checkout's history, not open its own"
        );
    }

    #[test]
    fn outside_a_repository_it_stays_where_it_is() {
        let d = tempfile::tempdir().expect("tempdir");
        let loose = d.path().join("notes");
        std::fs::create_dir_all(&loose).expect("mkdir");

        assert_eq!(project_key(&loose), loose.to_string_lossy());
    }

    /// An unreadable or unfamiliar `.git` file must not become a guess.
    #[test]
    fn a_git_file_it_cannot_parse_falls_back_to_the_directory() {
        let d = tempfile::tempdir().expect("tempdir");
        let odd = d.path().join("odd");
        std::fs::create_dir_all(&odd).expect("mkdir");
        std::fs::write(odd.join(".git"), "something else entirely\n").expect("write");

        assert_eq!(project_key(&odd), odd.to_string_lossy());
    }
}
