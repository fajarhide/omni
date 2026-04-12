use crate::pipeline::{ContentType, OutputSegment};

pub mod build;
pub mod cloud;
pub mod generic;
pub mod git;
pub mod infra;
pub mod jsts;
pub mod log;
pub mod system_ops;
pub mod tabular;
pub mod test;

pub trait Distiller: Send + Sync {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        session: Option<&crate::pipeline::SessionState>,
    ) -> String;
}

/// Dispatch distiller by command string, fallback to ContentType.
/// Ini adalah preferred API untuk post_tool.rs dan pipe.rs.
/// Command-first → lebih akurat karena ground truth.
pub fn distill_with_command(
    segments: &[crate::pipeline::OutputSegment],
    input: &str,
    command: &str,
    content_type: &ContentType,
    session: Option<&crate::pipeline::SessionState>,
) -> String {
    // Phase 1: Try exact command prefix match
    let base = {
        let first = command.split_whitespace().next().unwrap_or("");
        std::path::Path::new(first)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(first)
    };
    let cmd_lower = command.to_lowercase();

    // Git subcommand routing (paling granular — git punya 3 distiller targets)
    if base == "git" {
        return git::GitDistiller.distill(segments, input, session);
    }

    // Build tools → BuildDistiller
    if matches!(
        base,
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
    ) {
        // Tapi test → TestDistiller
        if cmd_lower.contains("test")
            || cmd_lower.contains("pytest")
            || matches!(base, "pytest" | "rspec" | "phpunit")
        {
            return test::TestDistiller.distill(segments, input, session);
        }
        return build::BuildDistiller.distill(segments, input, session);
    }

    // JS/TS ecosystem → JsTsDistiller
    if matches!(
        base,
        "vitest" | "playwright" | "tsc" | "eslint" | "prettier" | "jest" | "esbuild" | "vite"
    ) {
        return jsts::JsTsDistiller.distill(segments, input, session);
    }
    // npm/pnpm/yarn/bun: check subcommand
    if matches!(base, "npm" | "npx" | "pnpm" | "yarn" | "bun") {
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
        base,
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

    // System ops → SystemOpsDistiller
    if matches!(
        base,
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

    // Phase 2: Fallback to ContentType-based dispatch (existing behavior)
    get_distiller(content_type).distill(segments, input, session)
}

pub fn get_distiller(content_type: &ContentType) -> Box<dyn Distiller> {
    match content_type {
        ContentType::GitDiff | ContentType::GitStatus | ContentType::GitLog => {
            Box::new(git::GitDistiller)
        }
        ContentType::BuildOutput => Box::new(build::BuildDistiller),
        ContentType::TestOutput => Box::new(test::TestDistiller),
        ContentType::InfraOutput => Box::new(infra::InfraDistiller),
        ContentType::LogOutput => Box::new(log::LogDistiller),
        ContentType::TabularData => Box::new(tabular::TabularDistiller),
        ContentType::Cloud => Box::new(cloud::CloudDistiller),
        ContentType::SystemOps => Box::new(system_ops::SystemOpsDistiller),
        ContentType::JsTs => Box::new(jsts::JsTsDistiller),
        ContentType::StructuredData | ContentType::Unknown => Box::new(generic::GenericDistiller),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::scorer;

    macro_rules! snapshot_test {
        ($name:ident, $file:expr, $ctype:expr) => {
            #[test]
            fn $name() {
                let input = include_str!(concat!("../../tests/fixtures/", $file));
                let dummy_cmd = match $ctype {
                    ContentType::GitDiff => "git diff",
                    ContentType::GitStatus => "git status",
                    ContentType::GitLog => "git log",
                    ContentType::BuildOutput => "cargo build",
                    ContentType::TestOutput => "cargo test",
                    ContentType::InfraOutput => "docker build",
                    ContentType::Cloud => "kubectl get pods",
                    ContentType::SystemOps => "ls",
                    ContentType::JsTs => "vitest",
                    ContentType::LogOutput => "cat",
                    ContentType::TabularData => "cat",
                    _ => "",
                };
                let (segments, _) = scorer::score_with_command(input, dummy_cmd, None);
                let distiller = get_distiller(&$ctype);
                let output = distiller.distill(&segments, input, None);

                insta::assert_snapshot!(output);

                if $ctype == ContentType::GitDiff {
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
        ContentType::GitDiff
    );
    snapshot_test!(
        test_git_status_distillation,
        "git_status_dirty.txt",
        ContentType::GitStatus
    );
    snapshot_test!(
        test_cargo_build_distillation,
        "cargo_build_errors.txt",
        ContentType::BuildOutput
    );
    snapshot_test!(
        test_pytest_distillation,
        "pytest_failures.txt",
        ContentType::TestOutput
    );
    snapshot_test!(
        test_kubectl_distillation,
        "kubectl_pods_mixed.txt",
        ContentType::InfraOutput
    );
    snapshot_test!(
        test_docker_build_distillation,
        "docker_build_layered.txt",
        ContentType::InfraOutput
    );
    snapshot_test!(
        test_nginx_log_distillation,
        "nginx_access_log.txt",
        ContentType::LogOutput
    );
    snapshot_test!(
        test_cloud_kubectl,
        "kubectl_get_pods_mixed.txt",
        ContentType::Cloud
    );
    snapshot_test!(
        test_cloud_docker_ps,
        "docker_ps_mixed.txt",
        ContentType::Cloud
    );
    snapshot_test!(
        test_cloud_docker_build_error,
        "docker_build_error.txt",
        ContentType::Cloud
    );
    snapshot_test!(
        test_cloud_terraform_plan,
        "terraform_plan_cloud.txt",
        ContentType::Cloud
    );
    snapshot_test!(
        test_systemops_grep,
        "grep_recursive_output.txt",
        ContentType::SystemOps
    );
    snapshot_test!(
        test_systemops_ls,
        "ls_la_output.txt",
        ContentType::SystemOps
    );
    snapshot_test!(
        test_systemops_find,
        "find_project_output.txt",
        ContentType::SystemOps
    );
    snapshot_test!(test_systemops_env, "env_output.txt", ContentType::SystemOps);

    snapshot_test!(test_jsts_vitest, "vitest_mixed.txt", ContentType::JsTs);
    snapshot_test!(test_jsts_tsc, "tsc_errors.txt", ContentType::JsTs);
    snapshot_test!(
        test_jsts_playwright,
        "playwright_fail.txt",
        ContentType::JsTs
    );
    snapshot_test!(test_jsts_eslint, "eslint_errors.txt", ContentType::JsTs);
}
