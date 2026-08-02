use crate::pipeline::{CollapseMode, SegmentationMode};

pub struct ToolProfile {
    pub segmentation: SegmentationMode,
    pub collapse: CollapseMode,
}

impl Default for ToolProfile {
    fn default() -> Self {
        Self {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Generic,
        }
    }
}

/// Shell builtins that write nothing to stdout, so their presence in a chain
/// says nothing about who produced the output being distilled.
///
/// Deliberately short. Every name here is silent by definition, not merely quiet
/// in the common case: `mkdir`, `cp` and `rm` all print under `-v`, and putting
/// them here would let a chain be routed to a single distiller again. Leaving a
/// producer out costs a passthrough; letting one in costs the answer.
const SILENT_BUILTINS: &[&str] = &[
    "cd", "export", "set", "unset", "source", ".", "true", "false", "pushd", "popd", "umask",
    "alias", "shift", "local", "readonly",
];

/// The one command in `command` whose stdout is being distilled, or `None` when
/// several produced it.
///
/// `distill_with_command` reads the first executable of the command string and
/// hands that distiller the whole of stdout. On a chain the rest of the output
/// belongs to other programs: `git status && echo === && find .` came back as
/// `git: on branch main | staged:0 mod:0 untracked:0`, so the 40 lines of `find`
/// that the command was run for were deleted with no marker, no count and no
/// rewind hash, and the ratio read as a 99% win on the bytes that held the answer
/// (#264). `git status` is the worst case only because its distiller emits a
/// fixed one-liner whatever the input, leaving no residue to notice.
///
/// Splitting stdout back onto the chain is not possible: it is one stream with
/// nothing marking which program wrote which line. So the rule is the honest one.
/// One producer, route it. More than one, the caller passes the output through
/// untouched.
///
/// A pipeline resolves to its first stage, with one exception. Most filters
/// preserve the shape of what they are fed, so `kubectl get pods | head -20` is
/// still a pod table and still belongs to `kubectl`. `jq` and `yq` do not: they
/// rewrite the payload into something of their own, so the output is theirs.
/// Routing it upstream is how `kubectl get pod -o json | jq -r '...'` reached the
/// cloud distiller, which kept one arbitrary row of four and dropped the three
/// that carried the answer (#269).
pub fn sole_output_command(command: &str) -> Option<&str> {
    let segments = split_sequential(command);
    let producer = match segments.len() {
        0 => return None,
        1 => segments[0],
        _ => {
            let mut producers = segments.into_iter().filter(|seg| !is_silent(seg));
            let first = producers.next()?;
            producers.next().is_none().then_some(first)?
        }
    };
    Some(reshaped_by(producer).unwrap_or(producer))
}

/// The trailing pipeline stage when it rewrites the payload rather than
/// selecting from it, so the output stops belonging to whatever fed it.
///
/// Deliberately two names. Measured over 5,143 distinct recorded commands, 1,035
/// are pipelines, and routing every one of them by its last stage would hand 871
/// to `head`, `tail` or `sed` and stop distilling them at all. Those filters cut
/// rows out of a shape they leave intact, which is the opposite of what `jq` and
/// `yq` do. Where the general rule belongs is its own question, with its own
/// measurement.
fn reshaped_by(segment: &str) -> Option<&str> {
    let last = split_pipeline(segment).pop()?;
    let base = last
        .split_whitespace()
        .next()
        .map(|w| w.trim_matches(|c| c == '"' || c == '\''))?;
    matches!(base, "jq" | "yq").then_some(last)
}

/// Splits on unquoted single `|`, the pipe operator. `||` is a sequential
/// operator and `split_sequential` has already dealt with it.
fn split_pipeline(segment: &str) -> Vec<&str> {
    let bytes = segment.as_bytes();
    let mut stages = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut quote: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'\'' | b'"' | b'`' => quote = Some(b),
                b'\\' => i += 1,
                b'|' => {
                    push_segment(&mut stages, segment, start, i);
                    start = i + 1;
                }
                _ => {}
            },
        }
        i += 1;
    }
    push_segment(&mut stages, segment, start, bytes.len());
    stages
}

