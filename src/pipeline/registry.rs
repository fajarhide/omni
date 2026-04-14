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

pub fn resolve_profile(command: &str) -> ToolProfile {
    if command.is_empty() {
        return ToolProfile::default();
    }

    let cmd = command.trim();
    let base = {
        let first_word = cmd.split_whitespace().next().unwrap_or(cmd);
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
            "diff" | "show" | "whatchanged" => {
                if !cmd_lower.contains("--stat") {
                    return ToolProfile {
                        segmentation: SegmentationMode::GitHunk,
                        collapse: CollapseMode::Generic,
                    };
                }
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
