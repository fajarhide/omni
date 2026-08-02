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

/// The subcommand of a `cargo` invocation, skipping env-var prefixes, the path
/// the binary was called by, a `+toolchain` override, and any leading flags.
///
/// `None` when there is no subcommand at all (`cargo`, `cargo --version`), which
/// routes to passthrough — the honest answer for output we cannot classify.
fn cargo_subcommand(command: &str) -> Option<&str> {
    let mut tokens = command.split_whitespace().skip_while(|t| {
        // `RUST_BACKTRACE=1 cargo test`, and the cargo path itself.
        let is_env_assignment = t.contains('=') && !t.starts_with('-');
        let is_cargo = std::path::Path::new(t.trim_matches(['"', '\'', '`']))
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "cargo" || n == "cargo.exe")
            .unwrap_or(false);
        is_env_assignment || !is_cargo
    });
    tokens.next()?; // the cargo token itself

    // `cargo +nightly tree`, `cargo --offline tree`
    tokens.find(|t| !t.starts_with('-') && !t.starts_with('+'))
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

/// Commands whose output `distill_with_command` hands back byte-for-byte, and
/// which the hooks must therefore also exempt from the collapse fallback.
///
/// The three halves reach the same rule for different reasons:
///
/// * **Enumeration** (`ls`, `find`, `ps`, `wc`, `df`, `du`, `stat`, `tree`, bare
///   `env`) lists distinct data one datum per line. The items *are* the answer
///   and there is no noise to drop (#198/#200).
/// * **Bare interpreters** (`python`, `python3`, `ruby`) run an arbitrary
///   program whose stdout is the answer, not a build log (#190).
/// * **File readers** (`cat`, `head`, `tail`, `sed`, `awk`) emit whatever the
///   file holds, so any shape a summariser recognises in it is a coincidence
///   (#235/#236).
///
/// The single list is the point. A passthrough returns its input, so it can
/// never beat `beats_guardrail`, so the hooks treat it as a distiller that
/// punted and collapse it anyway — which put 40 distinct data rows behind one
/// `[N similar lines collapsed]` marker and reported 95.7% saved (#214). Adding
/// a passthrough arm without adding it here is that bug, so both live in one
/// predicate rather than in a routing arm and a guard that can drift apart.
///
/// `grep`/`rg` are deliberately excluded: their distiller hoists the repeated
/// path losslessly and keeps every match.
pub fn passes_through_verbatim(command: &str) -> bool {
    let base_exec = extract_base_executable(command);
    let base = std::path::Path::new(&base_exec)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(base_exec.as_str());
    matches!(
        base,
        "ls" | "tree"
            | "find"
            | "ps"
            | "df"
            | "du"
            | "stat"
            | "wc"
            | "python"
            | "python3"
            | "ruby"
            // File readers. Their stdout is whatever the file holds, so there is
            // no grammar to distil and every shape a summariser recognises in it
            // is a coincidence. `cat docs/DEVELOPMENT.md` — 127 lines of prose —
            // came back as `tree: 127 entries` because one code block inside the
            // document drew a directory layout (#236), and `sed -n '285,340p'` of
            // a Swift file lost 37 of 56 lines to the enumeration cut (#235).
            // Same reasoning as the interpreters above: an arbitrary payload.
            | "cat"
            | "head"
            | "tail"
            | "sed"
            | "awk"
    )
        // `extract_base_executable` strips a leading `env`/`command` wrapper, so
        // bare `env` and `command env` both leave base empty — match on any `env`
        // token in that case rather than only the first word (review of #203).
        || (base.is_empty() && command.split_whitespace().any(|t| t == "env"))
        // A git command asked for a file list by flag rather than by name (#231).
        // It belongs here and not only in the routing arm: a passthrough that the
        // collapse fallback then folds up is #214, and the file rows of a
        // `--stat` all share a shape.
        || (base == "git" && git_enumerates_files(command))
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

/// `gh` and `glab` are wrappers: `gh pr list` prints the enumeration
/// `VcsDistiller` was written to summarise, and `gh api …` prints whatever the
/// endpoint returned. The distiller cuts to the first 10 lines on size alone,
/// with no grammar check, so a Swift file fetched through
/// `gh api … | base64 -d | sed -n '285,340p'` lost 37 of its 56 lines under
/// `... [37 more items — use --limit to see more]` — a flag neither `gh api`
/// nor `sed` has, so the suggested recovery could not be run (#235). The same
/// cut took a Homebrew cask down to 10 lines and stripped its blank lines on
/// the way (#226).
///
/// `--limit` is the tell: it belongs to the `list` subcommands and nothing else,
/// so those are the only outputs this distiller can honestly claim.
fn is_vcs_list_command(command: &str) -> bool {
    command.split_whitespace().any(|t| t == "list")
}

/// `--stat`, `--numstat`, `--name-only` and `--name-status` exist only to make
/// git emit a file list. The flag is a reliable signal in a way the subcommand
/// is not: `git show` without one prints a diff and is legitimately distillable,
/// and with one it prints the enumeration that *is* the answer.
///
/// `git show --stat` reached `distill_log`, because the input holds no
/// `diff --git` and no `On branch`. There the `--oneline` subject matched
/// `RE_GIT_LOG_HASH` and was kept, while the stat rows fell past every arm to
/// the rule that drops body lines before the next commit (#199) — correct for
/// `git log`, wrong for `git show`, where that position holds the payload. The
/// fail-open guard below `distill_log` then did not fire, because `result` was
/// non-empty: one line *had* been recognised. That is #228 again in a different
/// distiller — a partial recognition disarming a guard whose condition is
/// "recognised nothing" — and it published a 79.9% saving for a commit whose
/// four files, three of them screenshots, had all vanished from the answer
/// (#231).
fn git_enumerates_files(command: &str) -> bool {
    command.split_whitespace().any(|t| {
        matches!(
            t,
            "--stat" | "--numstat" | "--shortstat" | "--name-only" | "--name-status"
        )
    })
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
    // A chain's stdout belongs to several programs and arrives as one stream, so
    // there is no honest way to hand it to the distiller named by the first of
    // them: `git status && echo === && find .` came back as the git one-liner
    // with the `find` output deleted, unmarked (#264). One producer routes;
    // several pass through.
    let Some(command) = crate::pipeline::registry::sole_output_command(command) else {
        return input.to_string();
    };

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
        if git_enumerates_files(command) {
            return input.to_string();
        }
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
        if !is_vcs_list_command(command) {
            return input.to_string();
        }
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
            | "rake"
            | "rubocop"
            | "dotnet"
            | "gradle"
            | "mvn"
            | "pytest"
            | "rspec"
            | "phpunit"
    ) {
        // `cargo` is routed by subcommand, not by executable (#170). Most of
        // cargo is not a build: `cargo tree` prints a dependency tree that *is*
        // the answer, and `BuildDistiller` summarised 21.4 KB of it as
        // `Build: ok` — 9 bytes — reported as a 100% reduction. A command whose
        // output is data must not be handed to a distiller that only knows how
        // to count compile steps.
        if base == "cargo" {
            return match cargo_subcommand(command) {
                Some("test" | "bench") => test::TestDistiller.distill(segments, input, session),
                Some(
                    "build" | "check" | "run" | "clippy" | "rustc" | "fix" | "doc" | "install",
                ) => build::BuildDistiller.distill(segments, input, session),
                // tree, metadata, search, pkgid, locate-project, vendor, add,
                // remove, update, publish, … — output is the answer, hand it back.
                _ => input.to_string(),
            };
        }

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
        return cloud::CloudDistiller { tool: &base }.distill(segments, input, session);
    }

    // Enumeration commands list distinct data, one datum per line, with no noise
    // to drop — the paths, filenames, processes and mounts *are* the answer. A
    // command audit (docs/COMMAND_AUDIT.md) measured every one of these either
    // dropping items (`find` 68/98 paths #198, `ls` every filename, `ps` most
    // processes, `wc` per-file counts) or *growing* the output (`env` +50%,
    // `df` +16%). There is nothing to distill losslessly here, so hand the output
    // back verbatim rather than truncate it to a shorter, plausible, incomplete
    // list (#200). `grep`/`rg` are NOT here: their distiller hoists the repeated
    // path losslessly and keeps every match.
    //
    // The bare interpreters ride the same predicate: `python3 -c "..."` handed to
    // BuildDistiller fabricated `Build: ok` for any script that printed no
    // error line (#190). They sit here rather than in an arm of their own so the
    // hooks' collapse exemption cannot drift away from the routing (#214). No
    // `contains("test")` shortcut to TestDistiller for them, either: it matched
    // inside a `-c` code argument or a path segment (`ruby /proj/contest/x.rb`),
    // and TestDistiller fabricates too, which is #190 wearing another distiller's
    // name. Real runners are handled upstream, where `signals/tools/pytest.toml`
    // and `mypy.toml` are TOML-first and shadow this path for `python -m pytest`.
    if passes_through_verbatim(command) {
        return input.to_string();
    }

    // System ops → SystemOpsDistiller
    if matches!(
        base.as_str(),
        "grep"
            | "rg"
            | "curl"
            | "wget"
            | "sort"
            | "uniq"
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
    fn reads_the_cargo_subcommand_past_prefixes_and_flags() {
        assert_eq!(cargo_subcommand("cargo tree"), Some("tree"));
        assert_eq!(cargo_subcommand("cargo tree --depth 1"), Some("tree"));
        assert_eq!(
            cargo_subcommand("RUST_BACKTRACE=1 cargo test"),
            Some("test")
        );
        assert_eq!(cargo_subcommand("cargo +nightly build"), Some("build"));
        assert_eq!(cargo_subcommand("cargo --offline tree"), Some("tree"));
        assert_eq!(
            cargo_subcommand("/usr/local/bin/cargo clippy"),
            Some("clippy")
        );
        // No subcommand at all — routes to passthrough, which is the honest
        // answer for output we cannot classify.
        assert_eq!(cargo_subcommand("cargo"), None);
        assert_eq!(cargo_subcommand("cargo --version"), None);
    }

    /// From #170: `cargo tree` prints a dependency tree that *is* the answer.
    /// Routing every `cargo` command to `BuildDistiller` summarised 21.4 KB of
    /// it as `Build: ok` — 9 bytes — and reported a 100% reduction as a win.
    #[test]
    fn hands_back_the_output_of_cargo_commands_that_are_not_builds() {
        let tree = "omni v0.6.3 (/repo)\n\
                    ├── anyhow v1.0.100\n\
                    ├── chrono v0.4.42\n\
                    │   ├── iana-time-zone v0.1.64\n\
                    │   └── num-traits v0.2.19\n\
                    └── serde v1.0.228\n";

        for cmd in [
            "cargo tree",
            "cargo metadata --no-deps",
            "cargo search serde",
            "cargo pkgid",
        ] {
            assert_eq!(
                distill_with_command(&[], tree, cmd, None),
                tree,
                "`{cmd}` output is data, not build progress — it must survive"
            );
        }
    }

    /// From #190: a `python3 -c "..."` script prints arbitrary output that *is*
    /// the answer. Routing bare interpreters to `BuildDistiller` fabricated
    /// `Build: ok` for any script with no error/warning line — inventing success
    /// for a verification command whose real answer was the printed text.
    #[test]
    fn hands_back_the_output_of_bare_script_interpreters() {
        let script_out = " tokenRefs : ['vmuser-a']\n VSS dests : ['vault-a']\n dangling  : NONE\n";

        for cmd in [
            "python3 -c \"import re; print('dangling  : NONE')\"",
            "python audit.py",
            "ruby check.rb",
            // The substring "test" inside a `-c` arg or a path segment must NOT
            // route to TestDistiller — it fabricates `Tests: 1 passed` on this
            // output, which is #190 via a different distiller. Boundary locked.
            "python3 -c \"testing_flag = True; print('config ok')\"",
            "ruby /projects/contest/verify.rb",
        ] {
            let segments = scorer::score_with_command(script_out, cmd, None);
            let out = distill_with_command(&segments, script_out, cmd, None);
            assert_eq!(
                out, script_out,
                "`{cmd}` output is data, not build progress — it must survive verbatim"
            );
            assert_ne!(
                out.trim(),
                "Build: ok",
                "`{cmd}` fabricated a build verdict"
            );
        }
    }

    #[test]
    fn still_routes_real_cargo_builds_and_tests_to_their_distillers() {
        // Guards the other direction: the fix must not turn every cargo command
        // into passthrough and quietly end all savings on the ones that work.
        let noisy_build: String = (0..40)
            .map(|i| format!("   Compiling crate_{i} v0.1.0\n"))
            .collect::<String>()
            + "    Finished `dev` profile [unoptimized] target(s) in 12.61s\n";

        let out = distill_with_command(&[], &noisy_build, "cargo build", None);
        assert!(
            out.len() < noisy_build.len(),
            "cargo build must still be distilled, got {out:?}"
        );
    }

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
        assert!(
            output.contains(":8080"),
            "dev-server port was dropped: {output:?}"
        );
    }

    // #143 umbrella: each distiller below is routed a payload that trips its
    // content detector but carries no signal it can actually parse. The zero-state
    // must pass the input through, never emit a confident green string. Each asserts
    // the false claim is absent AND the original content survives (passthrough).

    #[test]
    fn tsc_zero_parse_passes_through_instead_of_no_errors() {
        // `tsc --` command echo trips is_tsc_output, but there is no `error TS`
        // line to parse — the #106 shape.
        let input = "> tsc --noEmit\nresolving project references, nothing to compile\n";
        let segments = scorer::score_with_command(input, "tsc", None);
        let output = distill_with_command(&segments, input, "tsc", None);
        assert!(
            !output.contains("tsc: no errors"),
            "false claim: {output:?}"
        );
        assert!(output.contains("nothing to compile"), "dropped: {output:?}");
    }

    #[test]
    fn playwright_zero_parse_passes_through_instead_of_passed() {
        // A `[chromium]` banner trips is_playwright_output with no result summary.
        let input = "[chromium] › launching browser\nno spec files matched the filter\n";
        let segments = scorer::score_with_command(input, "playwright", None);
        let output = distill_with_command(&segments, input, "playwright", None);
        assert!(!output.contains("0/0 passed"), "false claim: {output:?}");
        assert!(output.contains("no spec files"), "dropped: {output:?}");
    }

    #[test]
    fn eslint_zero_parse_passes_through_instead_of_no_problems() {
        // A banner naming an eslint rule id trips is_eslint_output, but there is no
        // finding line or `problems (` summary to parse — the #114 shape.
        let input = "Oxc linter v0.15\nusing preset @typescript-eslint/recommended\n";
        let segments = scorer::score_with_command(input, "eslint", None);
        let output = distill_with_command(&segments, input, "eslint", None);
        assert!(
            !output.contains("no problems found"),
            "false claim: {output:?}"
        );
        assert!(output.contains("Oxc linter"), "dropped: {output:?}");
    }

    #[test]
    fn security_zero_parse_passes_through_instead_of_no_issues() {
        // A clean scan with no severity token must not be certified `no issues`.
        let input = "trivy image myapp:latest\nscanning filesystem, database up to date\n";
        let segments = scorer::score_with_command(input, "trivy", None);
        let output = distill_with_command(&segments, input, "trivy", None);
        assert!(
            !output.contains("no issues found"),
            "false claim: {output:?}"
        );
        assert!(
            output.contains("database up to date"),
            "dropped: {output:?}"
        );
    }

    #[test]
    fn docker_logs_zero_parse_passes_through_instead_of_no_errors_detected() {
        // A manifest that merely mentions "docker logs" routes to distill_docker_logs
        // but is not log-shaped (is_docker_logs false) — the #112 misroute.
        let input =
            "apiVersion: v1\nkind: Pod\n# inspect with: docker logs app\nmetadata:\n  name: app\n";
        let segments = scorer::score_with_command(input, "docker", None);
        let output = distill_with_command(&segments, input, "docker", None);
        assert!(
            !output.contains("no errors detected"),
            "false claim: {output:?}"
        );
        assert!(output.contains("kind: Pod"), "dropped: {output:?}");
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
    // #107: `--oneline` puts the hash and subject on one line. The distiller used
    // to keep 7 chars of hash and drop the subject, joining every commit into a
    // wall of hashes. This locks the whole line surviving — there was no git_log
    // snapshot before, which is why the regression shipped unseen.
    snapshot_test!(
        test_git_log_oneline_distillation,
        "git_log.txt",
        "git log --oneline"
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

    // #198/#200: `ls`, `find` and `env` are enumerations — every line is the
    // answer, not noise — so they must pass through verbatim, never be summarised
    // or truncated. (These replaced snapshot tests that asserted the old lossy
    // distillation; `grep` above stays snapshotted because its hoisting is
    // lossless.)
    macro_rules! passthrough_test {
        ($name:ident, $file:expr, $cmd:expr) => {
            #[test]
            fn $name() {
                let input = include_str!(concat!("../../tests/fixtures/", $file));
                let segments = scorer::score_with_command(input, $cmd, None);
                let output = distill_with_command(&segments, input, $cmd, None);
                assert_eq!(
                    output, input,
                    concat!(
                        $cmd,
                        " is an enumeration; it must pass through verbatim (#200)"
                    )
                );
            }
        };
    }
    passthrough_test!(ls_passes_through_verbatim, "ls_la_output.txt", "ls -l");
    passthrough_test!(
        find_passes_through_verbatim,
        "find_project_output.txt",
        "find ."
    );
    passthrough_test!(env_passes_through_verbatim, "env_output.txt", "env");

    /// Review of #205: bare `env` and `command env` both leave `base` empty (the
    /// wrapper is stripped), so the guard matches on any `env` token. Locks that
    /// in — a narrowing back to "first word only" would silently drop `command
    /// env` passthrough.
    #[test]
    fn treats_bare_and_wrapped_env_as_enumeration() {
        assert!(passes_through_verbatim("env"));
        assert!(passes_through_verbatim("command env"));
        assert!(passes_through_verbatim("ls -la"));
        assert!(!passes_through_verbatim("grep -r foo"));
        assert!(!passes_through_verbatim("echo hi"));
    }

    /// #214 moved the interpreters into this predicate and deleted their own
    /// routing arm, so the predicate is now the only thing keeping them out of
    /// `BuildDistiller` *and* out of the hooks' collapse fallback. Dropping one
    /// entry costs both at once, which is worth catching here rather than in the
    /// integration test, where it only shows up after it has propagated.
    #[test]
    fn treats_bare_script_interpreters_as_passthrough() {
        assert!(passes_through_verbatim("python3 -c \"print('x')\""));
        assert!(passes_through_verbatim("python audit.py"));
        assert!(passes_through_verbatim("ruby /proj/contest/verify.rb"));
        assert!(passes_through_verbatim("/usr/bin/python3 gen.py"));
        // `pip` and `rake` stay on the build path: their output is task oriented.
        assert!(!passes_through_verbatim("pip install requests"));
        assert!(!passes_through_verbatim("rake db:migrate"));
    }

    /// #236: a prose document that happens to quote a directory layout was
    /// classified by the one line holding a box-drawing character and delivered
    /// as `tree: N entries` — a different *kind* of thing, asserted with no
    /// marker. Reading a file is not a grammar, so the readers pass through.
    #[test]
    fn hands_back_a_document_that_quotes_a_directory_tree() {
        let doc = "# Development Guide\n\
                   \n\
                   Guide for contributors working on the OMNI codebase.\n\
                   \n\
                   ## Layout\n\
                   \n\
                   ```\n\
                   src/\n\
                   ├── main.rs\n\
                   ├── pipeline/\n\
                   └── distillers/\n\
                   ```\n\
                   \n\
                   ## Verification — run these yourself, CI does not\n\
                   \n\
                   Run `yamllint` and `kubeconform` before committing manifests.\n\
                   CI validates changed files individually and never builds an overlay.\n";

        for cmd in [
            "cat docs/DEVELOPMENT.md",
            "head -80 docs/DEVELOPMENT.md",
            "tail -60 CLAUDE.md",
            "awk 'NR>10' docs/DEVELOPMENT.md",
        ] {
            let segments = scorer::score_with_command(doc, cmd, None);
            assert_eq!(
                distill_with_command(&segments, doc, cmd, None),
                doc,
                "`{cmd}` must hand the document back verbatim (#236)"
            );
        }
    }

    /// #231: `--stat` exists only to produce the file list, and `distill_log`
    /// dropped every row of it while keeping the `--oneline` subject — so the
    /// output stayed well-formed, got shorter, and read as a complete answer to
    /// a question it no longer answered. The reporter was checking whether
    /// `git add -A` had swept three screenshots into a commit; it came back
    /// naming no files.
    #[test]
    fn hands_back_the_file_rows_git_was_asked_to_enumerate() {
        let stat = "9cd7a80 docs(changelog): record the #228 pass-counter fix\n \
                    CHANGELOG.md | 1 +\n \
                    media/shot-a.png | Bin 0 -> 12043 bytes\n \
                    media/shot-b.png | Bin 0 -> 9981 bytes\n \
                    media/shot-c.png | Bin 0 -> 11202 bytes\n \
                    4 files changed, 1 insertion(+)\n";

        for cmd in [
            "git show --stat --oneline HEAD",
            "git log --stat -3",
            "git show --name-only HEAD",
            "git diff --numstat main",
        ] {
            let segments = scorer::score_with_command(stat, cmd, None);
            assert_eq!(
                distill_with_command(&segments, stat, cmd, None),
                stat,
                "`{cmd}` asked for the file list; it must survive (#231)"
            );
        }
    }

    /// #235: a contiguous line range of source code is what the caller already
    /// chose, so there is nothing to cut. The reported loss was 37 of 56 lines,
    /// under a marker suggesting a `--limit` flag neither command has.
    ///
    /// The reported pipeline leads with `gh`, so `VcsDistiller` claimed it — the
    /// bare `sed` form goes through the reader arm. Both are covered here
    /// because either alone leaves the reproduction broken.
    #[test]
    fn hands_back_a_source_line_range_however_it_was_fetched() {
        let mut src = String::new();
        for i in 0..56 {
            src.push_str(&format!(
                "    self.frame.origin.x = offset + {i}.0 * spacing\n"
            ));
        }

        for cmd in [
            "sed -n '285,340p' Kit/module/module.swift",
            "gh api repos/o/r/contents/Kit/module.swift --jq '.content' | base64 -d | sed -n '285,340p'",
            "gh api repos/fajarhide/homebrew-tap/contents/Casks/bubo.rb --jq '.content' | base64 -d",
        ] {
            let segments = scorer::score_with_command(&src, cmd, None);
            assert_eq!(
                distill_with_command(&segments, &src, cmd, None),
                src,
                "`{cmd}` must not be cut to an enumeration (#235, #226)"
            );
        }
    }

    /// The gate must not disarm the distiller it guards: a real `gh pr list` is
    /// still an enumeration `--limit` can re-fetch, so it still summarises.
    #[test]
    fn still_summarises_a_real_gh_list() {
        let mut listing = String::new();
        for i in 1..=25 {
            listing.push_str(&format!(
                "#{i}\tSome pull request title\tbranch-{i}\tOPEN\n"
            ));
        }

        let cmd = "gh pr list --limit 25";
        let segments = scorer::score_with_command(&listing, cmd, None);
        let out = distill_with_command(&segments, &listing, cmd, None);
        assert!(
            out.contains("more items"),
            "a gh list should still be summarised:\n{out}"
        );
    }

    /// The gate must not disarm the distiller it guards: a plain `git log` is
    /// still summarised, so the fix is not a blanket passthrough for `git`.
    #[test]
    fn still_distills_a_plain_git_log() {
        let log = "commit a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0\n\
                   Author: Someone <someone@example.com>\n\
                   Date:   Mon Jul 27 10:00:00 2026 +0700\n\
                   \n\
                       fix(collapse): keep a group inside its section\n\
                   \n\
                   commit b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1\n\
                   Author: Someone <someone@example.com>\n\
                   Date:   Mon Jul 27 09:00:00 2026 +0700\n\
                   \n\
                       test(perf): widen the latency budget\n";

        let cmd = "git log -2";
        let segments = scorer::score_with_command(log, cmd, None);
        let out = distill_with_command(&segments, log, cmd, None);

        assert!(
            !out.contains("Author:"),
            "plain `git log` must still drop metadata:\n{out}"
        );
        assert!(
            out.contains("fix(collapse)") && out.contains("test(perf)"),
            "every subject must survive:\n{out}"
        );
    }

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
    // #216: the generic arm answered a schema dump with `DB: ok (N lines
    // output)` — a verdict invented in the success direction over a payload it
    // never read. With no error segment and nothing tabular parsed it must hand
    // the DDL back verbatim.
    snapshot_test!(
        database_sqlite_schema_passes_through_verbatim,
        "sql_create.txt",
        "sqlite3 app.db .schema"
    );
    snapshot_test!(
        test_security_trivy_scan,
        "trivy_output.txt",
        "trivy image myapp:latest"
    );
}