fn is_silent(segment: &str) -> bool {
    segment
        .split_whitespace()
        .next()
        .map(|w| w.trim_matches(|c| c == '"' || c == '\''))
        .is_some_and(|base| SILENT_BUILTINS.contains(&base))
}

/// Splits on unquoted `&&`, `||`, `;` and newlines, the operators that run
/// commands one after another so each one can write to stdout.
///
/// Quote tracking is what stops `echo "a && b"` from reading as two commands. It
/// is deliberately one-directional: an unbalanced quote leaves the scanner inside
/// a string and yields one segment, which routes as it does today rather than
/// inventing a split.
fn split_sequential(command: &str) -> Vec<&str> {
    let bytes = command.as_bytes();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut quote: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                } else if b == b'\\' && q == b'"' {
                    i += 1;
                }
            }
            None => match b {
                b'\'' | b'"' | b'`' => quote = Some(b),
                b'\\' => i += 1,
                b'\n' | b';' => {
                    push_segment(&mut segments, command, start, i);
                    start = i + 1;
                }
                b'&' | b'|' if i + 1 < bytes.len() && bytes[i + 1] == b => {
                    push_segment(&mut segments, command, start, i);
                    i += 1;
                    start = i + 1;
                }
                _ => {}
            },
        }
        i += 1;
    }
    push_segment(&mut segments, command, start, bytes.len());
    segments
}

// Safety: `start` and `end` only ever come from positions of `;`, `\n`, `&`, `|`
// or the string's own length. Those are ASCII, and every byte inside a multi-byte
// UTF-8 sequence is >= 0x80, so none of them can match. The escape skip can leave
// the cursor mid-character, but a continuation byte matches no separator either,
// so the recorded bounds stay on char boundaries.
#[allow(clippy::string_slice)]
fn push_segment<'a>(out: &mut Vec<&'a str>, command: &'a str, start: usize, end: usize) {
    let seg = command[start..end].trim();
    if !seg.is_empty() {
        out.push(seg);
    }
}

