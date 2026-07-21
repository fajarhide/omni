use crate::pipeline::OutputSegment;

pub mod build;
pub mod cloud;
pub mod database;
pub mod generic;
pub mod git;
pub mod jsts;
pub mod readfile;
pub mod search;
pub mod security;
pub mod system_ops;
pub mod test;
pub mod vcs;

pub trait Distiller: Send + Sync {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        session: Option<&crate::pipeline::SessionState>,
    ) -> String;
}

fn extract_base_executable(command: &str) -> String {
    let tokens = shell_split_tokens(command, 8);
    if tokens.is_empty() {
        return String::new();
    }

    let mut i = 0usize;
    while i < tokens.len() {
        let t = tokens[i].as_str();
        if t == "env" || t == "command" {
            i += 1;
            continue;
        }
        if looks_like_env_assignment(t) {
            i += 1;
            continue;
        }
        return tokens[i].clone();
    }

    String::new()
}

fn looks_like_env_assignment(token: &str) -> bool {
    let Some((key, _value)) = token.split_once('=') else {
        return false;
    };
    if key.is_empty() {
        return false;
    }
    key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn shell_split_tokens(input: &str, max_tokens: usize) -> Vec<String> {
    #[derive(Clone, Copy)]
    enum Mode {
        None,
        Single,
        Double,
        Backtick,
    }

    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut mode = Mode::None;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if tokens.len() >= max_tokens {
            break;
        }

        match mode {
            Mode::None => match ch {
                '\'' => mode = Mode::Single,
                '"' => mode = Mode::Double,
                '`' => mode = Mode::Backtick,
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                c if c.is_whitespace() => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                    while matches!(chars.peek(), Some(p) if p.is_whitespace()) {
                        chars.next();
                    }
                }
                _ => current.push(ch),
            },
            Mode::Single => {
                if ch == '\'' {
                    mode = Mode::None;
                } else {
                    current.push(ch);
                }
            }
            Mode::Double => match ch {
                '"' => mode = Mode::None,
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                _ => current.push(ch),
            },
            Mode::Backtick => match ch {
                '`' => mode = Mode::None,
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                _ => current.push(ch),
            },
        }
    }

    if !current.is_empty() && tokens.len() < max_tokens {
        tokens.push(current);
    }

    tokens
}

fn should_passthrough_config_output(command: &str, input: &str) -> bool {
    let line_count = input.lines().count();
    if line_count > 500 {
        return false;
    }

    let tokens = shell_split_tokens(command, 32);
    if tokens.is_empty() {
        return false;
    }

    let candidate_path = tokens
        .iter()
        .rev()
        .find(|t| {
            let s = t.as_str();
            !s.is_empty() && s != "cat" && s != "--" && !s.starts_with('-')
        })
        .map(|s| s.to_string());

    let Some(path) = candidate_path else {
        return false;
    };

    let lower = path.to_lowercase();
    lower.ends_with(".env")
        || lower.ends_with(".toml")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".json")
}

/// Zero-state guard: a distiller may emit a success/clean summary only if it
/// positively parsed a recognized signal. With no evidence anything was parsed,
/// fail open by returning the raw input unchanged — never a synthesized
/// `✓`/"no errors"/"passed" string that is byte-identical to a real clean run.
///
/// This is the shared invariant behind #143: a misdetection upstream then
/// degrades to passthrough instead of a confident false claim.
pub(crate) fn require_parsed(parsed: bool, input: &str, summary: String) -> String {
    if parsed { summary } else { input.to_string() }
}

