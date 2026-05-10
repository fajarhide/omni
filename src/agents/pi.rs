use crate::agents::AgentIntegration;
use colored::*;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

// ---------------------------------------------------------------------------
// Install mode
// ---------------------------------------------------------------------------

/// How the user wants to install the OMNI Pi package.
#[derive(Debug, Clone, PartialEq)]
pub enum PiInstallMode {
    /// `pi install git:github.com/<owner>/omni`
    Global,
    /// `pi install git:github.com/<owner>/omni --local`
    Local,
    /// Print the command but do not execute it.
    Manual,
}

// ---------------------------------------------------------------------------
// Configuration defaults
// ---------------------------------------------------------------------------

/// Default git package source for `pi install`.
const DEFAULT_PACKAGE_SOURCE: &str = "git:github.com/wayanjimmy/omni@v0.6.0-pi-alpha";

/// Environment variable override for the package source.
const PACKAGE_SOURCE_ENV: &str = "OMNI_PI_PACKAGE_SOURCE";

/// Read the package source, checking the environment variable first.
fn package_source() -> String {
    std::env::var(PACKAGE_SOURCE_ENV).unwrap_or_else(|_| DEFAULT_PACKAGE_SOURCE.to_string())
}

// ---------------------------------------------------------------------------
// Pi binary detection
// ---------------------------------------------------------------------------

/// Locate the `pi` binary on `PATH`. Returns `None` if not found.
fn find_pi_binary() -> Option<PathBuf> {
    let candidates = ["pi", "pi-cli"];
    for candidate in candidates {
        if let Ok(output) = Command::new(candidate).arg("--version").output()
            && output.status.success()
        {
            return Some(PathBuf::from(candidate));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Pi settings helpers (read-only)
// ---------------------------------------------------------------------------

/// Return the default global Pi settings path.
fn pi_settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pi")
        .join("agent")
        .join("settings.json")
}

/// Return the project-local Pi settings path (`.pi/settings.json` in CWD).
fn pi_local_settings_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".pi")
        .join("settings.json")
}

/// Return the legacy extension path that could cause double-loading.
fn legacy_extension_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pi")
        .join("agent")
        .join("extensions")
        .join("omni.ts")
}

/// Read-only parsed Pi settings for diagnostics.
struct PiSettingsSnapshot {
    _path: PathBuf,
    json: Option<Value>,
}

impl PiSettingsSnapshot {
    /// Load settings, preferring whichever path contains an OMNI package reference.
    /// Falls back to whichever file exists.
    fn load() -> Self {
        let global_path = pi_settings_path();
        let local_path = pi_local_settings_path();

        let global_json = std::fs::read_to_string(&global_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok());

        let local_json = std::fs::read_to_string(&local_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok());

        let (path, json) = if let Some(ref g) = global_json
            && find_omni_references(g)
        {
            (global_path, global_json)
        } else if let Some(ref l) = local_json
            && find_omni_references(l)
        {
            (local_path, local_json)
        } else {
            (global_path, global_json.or(local_json))
        };

        Self { _path: path, json }
    }

    /// True if settings contain a package entry referencing OMNI or pi-omni.
    fn has_omni_package(&self) -> bool {
        let Some(val) = &self.json else {
            return false;
        };
        find_omni_references(val)
    }

    /// Return package/extension sources that appear to be OMNI-related duplicates.
    fn duplicate_sources(&self) -> Vec<String> {
        let Some(val) = &self.json else {
            return vec![];
        };
        collect_omni_sources(val)
    }
}

/// Search a JSON value recursively for strings containing "omni" (case-insensitive).
fn find_omni_references(val: &Value) -> bool {
    match val {
        Value::String(s) => s.to_lowercase().contains("omni"),
        Value::Array(arr) => arr.iter().any(find_omni_references),
        Value::Object(map) => map.values().any(find_omni_references),
        _ => false,
    }
}

/// Collect OMNI-related source strings from Pi settings.
fn collect_omni_sources(val: &Value) -> Vec<String> {
    let mut sources = Vec::new();
    collect_omni_sources_recursive(val, &mut sources);
    sources
}

