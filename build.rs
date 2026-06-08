use std::process::Command;

fn main() {
    // Re-run if .git/HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");

    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let build_date = Command::new("date")
        .args(["+%Y-%m-%d %H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=OMNI_GIT_HASH={}", git_hash);
    println!("cargo:rustc-env=OMNI_BUILD_DATE={}", build_date);
}
