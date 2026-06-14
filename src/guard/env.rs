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
    "SSH_ASKPASS",
    "SSH_AUTH_SOCK",
    "GIT_SSH_COMMAND",
    "GIT_SSH",
    "SVN_SSH",
    "CVS_RSH",
    "PERL5LIB",
    "PERL5OPT",
    "PERLLIB",
    "AWKPATH",
    "AWKLIBPATH",
    "XAUTHORITY",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "LOCPATH",
    "NLSPATH",
    "GCONV_PATH",
    "GIT_EXTERNAL_DIFF",
    "GIT_MERGE_AUTOEDIT",
    "PAGER",
    "EDITOR",
    "VISUAL",
    "MANPAGER",
    "MANPATH",
    "HOSTALIASES",
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

/// Returns true if OMNI_PASSTHROUGH is enabled (1/true/yes).
/// When enabled, OMNI will bypass distillation and emit raw output.
pub fn is_passthrough() -> bool {
    env::vars().any(|(k, v)| {
        if !k.eq_ignore_ascii_case("OMNI_PASSTHROUGH") {
            return false;
        }
        matches!(
            v.trim().to_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[derive(Debug, PartialEq)]
pub enum ValidationError {
    InvalidLoopId,
    GoalTooLong,
    GoalContainsShellMetachars,
    BudgetTooLarge,
}

pub fn validate_loop_context(
    loop_id: Option<&str>,
    goal: Option<&str>,
    budget_tokens: Option<u64>,
) -> Result<(), ValidationError> {
    // loop_id: only alphanumeric and dash, max 64 chars
    if let Some(id) = loop_id
        && (!id.chars().all(|c| c.is_alphanumeric() || c == '-') || id.len() > 64)
    {
        return Err(ValidationError::InvalidLoopId);
    }

    // goal: max 500 chars, no shell metacharacters
    if let Some(goal) = goal {
        if goal.len() > 500 {
            return Err(ValidationError::GoalTooLong);
        }
        // Block shell injection
        let blocked = ['`', '$', ';', '|', '&', '>', '<', '(', ')'];
        if goal.chars().any(|c| blocked.contains(&c)) {
            return Err(ValidationError::GoalContainsShellMetachars);
        }
    }

    // budget: max 10M tokens (sanity check)
    if budget_tokens.unwrap_or(0) > 10_000_000 {
        return Err(ValidationError::BudgetTooLarge);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_env_removes_ld_preload() {
        let mock_env = vec![
            ("LD_PRELOAD".to_string(), "bad.so".to_string()),
            ("NORMAL_VAR".to_string(), "123".to_string()),
        ];
        let sanitized = sanitize_vars(mock_env);
        let contains = sanitized.iter().any(|(k, _)| k == "LD_PRELOAD");
        assert!(!contains);
    }

    #[test]
    fn test_sanitize_env_removes_all_denylist_entries() {
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
    fn test_sanitize_env_preserves_path_and_normal_vars() {
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

    #[test]
    fn test_is_passthrough_enabled_by_value() {
        // SAFETY: Test runs single-threaded; no concurrent env access.
        unsafe {
            std::env::set_var("OMNI_PASSTHROUGH", "1");
        }
        assert!(is_passthrough());
        unsafe {
            std::env::set_var("OMNI_PASSTHROUGH", "true");
        }
        assert!(is_passthrough());
        unsafe {
            std::env::set_var("OMNI_PASSTHROUGH", "0");
        }
        assert!(!is_passthrough());
        unsafe {
            std::env::remove_var("OMNI_PASSTHROUGH");
        }
    }

    #[test]
    fn test_loop_id_injection_rejected() {
        assert_eq!(
            validate_loop_context(Some("abc-123;rm-rf"), None, None),
            Err(ValidationError::InvalidLoopId)
        );
        assert_eq!(
            validate_loop_context(Some("a".repeat(65).as_str()), None, None),
            Err(ValidationError::InvalidLoopId)
        );
        assert_eq!(
            validate_loop_context(Some("valid-id-123"), None, None),
            Ok(())
        );
    }

    #[test]
    fn test_goal_shell_metachar_sanitized() {
        assert_eq!(
            validate_loop_context(None, Some("fix this; echo 'pwned'"), None),
            Err(ValidationError::GoalContainsShellMetachars)
        );
        assert_eq!(
            validate_loop_context(None, Some("fix the auth (bug)"), None),
            Err(ValidationError::GoalContainsShellMetachars)
        );
        assert_eq!(
            validate_loop_context(None, Some("fix the auth issue"), None),
            Ok(())
        );
    }

    #[test]
    fn test_budget_overflow_clamped() {
        assert_eq!(
            validate_loop_context(None, None, Some(10_000_001)),
            Err(ValidationError::BudgetTooLarge)
        );
        assert_eq!(validate_loop_context(None, None, Some(10_000_000)), Ok(()));
    }
}