fn collect_omni_sources_recursive(val: &Value, out: &mut Vec<String>) {
    match val {
        Value::String(s) if s.to_lowercase().contains("omni") => {
            out.push(s.clone());
        }
        Value::Array(arr) => {
            for v in arr {
                collect_omni_sources_recursive(v, out);
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                collect_omni_sources_recursive(v, out);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Install args construction
// ---------------------------------------------------------------------------

/// Build the argument list for `pi install`.
fn build_install_args(source: &str, mode: &PiInstallMode) -> Vec<String> {
    let mut args = vec!["install".to_string(), source.to_string()];
    if *mode == PiInstallMode::Local {
        args.push("--local".to_string());
    }
    args
}

/// Print the install command without executing it.
fn print_manual_command(source: &str, mode: &PiInstallMode) {
    let args = build_install_args(source, mode);
    println!("  {} pi {}", "$".bright_black(), args.join(" ").cyan());
}

// ---------------------------------------------------------------------------
// PiIntegration
// ---------------------------------------------------------------------------

pub struct PiIntegration;

impl AgentIntegration for PiIntegration {
    fn id(&self) -> &'static str {
        "pi"
    }

    fn name(&self) -> &'static str {
        "Pi Agent"
    }

    fn install(&self, _exe_path: &str) -> anyhow::Result<()> {
        let source = package_source();
        let mode = PiInstallMode::Global;
        run_install_with_mode(&source, &mode)
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        // Pi does not expose a reliable package-removal command.
        // Print actionable manual cleanup instructions instead of editing
        // Pi settings blindly.
        println!(
            "  {} Pi does not expose a package-removal command.",
            "ℹ".blue()
        );
        println!("  {} To remove the OMNI Pi package manually:", "→".cyan());
        println!(
            "    1. Edit {} and remove the OMNI package entry.",
            pi_settings_path().display()
        );
        println!(
            "    2. Remove any stale files in {}.",
            legacy_extension_path()
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "~/.pi/agent/extensions/".to_string())
        );
        println!("    3. Restart Pi to apply the changes.\n");
        Ok(())
    }

    fn doctor_check(&self, fix_mode: bool, warnings: &mut Vec<String>) -> bool {
        let mut healthy = true;
        println!("\n  {}", "Pi Agent:".cyan());

        // 1. Pi binary
        if let Some(_bin) = find_pi_binary() {
            println!(
                "   {:<15} {} {}",
                "Binary:".bright_black(),
                "installed".yellow(),
                "[OK]".green().bold()
            );
        } else {
            println!(
                "   {:<15} not found on PATH {}",
                "Binary:".bright_black(),
                "[MISSING]".yellow().bold()
            );
            // Pi is optional — warn but don't fail doctor
            return true;
        }

        // 2. Package registration
        let snapshot = PiSettingsSnapshot::load();
        if snapshot.has_omni_package() {
            println!(
                "   {:<15} OMNI package registered {}",
                "Package:".bright_black(),
                "[OK]".green().bold()
            );
        } else {
            println!(
                "   {:<15} not configured {}",
                "Package:".bright_black(),
                "[WARNING]".yellow().bold()
            );
            if fix_mode {
                let source = package_source();
                println!(
                    "   {:<15} installing OMNI Pi package...",
                    "Fix:".bright_black()
                );
                if let Err(e) = run_install_with_mode(&source, &PiInstallMode::Global) {
                    warnings.push(format!("Failed to install Pi package: {e}"));
                    healthy = false;
                }
            } else {
                warnings.push(
                    "Pi is installed but OMNI package is not configured. Run: omni init --pi"
                        .to_string(),
                );
            }
        }

        // 3. Duplicate/legacy detection
        let duplicates = snapshot.duplicate_sources();
        if duplicates.len() > 1 {
            println!(
                "   {:<15} {} OMNI sources detected {}",
                "Duplicates:".bright_black(),
                duplicates.len().to_string().red(),
                "[WARNING]".yellow().bold()
            );
            for src in &duplicates {
                println!("     {} {}", "•".yellow(), src.bright_black());
            }
            warnings.push(
                "Multiple OMNI Pi sources detected. This may cause double-loading. Remove duplicates manually.".to_string(),
            );
        }

        // 4. Legacy extension file
        let legacy = legacy_extension_path();
        if legacy.exists() {
            println!(
                "   {:<15} {} {}",
                "Legacy:".bright_black(),
                legacy.display(),
                "[WARNING]".yellow().bold()
            );
            warnings.push(format!(
                "Legacy Pi extension file found at {}. Remove it to prevent double-loading.",
                legacy.display()
            ));
        }

        // 5. Invalid settings JSON
        if snapshot.json.is_none() && pi_settings_path().exists() {
            println!(
                "   {:<15} invalid JSON {}",
                "Settings:".bright_black(),
                "[ERROR]".red().bold()
            );
            warnings.push(format!(
                "Pi settings at {} contain invalid JSON. Fix manually.",
                pi_settings_path().display()
            ));
            healthy = false;
        }

        healthy
    }
}

// ---------------------------------------------------------------------------
// Install logic
// ---------------------------------------------------------------------------

/// Execute the Pi package install (or delegate to manual mode).
fn run_install_with_mode(source: &str, mode: &PiInstallMode) -> anyhow::Result<()> {
    match mode {
        PiInstallMode::Manual => {
            println!("  {} Manual install commands:\n", "ℹ".blue());
            print_manual_command(source, &PiInstallMode::Global);
            print_manual_command(source, &PiInstallMode::Local);
            println!();
            return Ok(());
        }
        PiInstallMode::Global | PiInstallMode::Local => {}
    }

    let pi_bin = find_pi_binary().ok_or_else(|| {
        anyhow::anyhow!(
            "Pi binary not found on PATH. Install Pi first: https://github.com/earendil-works/pi"
        )
    })?;

    let args = build_install_args(source, mode);
    println!("  {} Running: pi {}", "⟳".yellow(), args.join(" ").cyan());

    let output = Command::new(&pi_bin).args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("  {} pi install failed: {}", "✗".red(), stderr.trim());
        eprintln!(
            "  {} You can try manually: pi {}",
            "→".cyan(),
            args.join(" ")
        );
        anyhow::bail!("Pi package install failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        for line in stdout.lines().take(10) {
            println!("    {}", line.bright_black());
        }
    }

    println!(
        "  {} Pi package installed successfully.",
        "✓".green().bold()
    );

    // Post-install: warn about duplicates and legacy files.
    let snapshot = PiSettingsSnapshot::load();
    let duplicates = snapshot.duplicate_sources();
    if duplicates.len() > 1 {
        println!(
            "  {} {} OMNI-related sources detected in Pi settings:",
            "⚠".yellow(),
            duplicates.len()
        );
        for src in &duplicates {
            println!("    {} {}", "•".yellow(), src.bright_black());
        }
        println!(
            "  {} Remove duplicate entries to prevent double-loading.",
            "→".cyan()
        );
    }

    let legacy = legacy_extension_path();
    if legacy.exists() {
        println!(
            "  {} Legacy extension file found at {}",
            "⚠".yellow(),
            legacy.display()
        );
        println!(
            "  {} Remove it to prevent double-loading: rm {}",
            "→".cyan(),
            legacy.display()
        );
    }

    println!(
        "  {} Restart Pi to activate the OMNI extension.\n",
        "✓".green().bold()
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Public helpers for CLI
// ---------------------------------------------------------------------------

/// Determine install mode from CLI flags.
pub fn install_mode_from_flags(args: &[String]) -> PiInstallMode {
    if args.iter().any(|a| a == "--pi-manual") {
        PiInstallMode::Manual
    } else if args.iter().any(|a| a == "--pi-local") {
        PiInstallMode::Local
    } else {
        PiInstallMode::Global
    }
}

/// Install with a specific mode (called from `omni init --pi`).
pub fn install_with_mode(exe_path: &str, mode: &PiInstallMode) -> anyhow::Result<()> {
    let source = package_source();
    let _integration = PiIntegration;
    if matches!(mode, PiInstallMode::Manual) {
        return run_install_with_mode(&source, mode);
    }
    // For Global/Local, we still use the integration but override mode.
    let _ = exe_path;
    run_install_with_mode(&source, mode)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_global_args_by_default() {
        let args = build_install_args("git:github.com/fajarhide/omni", &PiInstallMode::Global);
        assert_eq!(args, vec!["install", "git:github.com/fajarhide/omni"]);
    }

    #[test]
    fn appends_local_flag_for_local_mode() {
        let args = build_install_args("git:github.com/fajarhide/omni", &PiInstallMode::Local);
        assert_eq!(
            args,
            vec!["install", "git:github.com/fajarhide/omni", "--local"]
        );
    }

    #[test]
    fn manual_mode_returns_no_args_for_execution() {
        let args = build_install_args("git:github.com/fajarhide/omni", &PiInstallMode::Manual);
        assert_eq!(args, vec!["install", "git:github.com/fajarhide/omni"]);
    }

    #[test]
    fn detects_omni_package_in_settings() {
        let json: Value =
            serde_json::from_str(r#"{"packages": [{"source": "git:github.com/fajarhide/omni"}]}"#)
                .unwrap();
        assert!(find_omni_references(&json));
    }

    #[test]
    fn detects_pi_omni_reference() {
        let json: Value = serde_json::from_str(
            r#"{"packages": [{"source": "git:github.com/wayanjimmy/pi-omni"}]}"#,
        )
        .unwrap();
        assert!(find_omni_references(&json));
    }

    #[test]
    fn no_false_positive_on_clean_settings() {
        let json: Value = serde_json::from_str(r#"{"theme": "dark", "packages": []}"#).unwrap();
        assert!(!find_omni_references(&json));
    }

    #[test]
    fn collects_duplicate_sources() {
        let json: Value = serde_json::from_str(
            r#"{"packages": [{"source": "git:github.com/fajarhide/omni"}, {"source": "git:github.com/wayanjimmy/pi-omni"}]}"#,
        )
        .unwrap();
        let sources = collect_omni_sources(&json);
        assert_eq!(sources.len(), 2);
        assert!(sources[0].contains("omni"));
        assert!(sources[1].contains("omni"));
    }

    #[test]
    fn install_mode_global_from_default_flags() {
        let args: Vec<String> = vec!["--pi".to_string()];
        let mode = install_mode_from_flags(&args);
        assert_eq!(mode, PiInstallMode::Global);
    }

    #[test]
    fn install_mode_local_from_flag() {
        let args: Vec<String> = vec!["--pi".to_string(), "--pi-local".to_string()];
        let mode = install_mode_from_flags(&args);
        assert_eq!(mode, PiInstallMode::Local);
    }

    #[test]
    fn install_mode_manual_from_flag() {
        let args: Vec<String> = vec!["--pi".to_string(), "--pi-manual".to_string()];
        let mode = install_mode_from_flags(&args);
        assert_eq!(mode, PiInstallMode::Manual);
    }

    #[test]
    fn manual_takes_priority_over_local() {
        let args: Vec<String> = vec![
            "--pi".to_string(),
            "--pi-local".to_string(),
            "--pi-manual".to_string(),
        ];
        let mode = install_mode_from_flags(&args);
        assert_eq!(mode, PiInstallMode::Manual);
    }

    #[test]
    fn settings_path_is_under_home() {
        let path = pi_settings_path();
        assert!(path.to_string_lossy().contains(".pi"));
        assert!(path.to_string_lossy().contains("settings.json"));
    }

    #[test]
    fn legacy_path_is_under_extensions() {
        let path = legacy_extension_path();
        assert!(path.to_string_lossy().contains("extensions"));
        assert!(path.to_string_lossy().contains("omni.ts"));
    }

    #[test]
    fn package_source_env_override() {
        // Default source
        let default = package_source();
        assert!(default.contains("omni"));

        // With env override (set and restore)
        // Safety: test is single-threaded.
        unsafe {
            std::env::set_var(PACKAGE_SOURCE_ENV, "git:github.com/test/repo");
        }
        let overridden = package_source();
        assert_eq!(overridden, "git:github.com/test/repo");

        // Clean up
        unsafe {
            std::env::remove_var(PACKAGE_SOURCE_ENV);
        }
    }

    #[test]
    fn pi_settings_snapshot_handles_missing_file() {
        // Use a temp dir with no settings — should not panic.
        let snapshot = PiSettingsSnapshot {
            _path: PathBuf::from("/tmp/omni-test-nonexistent/settings.json"),
            json: None,
        };
        assert!(!snapshot.has_omni_package());
        assert!(snapshot.duplicate_sources().is_empty());
    }

    #[test]
    fn pi_settings_snapshot_handles_invalid_json() {
        let bad_json: Value = serde_json::from_str("not valid json").unwrap_or(Value::Null);
        let snapshot = PiSettingsSnapshot {
            _path: PathBuf::from("/tmp/test"),
            json: Some(bad_json),
        };
        // Null value should not contain OMNI references
        assert!(!snapshot.has_omni_package());
    }

    #[test]
    fn finds_references_nested_in_objects() {
        let json: Value = serde_json::from_str(
            r#"{"level1": {"level2": {"source": "git:github.com/fajarhide/omni"}}}"#,
        )
        .unwrap();
        assert!(find_omni_references(&json));
    }

    #[test]
    fn pi_integration_id_and_name() {
        let pi = PiIntegration;
        assert_eq!(pi.id(), "pi");
        assert_eq!(pi.name(), "Pi Agent");
    }
}