/// Distill output based on command
#[tracing::instrument(skip_all)]
pub fn distill_with_command(
    segments: &[crate::pipeline::OutputSegment],
    input: &str,
    command: &str,
    session: Option<&crate::pipeline::SessionState>,
) -> String {
    // 1. Resolve pipeline profile (though we match command here too)
    let _profile = crate::pipeline::registry::resolve_profile(command);

    // Phase 1: Try exact command prefix match
    let base_exec = extract_base_executable(command);
    let base = std::path::Path::new(&base_exec)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(base_exec.as_str())
        .to_string();
    let cmd_lower = command.to_lowercase();

    // Git subcommand routing
    if base == "git" {
        return git::GitDistiller.distill(segments, input, session);
    }

    // Database tools
    if matches!(
        base.as_str(),
        "psql" | "mysql" | "sqlite3" | "pg_dump" | "redis-cli"
    ) {
        return database::DatabaseDistiller.distill(segments, input, session);
    }

    // Security scanners
    if matches!(
        base.as_str(),
        "semgrep" | "trivy" | "snyk" | "hadolint" | "gosec" | "bandit"
    ) {
        return security::SecurityDistiller.distill(segments, input, session);
    }

    // GitHub/VCS CLIs
    if matches!(base.as_str(), "gh" | "hub" | "glab") {
        return vcs::VcsDistiller.distill(segments, input, session);
    }

    // Java/JVM — use BuildDistiller (sudah ada)
    if matches!(
        base.as_str(),
        "java" | "javac" | "mvn" | "mvnw" | "gradle" | "gradlew"
    ) {
        if cmd_lower.contains("test") {
            return test::TestDistiller.distill(segments, input, session);
        }
        return build::BuildDistiller.distill(segments, input, session);
    }

    // Flutter/Dart
    if matches!(base.as_str(), "flutter" | "dart") {
        if cmd_lower.contains("test") || cmd_lower.contains("analyze") {
            return test::TestDistiller.distill(segments, input, session);
        }
        return build::BuildDistiller.distill(segments, input, session);
    }

    // Build tools → BuildDistiller
    if matches!(
        base.as_str(),
        "cargo"
            | "make"
            | "cmake"
            | "gcc"
            | "g++"
            | "clang"
            | "rustc"
            | "go"
            | "pip"
            | "pip3"
            | "ruff"
            | "mypy"
            | "black"
            | "ruby"
            | "rake"
            | "rubocop"
            | "dotnet"
            | "gradle"
            | "mvn"
            | "pytest"
            | "python"
            | "python3"
            | "rspec"
            | "phpunit"
    ) {
        // Tapi test → TestDistiller
        if cmd_lower.contains("test")
            || cmd_lower.contains("pytest")
            || matches!(base.as_str(), "pytest" | "rspec" | "phpunit")
        {
            return test::TestDistiller.distill(segments, input, session);
        }
        return build::BuildDistiller.distill(segments, input, session);
    }

    // JS/TS ecosystem → JsTsDistiller
    if matches!(
        base.as_str(),
        "vitest" | "playwright" | "tsc" | "eslint" | "prettier" | "jest" | "esbuild" | "vite"
    ) {
        return jsts::JsTsDistiller.distill(segments, input, session);
    }
    // npm/pnpm/yarn/bun: check subcommand
    if matches!(base.as_str(), "npm" | "npx" | "pnpm" | "yarn" | "bun") {
        if cmd_lower.contains("test")
            || cmd_lower.contains("vitest")
            || cmd_lower.contains("jest")
            || cmd_lower.contains("playwright")
        {
            return jsts::JsTsDistiller.distill(segments, input, session);
        }
        // install/build → still JsTs ecosystem (pnpm install, npm run build)
        return jsts::JsTsDistiller.distill(segments, input, session);
    }

    // Cloud & infra → CloudDistiller
    if matches!(
        base.as_str(),
        "docker"
            | "podman"
            | "kubectl"
            | "helm"
            | "terraform"
            | "tofu"
            | "aws"
            | "gcloud"
            | "az"
            | "doctl"
    ) {
        return cloud::CloudDistiller.distill(segments, input, session);
    }

    // Config file protection: avoid over-distilling small configs
    if base == "cat" && should_passthrough_config_output(command, input) {
        return input.to_string();
    }

    // System ops → SystemOpsDistiller
    if matches!(
        base.as_str(),
        "ls" | "tree"
            | "find"
            | "grep"
            | "rg"
            | "ps"
            | "df"
            | "du"
            | "env"
            | "stat"
            | "cat"
            | "head"
            | "tail"
            | "curl"
            | "wget"
            | "wc"
            | "sort"
            | "uniq"
            | "awk"
            | "sed"
            | "tar"
            | "zip"
            | "unzip"
            | "apt"
            | "apt-get"
            | "brew"
            | "yum"
            | "dnf"
    ) {
        return system_ops::SystemOpsDistiller.distill(segments, input, session);
    }

    // Phase 2: Fallback to generic distiller
    generic::GenericDistiller.distill(segments, input, session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::scorer;

    #[test]
    fn test_extract_base_executable_handles_quotes_and_env_prefixes() {
        assert_eq!(extract_base_executable("git diff"), "git");
        assert_eq!(
            extract_base_executable("\"/usr/local/bin/cargo\" build"),
            "/usr/local/bin/cargo"
        );
        assert_eq!(extract_base_executable("'cargo' test"), "cargo");
        assert_eq!(
            extract_base_executable("`/usr/bin/python3` -V"),
            "/usr/bin/python3"
        );
        assert_eq!(
            extract_base_executable("RUST_BACKTRACE=1 cargo test"),
            "cargo"
        );
        assert_eq!(
            extract_base_executable("env FOO=1 \"/path/to/git\" status"),
            "/path/to/git"
        );
    }

    #[test]
    fn vitest_zero_parse_passes_through_instead_of_claiming_success() {
        // #143 / #115: a Vite dev server (no tests) must never distill to a
        // green `vitest: ✓ 0/0 passed`. With nothing parsed, fail open.
        let input = include_str!("../../tests/fixtures/vite_dev_server.txt");
        let segments = scorer::score_with_command(input, "vitest", None);
        let output = distill_with_command(&segments, input, "vitest", None);

        assert!(
            !output.contains("0/0 passed"),
            "zero-parse produced a false success claim: {output:?}"
        );
        // The load-bearing signal (the bound port) survives.
        assert!(output.contains(":8080"), "dev-server port was dropped: {output:?}");
    }

    #[test]
    fn test_cat_small_config_passthrough() {
        let input = "[db]\nurl = \"postgres://example\"\nmax_connections = 10\n";
        let cmd = "cat config.toml";
        let segments = scorer::score_with_command(input, cmd, None);
        let output = distill_with_command(&segments, input, cmd, None);
        assert_eq!(output, input);
    }

    macro_rules! snapshot_test {
        ($name:ident, $file:expr, $cmd:expr) => {
            #[test]
            fn $name() {
                let input = include_str!(concat!("../../tests/fixtures/", $file));
                let segments = scorer::score_with_command(input, $cmd, None);
                let output = distill_with_command(&segments, input, $cmd, None);

                insta::assert_snapshot!(output);

                if $cmd == "git diff" {
                    assert!(
                        output.len() < input.len() * 60 / 100,
                        "Git diff distiller must achieve >40% reduction (now {} len vs initial {})",
                        output.len(),
                        input.len()
                    );
                }
            }
        };
    }

    snapshot_test!(
        test_git_diff_distillation,
        "git_diff_multi_file.txt",
        "git diff"
    );
    snapshot_test!(
        test_git_status_distillation,
        "git_status_dirty.txt",
        "git status"
    );
    snapshot_test!(
        test_cargo_build_distillation,
        "cargo_build_errors.txt",
        "cargo build"
    );
    snapshot_test!(test_pytest_distillation, "pytest_failures.txt", "pytest");
    snapshot_test!(
        test_kubectl_distillation,
        "kubectl_pods_mixed.txt",
        "kubectl get pods"
    );
    snapshot_test!(
        test_docker_build_distillation,
        "docker_build_layered.txt",
        "docker build"
    );
    snapshot_test!(
        test_nginx_log_distillation,
        "nginx_access_log.txt",
        "cat access.log"
    );
    snapshot_test!(test_cloud_kubectl, "kubectl_get_pods_mixed.txt", "kubectl");
    snapshot_test!(test_cloud_docker_ps, "docker_ps_mixed.txt", "docker ps");
    snapshot_test!(
        test_cloud_docker_build_error,
        "docker_build_error.txt",
        "docker build"
    );
    snapshot_test!(
        test_cloud_terraform_plan,
        "terraform_plan_cloud.txt",
        "terraform plan"
    );
    snapshot_test!(test_systemops_grep, "grep_recursive_output.txt", "grep -r");
    snapshot_test!(test_systemops_ls, "ls_la_output.txt", "ls -l");
    snapshot_test!(test_systemops_find, "find_project_output.txt", "find .");
    snapshot_test!(test_systemops_env, "env_output.txt", "env");

    snapshot_test!(test_jsts_vitest, "vitest_mixed.txt", "vitest");
    snapshot_test!(test_jsts_tsc, "tsc_errors.txt", "tsc");
    // #106: a composite `npm run <script>` (an `&&` chain) must NOT be claimed by a
    // single tool distiller — `tsc --` in npm's echo used to collapse the whole thing
    // to `tsc: no errors`. jsts declines composites (returns them for the pipeline's
    // generic collapse), so every gate's verdict survives.
    snapshot_test!(
        test_jsts_npm_composite,
        "npm_run_verify.txt",
        "npm run verify"
    );
    snapshot_test!(
        test_jsts_playwright,
        "playwright_fail.txt",
        "playwright test"
    );
    snapshot_test!(test_jsts_eslint, "eslint_errors.txt", "eslint");
    // #114: `prettier --write` (via `npm run format`) used to report `eslint: no
    // problems found` — `is_eslint_output` matched the filename `eslint.config.js`
    // in prettier's file list. It must now be recognised as prettier and summarised
    // per real prettier output (both modes), never as a clean run of another tool.
    snapshot_test!(
        test_jsts_prettier_write,
        "prettier_write.txt",
        "npm run format"
    );
    snapshot_test!(
        test_jsts_prettier_check,
        "prettier_check.txt",
        "prettier --check ."
    );

    snapshot_test!(
        test_database_psql_error,
        "psql_error.txt",
        "psql -U postgres mydb"
    );
    snapshot_test!(
        test_security_trivy_scan,
        "trivy_output.txt",
        "trivy image myapp:latest"
    );
}
