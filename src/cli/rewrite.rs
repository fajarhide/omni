// `rewrite_logic` has no CLI entry point: the `omni rewrite` subcommand was
// deleted in #164 (zero invocations on record). The module stays because
// `hooks::pre_tool` calls it on every command — see #157.

/// True when the command carries its own downstream stages — a pipe, a
/// redirect, or a chain operator standing outside quotes.
///
/// The rewrite wraps the **whole** command string, so `bash tidy.sh 2>&1 | tail -3`
/// ran as `omni exec bash tidy.sh 2>&1 | tail -3`: distillation landed upstream
/// of a pipeline the caller wrote deliberately, and `tail` returned OMNI's
/// markers instead of the script's last three lines — the summary line the pipe
/// existed to read (#157). A redirect is the same defect with a file on the end:
/// `npm run build > build.log 2>&1` wrote a truncated log **plus OMNI's banner**
/// into the file on disk, so the documented escape hatch from distillation
/// returned less than the terminal did (#170 for cargo, #207 for npm).
///
/// A caller that wrote its own stages has already said how it wants the output
/// shaped, so passing the command through untouched is the fail-open read. The
/// PostToolUse hook still distills whatever the pipeline finally produces, so
/// this costs no coverage on the path the agent actually reads.
fn has_downstream_stage(cmd: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut chars = cmd.chars();

    while let Some(c) = chars.next() {
        match quote {
            // Inside single quotes bash treats every byte literally, so only a
            // double-quoted or backticked context honours the escape.
            Some(q) => {
                if c == '\\' && q != '\'' {
                    chars.next();
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '\\' => {
                    chars.next();
                }
                '\'' | '"' | '`' => quote = Some(c),
                '|' | '>' | '<' | ';' | '&' => return true,
                _ => {}
            },
        }
    }

    false
}

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

    let wants_rewrite =
        allow_list.iter().any(|&p| cmd_str.starts_with(p)) && !has_downstream_stage(cmd_str);

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
    use super::rewrite_logic;

    #[test]
    fn rewrites_a_plain_command() {
        assert_eq!(
            rewrite_logic("git status")
                .expect("git should rewrite")
                .split(" exec ")
                .nth(1),
            Some("git status")
        );
    }

    /// #157: the rewrite wrapped the whole string, so distillation sat upstream
    /// of the caller's `tail` and the script's summary line was deleted before
    /// `tail` ever saw it.
    #[test]
    fn leaves_a_command_that_pipes_into_its_own_stage() {
        assert_eq!(rewrite_logic("bash tidy.sh 2>&1 | tail -3"), None);
        assert_eq!(rewrite_logic("git log | head -5"), None);
    }

    /// #170 / #207: `npm run build > build.log 2>&1` wrote a truncated log plus
    /// OMNI's banner into the file the shell was told to write.
    #[test]
    fn leaves_a_command_that_redirects_to_a_file() {
        assert_eq!(rewrite_logic("npm run build > build.log 2>&1"), None);
        assert_eq!(rewrite_logic("cargo tree > tree.log"), None);
        assert_eq!(rewrite_logic("make ci >> ci.log"), None);
    }

    #[test]
    fn leaves_a_chained_command() {
        assert_eq!(rewrite_logic("npm ci && npm test"), None);
        assert_eq!(rewrite_logic("git fetch; git status"), None);
    }

    /// An operator inside quotes is data, not a stage — blocking on it would
    /// give up coverage for nothing.
    #[test]
    fn still_rewrites_when_the_operator_is_quoted() {
        for cmd in [
            "git commit -m \"fix: a | b\"",
            "git log --grep 'a > b'",
            "git commit -m \"escaped \\\" quote; still one arg\"",
        ] {
            assert!(
                rewrite_logic(cmd).is_some(),
                "quoted operator should not block the rewrite: {cmd}"
            );
        }
    }

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
