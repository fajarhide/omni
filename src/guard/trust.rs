use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(not(test))]
fn get_trusted_file_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omni")
        .join("trusted.json")
}

#[cfg(test)]
thread_local! {
    pub static TEST_TRUST_FILE: std::cell::RefCell<PathBuf> = std::cell::RefCell::new({
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("must succeed").as_nanos();
        let tid = std::thread::current().id();
        std::env::temp_dir().join(format!("omni_test_trusted_{:?}_{}.json", tid, nanos))
    });
}

#[cfg(test)]
fn get_trusted_file_path() -> PathBuf {
    TEST_TRUST_FILE.with(|f| f.borrow().clone())
}

pub fn compute_hash(path: &Path) -> Result<String> {
    let content = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

pub fn is_trusted(project_path: &Path) -> bool {
    let target_file = project_path.join("omni_config.json");
    if !target_file.exists() {
        // Technically if no config, it's not overriding anything maliciously.
        // But the trust system checks if the local config is safely tracked.
        return false; // Or true depending on logic. The prompt implies false if unknown.
    }

    let current_hash = match compute_hash(&target_file) {
        Ok(h) => h,
        Err(_) => return false,
    };

    let trust_file = get_trusted_file_path();
    if !trust_file.exists() {
        return false;
    }

    let content = match fs::read_to_string(&trust_file) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let trusted_map: HashMap<String, String> = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let proj_key = project_path.to_string_lossy().to_string();
    if let Some(trusted_hash) = trusted_map.get(&proj_key)
        && trusted_hash == &current_hash
    {
        return true;
    }

    false
}

pub fn trust_project(project_path: &Path) -> Result<String> {
    let target_file = project_path.join("omni_config.json");
    let current_hash = compute_hash(&target_file)?;

    let trust_file = get_trusted_file_path();
    if let Some(p) = trust_file.parent() {
        fs::create_dir_all(p)?;
    }

    let mut trusted_map: HashMap<String, String> = if trust_file.exists() {
        let content = fs::read_to_string(&trust_file).unwrap_or_else(|_| "{}".to_string());
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let proj_key = project_path.to_string_lossy().to_string();
    trusted_map.insert(proj_key, current_hash.clone());

    let new_content = serde_json::to_string_pretty(&trusted_map)?;
    fs::write(&trust_file, new_content)?;

    Ok(current_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_is_trusted_false_for_unknown_project() {
        let dir = tempdir().unwrap();
        // Create an untrusted omni_config.json
        let cfg = dir.path().join("omni_config.json");
        fs::write(&cfg, "{}").unwrap();

        // Since trusted.json in ~/.omni doesn't contain it, is_trusted is false!
        assert!(!is_trusted(dir.path()));
    }

    #[test]
    fn test_trust_project_and_is_trusted_roundtrip() {
        // Override home dir for tests to avoid writing to real ~/.omni/trusted.json
        // Wait, since we can't cleanly override home_dir globally across threads using env var natively without breaking things,
        // we'll run the test but cautiously. It's an integration-like test.
        // For actual safe tests, a custom home directory env var would be used.
        // Let's manually write a test relying on the actual path but using a unique temp string as project_path so it doesn't collide.
        let dir = tempdir().unwrap();
        let cfg = dir.path().join("omni_config.json");
        fs::write(&cfg, "{\"trusted\": true}").unwrap();

        assert!(!is_trusted(dir.path()));

        let hash = trust_project(dir.path()).unwrap();
        assert!(!hash.is_empty());
        assert!(is_trusted(dir.path()));
    }

    #[test]
    fn test_is_trusted_false_setelah_file_dimodifikasi() {
        let dir = tempdir().unwrap();
        let cfg = dir.path().join("omni_config.json");
        fs::write(&cfg, "{\"trusted\": true}").unwrap();

        let _ = trust_project(dir.path()).unwrap();
        assert!(is_trusted(dir.path()));

        // Modifikasi file
        fs::write(&cfg, "{\"trusted\": false, \"malicious\": true}").unwrap();

        // Hash changed, is_trusted must be false!
        assert!(!is_trusted(dir.path()));
    }
}
