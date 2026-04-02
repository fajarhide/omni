use std::env;

pub const DENYLIST: &[&str] = &[
    "BASH_ENV",
    "ENV",
    "ZDOTDIR",
    "BASH_PROFILE",
    "NODE_OPTIONS",
    "NODE_EXTRA_CA_CERTS",
    "PYTHONSTARTUP",
    "PYTHONPATH",
    "PYTHONHOME",
    "RUBYOPT",
    "RUBYLIB",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_FORCE_FLAT_NAMESPACE",
    "GIT_EXEC_PATH",
    "GIT_ASKPASS",
    "GIT_TEMPLATE_DIR",
    "JAVA_TOOL_OPTIONS",
    "_JAVA_OPTIONS",
    "IFS",
    "CDPATH",
    "PROMPT_COMMAND",
];

pub fn sanitize_env() -> Vec<(String, String)> {
    env::vars()
        .filter(|(k, _)| !DENYLIST.iter().any(|d| d.eq_ignore_ascii_case(k)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_env_menghapus_ld_preload() {
        unsafe {
            std::env::set_var("LD_PRELOAD", "bad.so");
        }
        let sanitized = sanitize_env();
        let contains = sanitized.iter().any(|(k, _)| k == "LD_PRELOAD");
        assert!(!contains);
        unsafe {
            std::env::remove_var("LD_PRELOAD");
        }
    }

    #[test]
    fn test_sanitize_env_menghapus_semua_denylist_entries() {
        for key in DENYLIST {
            unsafe {
                std::env::set_var(key, "malicious_payload");
            }
        }

        let sanitized = sanitize_env();

        for (k, _) in sanitized {
            assert!(!DENYLIST.iter().any(|d| d.eq_ignore_ascii_case(&k)));
        }

        for key in DENYLIST {
            unsafe {
                std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn test_sanitize_env_mempertahankan_path_and_normal_vars() {
        unsafe {
            std::env::set_var("PATH", "/usr/bin:/bin");
            std::env::set_var("NORMAL_VAR", "123");
        }

        let sanitized = sanitize_env();
        let has_path = sanitized.iter().any(|(k, _)| k.to_uppercase() == "PATH");
        let has_normal = sanitized
            .iter()
            .any(|(k, v)| k == "NORMAL_VAR" && v == "123");

        assert!(has_path);
        assert!(has_normal);
    }
}
