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
    sanitize_vars(env::vars())
}

pub fn sanitize_vars(vars: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    vars.into_iter()
        .filter(|(k, _)| !DENYLIST.iter().any(|d| d.eq_ignore_ascii_case(k)))
        .collect()
}

/// Returns true if OMNI_QUIET=1 is set. Suppresses stderr stats in pipe mode.
pub fn is_quiet() -> bool {
    env::vars().any(|(k, _)| k.eq_ignore_ascii_case("OMNI_QUIET"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_env_menghapus_ld_preload() {
        let mock_env = vec![
            ("LD_PRELOAD".to_string(), "bad.so".to_string()),
            ("NORMAL_VAR".to_string(), "123".to_string()),
        ];
        let sanitized = sanitize_vars(mock_env);
        let contains = sanitized.iter().any(|(k, _)| k == "LD_PRELOAD");
        assert!(!contains);
    }

    #[test]
    fn test_sanitize_env_menghapus_semua_denylist_entries() {
        let mock_env: Vec<(String, String)> = DENYLIST
            .iter()
            .map(|key| (key.to_string(), "malicious_payload".to_string()))
            .collect();

        let sanitized = sanitize_vars(mock_env);

        for (k, _) in sanitized {
            assert!(!DENYLIST.iter().any(|d| d.eq_ignore_ascii_case(&k)));
        }
    }

    #[test]
    fn test_sanitize_env_mempertahankan_path_and_normal_vars() {
        let mock_env = vec![
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("NORMAL_VAR".to_string(), "123".to_string()),
        ];

        let sanitized = sanitize_vars(mock_env);
        let has_path = sanitized.iter().any(|(k, _)| k.to_uppercase() == "PATH");
        let has_normal = sanitized
            .iter()
            .any(|(k, v)| k == "NORMAL_VAR" && v == "123");

        assert!(has_path);
        assert!(has_normal);
    }
}