pub fn resolve_profile(command: &str) -> ToolProfile {
    if command.is_empty() {
        return ToolProfile::default();
    }

    let cmd = command.trim();
    let base = {
        let first_word = cmd
            .split_whitespace()
            .next()
            .unwrap_or(cmd)
            .trim_matches(|c| c == '"' || c == '\'');
        std::path::Path::new(first_word)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(first_word)
    };
    let cmd_lower = cmd.to_lowercase();

    // 1. Git — Hunk based
    if base == "git" {
        let parts: Vec<&str> = cmd_lower.split_whitespace().collect();
        let sub = parts.get(1).copied().unwrap_or("");
        match sub {
            "diff" | "show" | "whatchanged" if !cmd_lower.contains("--stat") => {
                return ToolProfile {
                    segmentation: SegmentationMode::GitHunk,
                    collapse: CollapseMode::Generic,
                };
            }
            _ => {}
        }
    }

    // 2. Test Runners — Outcome based
    if matches!(
        base,
        "pytest" | "rspec" | "phpunit" | "jest" | "vitest" | "playwright"
    ) {
        return ToolProfile {
            segmentation: SegmentationMode::TestGroup,
            collapse: CollapseMode::Test,
        };
    }
    if (base == "go" || base == "npm" || base == "yarn" || base == "pnpm")
        && (cmd_lower.contains("test") || cmd_lower.contains("check"))
    {
        return ToolProfile {
            segmentation: SegmentationMode::TestGroup,
            collapse: CollapseMode::Test,
        };
    }

    // Cargo subcommand awareness
    if base == "cargo" {
        let sub = cmd_lower.split_whitespace().nth(1).unwrap_or("");
        return match sub {
            "test" | "nextest" => ToolProfile {
                segmentation: SegmentationMode::TestGroup,
                collapse: CollapseMode::Test,
            },
            "clippy" | "check" => ToolProfile {
                segmentation: SegmentationMode::Line,
                collapse: CollapseMode::Build, // clippy warnings treated like build
            },
            "bench" => ToolProfile {
                segmentation: SegmentationMode::TestGroup,
                collapse: CollapseMode::Test,
            },
            _ => ToolProfile {
                segmentation: SegmentationMode::Line,
                collapse: CollapseMode::Build,
            },
        };
    }

    // 3. Build Tools — Build collapse
    if matches!(
        base,
        "rustc"
            | "make"
            | "cmake"
            | "gcc"
            | "g++"
            | "clang"
            | "go"
            | "pip"
            | "pip3"
            | "ruby"
            | "rake"
            | "bundle"
    ) {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Build,
        };
    }

    // 4. Cloud & Infra — Infra collapse
    if matches!(
        base,
        "docker" | "podman" | "kubectl" | "helm" | "terraform" | "tofu" | "aws" | "gcloud" | "az"
    ) {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Infra,
        };
    }

    // 5. System Ops & Logs — Log collapse
    if matches!(base, "grep" | "rg" | "cat" | "tail" | "head" | "curl")
        || cmd_lower.contains(".log")
    {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Log,
        };
    }

    // 6. Database Tools — Log/tabular collapse
    if matches!(
        base,
        "psql"
            | "mysql"
            | "sqlite3"
            | "pg_dump"
            | "pg_restore"
            | "mongodump"
            | "redis-cli"
            | "clickhouse"
    ) {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Log,
        };
    }

    // 7. Java/JVM Ecosystem — Build collapse
    if matches!(
        base,
        "java" | "javac" | "mvn" | "gradle" | "gradlew" | "mvnw" | "kotlin" | "kotlinc"
    ) {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Build,
        };
    }
    // JVM test runners
    if matches!(base, "mvn" | "gradle" | "gradlew") && cmd_lower.contains("test") {
        return ToolProfile {
            segmentation: SegmentationMode::TestGroup,
            collapse: CollapseMode::Test,
        };
    }

    // 8. Mobile Development
    if matches!(base, "flutter" | "dart") {
        if cmd_lower.contains("test") || cmd_lower.contains("analyze") {
            return ToolProfile {
                segmentation: SegmentationMode::TestGroup,
                collapse: CollapseMode::Test,
            };
        }
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Build,
        };
    }
    if matches!(base, "swift" | "xcodebuild" | "xcode-select") {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Build,
        };
    }

    // 9. Monorepo & Modern Build Tools
    if matches!(base, "nx" | "turbo" | "bazel" | "pants" | "buck") {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Build,
        };
    }

    // 10. GitHub & VCS Tools
    if matches!(base, "gh" | "hub" | "glab") {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Log,
        };
    }

    // 11. Extended Cloud & K8s Dev Tools
    if matches!(
        base,
        "skaffold"
            | "argocd"
            | "flux"
            | "k3s"
            | "k3d"
            | "kind"
            | "minikube"
            | "kustomize"
            | "cdk"
            | "pulumi"
            | "serverless"
            | "sam"
    ) {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Infra,
        };
    }

    // 12. Additional Security & Quality Tools
    if matches!(
        base,
        "semgrep" | "trivy" | "snyk" | "hadolint" | "gosec" | "bandit"
    ) {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Build, // treat like lint/build errors
        };
    }

    // 13. Deno & Bun — Runtime tests
    if base == "deno" {
        if cmd_lower.contains("test") || cmd_lower.contains("check") {
            return ToolProfile {
                segmentation: SegmentationMode::TestGroup,
                collapse: CollapseMode::Test,
            };
        }
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Build,
        };
    }

    // 14. Network & System Monitoring
    if matches!(
        base,
        "ping" | "traceroute" | "nmap" | "netstat" | "ss" | "tcpdump" | "htop" | "top"
    ) {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Log,
        };
    }

    // 15. Database Migration Tools
    if matches!(
        base,
        "alembic" | "flyway" | "liquibase" | "knex" | "typeorm" | "sequelize"
    ) {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Log,
        };
    }

    // 16. CI/CD Tools
    if matches!(base, "act" | "circleci" | "drone" | "woodpecker" | "tekton") {
        return ToolProfile {
            segmentation: SegmentationMode::Line,
            collapse: CollapseMode::Build,
        };
    }

    ToolProfile::default()
}

