use colored::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug)]
struct UpdateCache {
    latest_version: String,
    last_checked: u64,
}

fn get_cache_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omni")
        .join("update_cache.json")
}

pub fn check() -> Option<String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let cache_path = get_cache_path();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // 1. Try to load from cache
    if let Ok(content) = fs::read_to_string(&cache_path)
        && let Ok(cache) = serde_json::from_str::<UpdateCache>(&content)
        && now < cache.last_checked + 86400
    {
        if is_newer(&cache.latest_version, current_version) {
            return Some(cache.latest_version);
        }
        return None;
    }

    // 2. Not in cache or cache expired: Fetch from GitHub
    // We do this synchronously but with a very short timeout (2s)
    let url = "https://api.github.com/repos/fajarhide/omni/releases/latest";
    let agent = ureq::AgentBuilder::new()
        .timeout_read(std::time::Duration::from_secs(2))
        .timeout_connect(std::time::Duration::from_secs(2))
        .build();

    let resp = agent.get(url).set("User-Agent", "omni-cli").call();

    match resp {
        Ok(response) => {
            #[derive(Deserialize)]
            struct GitHubRelease {
                tag_name: String,
            }

            if let Ok(release) = response.into_json::<GitHubRelease>() {
                let latest = release.tag_name.trim_start_matches('v').to_string();

                // Save to cache
                let cache = UpdateCache {
                    latest_version: latest.clone(),
                    last_checked: now,
                };
                if let Ok(json) = serde_json::to_string(&cache) {
                    let _ = fs::create_dir_all(cache_path.parent().unwrap());
                    let _ = fs::write(&cache_path, json);
                }

                if is_newer(&latest, current_version) {
                    return Some(latest);
                }
            }
        }
        Err(_) => {
            // Fail silently on network errors
        }
    }

    None
}

fn is_newer(latest: &str, current: &str) -> bool {
    let v1: Vec<u32> = latest.split('.').filter_map(|s| s.parse().ok()).collect();
    let v2: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();

    for (a, b) in v1.iter().zip(v2.iter()) {
        if a > b {
            return true;
        }
        if a < b {
            return false;
        }
    }
    v1.len() > v2.len()
}

pub fn print_notification(latest: &str) {
    println!(
        "\n  {} A new version of OMNI is available: {} → {}",
        "✨".yellow(),
        env!("CARGO_PKG_VERSION").bright_black(),
        latest.green().bold()
    );
    println!(
        "      Run: {} to upgrade.\n",
        "brew upgrade fajarhide/tap/omni".cyan()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("0.5.3", "0.5.2"));
        assert!(is_newer("0.6.0", "0.5.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.5.2", "0.5.2"));
        assert!(!is_newer("0.5.1", "0.5.2"));
    }
}
