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
/// - Linux/macOS: `~/.omni`
/// - Windows: `%USERPROFILE%\.omni`
///   Falls back to temp directory if home directory is not available
#[inline]
pub fn omni_home() -> PathBuf {
    home_dir().unwrap_or_else(temp_dir).join(".omni")
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

// ─── Worktree Detection (L2-03) ────────────────────────────

/// Worktree detection utilities for multi-agent parallel worktree support.
/// These functions are infrastructure for L2-03 and will be consumed
/// by multiagent.rs once worktree-aware session isolation is fully wired.
#[allow(dead_code)]
pub mod worktree {
    use super::*;

    /// Find the git root by traversing upward from `start`.
    pub fn find_git_root(start: &std::path::Path) -> Option<PathBuf> {
        let mut current = start.to_path_buf();
        loop {
            if current.join(".git").exists() {
                return Some(current);
            }
            if !current.pop() {
                return None;
            }
        }
    }

    /// Check if a git root is actually a worktree (`.git` is a file, not a directory).
    pub fn is_worktree(git_root: &std::path::Path) -> bool {
        let git_entry = git_root.join(".git");
        git_entry.is_file() // file → worktree, directory → normal repo
    }

    /// For a worktree, resolve the main repository root.
    /// Parses `.git` file content: `gitdir: /path/to/main/.git/worktrees/<name>`
    pub fn find_main_repo_root(worktree_root: &std::path::Path) -> Option<PathBuf> {
        let git_file = worktree_root.join(".git");
        if !git_file.is_file() {
            return None;
        }
        let content = std::fs::read_to_string(&git_file).ok()?;
        let gitdir = content.trim().strip_prefix("gitdir: ")?;
        let gitdir_path = PathBuf::from(gitdir);
        // Go up from .git/worktrees/<name> → .git → repo root
        gitdir_path.parent()?.parent()?.parent().map(PathBuf::from)
    }

    /// Extract worktree name from path (last component).
    pub fn extract_worktree_name(path: &std::path::Path) -> Option<String> {
        path.file_name().and_then(|n| n.to_str()).map(String::from)
    }

    /// Resolve project context: returns (project_hash_root, is_worktree).
    /// Worktrees share the main repo root for project knowledge.
    pub fn resolve_project_root() -> (PathBuf, bool) {
        let cwd = env::current_dir().unwrap_or_default();
        let git_root = find_git_root(&cwd);
        match git_root {
            Some(ref root) if is_worktree(root) => {
                let main = find_main_repo_root(root).unwrap_or_else(|| root.clone());
                (main, true)
            }
            Some(root) => (root, false),
            None => (cwd, false),
        }
    }
}