/// For command chains (&&, ||, |, ;), return the profile of the most relevant command.
/// “Most relevant” = the last command that is not a simple pipe filter,
/// or the first command if all are equally important.
/// Example:
///   "cargo build && ./app"      → profile from "cargo build"
///   "npm install && npm test"   → profile from "npm test" (test more spesifik)
///   "cat file.log | grep error" → profile from "grep" (spesifik, not cat)
///   "cd /project && ls -la"     → profile from "ls" (action command)
pub fn resolve_profile_for_chain(command: &str) -> ToolProfile {
    // Split on shell operators
    let segments: Vec<&str> = command
        .split(['|', '&', ';'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "&" && *s != "|")
        .collect();

    if segments.is_empty() {
        return ToolProfile::default();
    }

    // Score tiap segment — pilih yang paling spesifik
    let scored: Vec<(usize, &str, u8)> = segments
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            let base = seg
                .split_whitespace()
                .next()
                .map(|w| w.trim_matches(|c| c == '"' || c == '\''))
                .and_then(|w| std::path::Path::new(w).file_name()?.to_str())
                .unwrap_or("");
            let specificity = command_specificity(base, seg);
            (i, *seg, specificity)
        })
        .collect();

    // Pilih command dengan specificity tertinggi (test runner > build > generic)
    let best = scored.iter().max_by_key(|(_, _, score)| score);

    if let Some((_, cmd, _)) = best {
        resolve_profile(cmd)
    } else {
        resolve_profile(segments[0])
    }
}

