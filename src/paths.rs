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
