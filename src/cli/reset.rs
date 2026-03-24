use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run() -> Result<(), String> {
    let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
    let omni_dir = home_dir.join(".omni");

    if !omni_dir.exists() {
        println!("[omni] The ~/.omni directory does not exist. Nothing to reset.");
        return Ok(());
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let backup_dir_name = format!(".omni.{}.bak", timestamp);
    let backup_dir = home_dir.join(&backup_dir_name);

    if let Err(e) = fs::rename(&omni_dir, &backup_dir) {
        return Err(format!(
            "Failed to backup ~/.omni to ~/{}: {}",
            backup_dir_name, e
        ));
    }

    println!("✓ Data backed up successfully.");
    println!("  Moved ~/.omni to ~/{}", backup_dir_name);
    println!();

    println!("Cleaning up agent integrations...");
    let args = vec!["--uninstall".to_string()];
    if let Err(e) = crate::cli::init::run_init(&args) {
        println!("  (Note: could not fully remove hooks/MCP configs: {})", e);
    }

    println!();
    println!(
        "You can now run 'brew uninstall fajarhide/tap/omni' safely for a completely clean uninstall."
    );

    Ok(())
}