/// Specificity score — test runner lebih spesifik dari generic shell command
fn command_specificity(base: &str, full_cmd: &str) -> u8 {
    let cmd_lower = full_cmd.to_lowercase();
    // Test runners — paling spesifik
    if matches!(
        base,
        "pytest" | "jest" | "vitest" | "rspec" | "phpunit" | "playwright"
    ) {
        return 100;
    }
    if (base == "cargo" || base == "go" || base == "npm") && cmd_lower.contains("test") {
        return 95;
    }
    // Build tools
    if matches!(base, "cargo" | "make" | "cmake" | "go" | "mvn" | "gradle") {
        return 80;
    }
    // Cloud/infra
    if matches!(base, "docker" | "kubectl" | "terraform" | "helm") {
        return 75;
    }
    // Grep/find (filter commands — biasanya pipe akhir)
    if matches!(base, "grep" | "rg" | "awk" | "sed") {
        return 60;
    }
    // Generic navigation
    if matches!(base, "cd" | "ls" | "cat" | "echo" | "true" | "false") {
        return 10;
    }
    50 // default
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #264. `git status && echo === && find .` was routed to the git distiller,
    /// which emits a fixed one-liner whatever the input, so the 40 lines of
    /// `find` the command was run for were deleted with no marker and the ratio
    /// read as a 99% win.
    #[test]
    fn declines_a_chain_written_to_by_several_commands() {
        assert_eq!(
            sole_output_command("git status && echo '=== tree ===' && find . -type f"),
            None
        );
        assert_eq!(sole_output_command("git log --oneline -12 && ls"), None);
        assert_eq!(sole_output_command("npm run build; npm test"), None);
    }

    #[test]
    fn routes_a_lone_command_to_itself() {
        assert_eq!(sole_output_command("cargo test"), Some("cargo test"));
    }

    /// The whole reason this is not "any chain is a passthrough": `cd` writes
    /// nothing, so the output still came from one program.
    #[test]
    fn looks_past_a_shell_builtin_that_prints_nothing() {
        assert_eq!(
            sole_output_command("cd /project && cargo test"),
            Some("cargo test")
        );
        assert_eq!(
            sole_output_command("export CI=1 && npm test"),
            Some("npm test")
        );
    }

    /// A filter selects rows out of a shape it leaves intact, so the output
    /// still belongs to whatever fed it and routing does not move.
    #[test]
    fn keeps_a_pipeline_intact() {
        assert_eq!(
            sole_output_command("cat app.log | grep ERROR"),
            Some("cat app.log | grep ERROR")
        );
        assert_eq!(
            sole_output_command("kubectl get pods | head -20"),
            Some("kubectl get pods | head -20")
        );
    }

    /// #269. `jq` rewrites the payload rather than selecting from it, so the
    /// output is its own. Routed upstream, `kubectl get pod -o json | jq -r '...'`
    /// reached the cloud distiller, which kept `created:` and dropped the pod
    /// phase, the node and the zone: the three fields the command was run to
    /// check.
    #[test]
    fn hands_a_pipeline_to_the_stage_that_reshaped_it() {
        assert_eq!(
            sole_output_command("kubectl get pod x -o json | jq -r '.status.phase'"),
            Some("jq -r '.status.phase'")
        );
        assert_eq!(
            sole_output_command("kubectl kustomize . | yq '.spec'"),
            Some("yq '.spec'")
        );
    }

    /// A pipe inside a quoted argument is not a pipe.
    #[test]
    fn does_not_split_a_pipe_inside_quotes() {
        assert_eq!(
            sole_output_command("grep -E \"jenkins|atlantis\" values.yaml"),
            Some("grep -E \"jenkins|atlantis\" values.yaml")
        );
    }

    /// Measured on the maintainer's 5,143 distinct recorded commands: without
    /// this, 202 of them stop being distilled because an operator inside a quoted
    /// argument reads as a chain. Splitting too eagerly only costs savings, so
    /// this is worth its lines and not more.
    #[test]
    fn does_not_split_on_an_operator_inside_quotes() {
        assert_eq!(
            sole_output_command("git commit -m \"fix: a && b\""),
            Some("git commit -m \"fix: a && b\"")
        );
        assert_eq!(
            sole_output_command("awk '/^kind:/{f=1} f && /^---/{f=0}' out.yaml"),
            Some("awk '/^kind:/{f=1} f && /^---/{f=0}' out.yaml")
        );
    }

    /// An unbalanced quote leaves the scanner inside a string. One segment out is
    /// the safe answer: it routes as it does today rather than inventing a split.
    #[test]
    fn treats_an_unbalanced_quote_as_one_command() {
        assert_eq!(
            sole_output_command("echo \"oops && ls"),
            Some("echo \"oops && ls")
        );
    }

    #[test]
    fn returns_none_for_an_empty_command() {
        assert_eq!(sole_output_command(""), None);
        assert_eq!(sole_output_command("   "), None);
        assert_eq!(sole_output_command(" && ; "), None);
    }

    #[test]
    fn splits_on_newlines_and_double_pipes_too() {
        assert_eq!(sole_output_command("git status\nls -la"), None);
        assert_eq!(sole_output_command("cargo build || cargo clean"), None);
    }

    #[test]
    fn test_registry_flutter_test_gets_test_profile() {
        let p = resolve_profile("flutter test");
        assert_eq!(p.segmentation, SegmentationMode::TestGroup);
        assert_eq!(p.collapse, CollapseMode::Test);
    }

    #[test]
    fn test_registry_cargo_clippy_gets_build_profile() {
        let p = resolve_profile("cargo clippy -- -D warnings");
        assert_eq!(p.collapse, CollapseMode::Build);
    }

    #[test]
    fn test_registry_psql_gets_log_profile() {
        let p = resolve_profile("psql -U myuser mydb");
        assert_eq!(p.collapse, CollapseMode::Log);
    }

    #[test]
    fn test_registry_nx_test_gets_build_profile() {
        let p = resolve_profile("nx test my-app");
        assert_eq!(p.collapse, CollapseMode::Build);
    }

    #[test]
    fn test_registry_unknown_command_gets_default() {
        let p = resolve_profile("my_custom_script.sh --verbose");
        assert_eq!(p.segmentation, SegmentationMode::Line);
        assert_eq!(p.collapse, CollapseMode::Generic);
    }

    #[test]
    fn test_chain_npm_install_then_test_picks_test() {
        let p = resolve_profile_for_chain("npm install && npm test");
        assert_eq!(p.segmentation, SegmentationMode::TestGroup);
    }

    #[test]
    fn test_chain_cat_pipe_grep_picks_grep() {
        let p = resolve_profile_for_chain("cat app.log | grep ERROR");
        assert_eq!(p.segmentation, SegmentationMode::Line);
        // grep profile
    }

    #[test]
    fn test_chain_cargo_build_then_run_picks_cargo() {
        let p = resolve_profile_for_chain("cargo build && ./target/debug/app");
        assert_eq!(p.collapse, CollapseMode::Build);
    }

    #[test]
    fn test_single_command_unchanged() {
        let p1 = resolve_profile("pytest");
        let p2 = resolve_profile_for_chain("pytest");
        assert_eq!(p1.segmentation, p2.segmentation);
        assert_eq!(p1.collapse, p2.collapse);
    }
}
