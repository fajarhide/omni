// `rewrite_logic` has no CLI entry point: the `omni rewrite` subcommand was
// deleted in #164 (zero invocations on record). The module stays because
// `hooks::pre_tool` calls it on every command — see #157.

pub fn rewrite_logic(cmd_str: &str) -> Option<String> {
    let allow_list = [
        "git ",
        "cargo ",
        "npm ",
        "pytest ",
        "kubectl ",
        "docker ",
        "terraform ",
        "make ",
        "node ",
        "python ",
        "go ",
        "bash ",
        "sh ",
    ];

    let wants_rewrite = allow_list.iter().any(|&p| cmd_str.starts_with(p));

    if wants_rewrite {
        // We always try to rewrite recognized tools to capture them.
        // run_exec will handle whether to use a shell or not.
        let exe_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("omni"));
        let exe_name = exe_path.to_string_lossy();
        // Claude Code on Windows runs hook output via Git Bash, which interprets
        // backslashes as escape characters and mangles the path
        // (`C:\Users\...` -> `C:Users...`). Use forward slashes so the path
        // survives bash unquoting; Windows accepts `/` in absolute paths.
        #[cfg(windows)]
        let exe_name = exe_name.replace('\\', "/");

        return Some(format!("{} exec {}", exe_name, cmd_str));
    }

    None
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::rewrite_logic;

    #[test]
    #[cfg(windows)]
    fn test_rewrite_uses_forward_slashes_on_windows() {
        // On Windows, the rewritten command must not contain backslashes from
        // the omni exe path; Git Bash strips them as escape characters.
        let rewritten = rewrite_logic("git status").expect("git should rewrite");
        let exe_part = rewritten.trim_end_matches(" exec git status");
        assert!(
            !exe_part.contains('\\'),
            "rewritten exe path should not contain backslashes: {exe_part}"
        );
    }
}
