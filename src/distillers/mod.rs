use crate::pipeline::OutputSegment;

pub mod build;
pub mod cloud;
pub mod generic;
pub mod git;
pub mod jsts;
pub mod system_ops;
pub mod test;

pub trait Distiller: Send + Sync {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        session: Option<&crate::pipeline::SessionState>,
    ) -> String;
}

/// Distill output based on command
pub fn distill_with_command(
    segments: &[crate::pipeline::OutputSegment],
    input: &str,
    command: &str,
    session: Option<&crate::pipeline::SessionState>,
) -> String {
    // 1. Resolve pipeline profile (though we match command here too)
    let _profile = crate::pipeline::registry::resolve_profile(command);

    // Phase 1: Try exact command prefix match
    let base = {
        let first = command.split_whitespace().next().unwrap_or("");
        std::path::Path::new(first)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(first)
    };
    let cmd_lower = command.to_lowercase();

    // Git subcommand routing
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
            | "pytest"
            | "python"
            | "python3"
            | "rspec"
            | "phpunit"
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

    // Phase 2: Fallback to generic distiller
    generic::GenericDistiller.distill(segments, input, session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::scorer;

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
    snapshot_test!(
        test_jsts_playwright,
        "playwright_fail.txt",
        "playwright test"
    );
    snapshot_test!(test_jsts_eslint, "eslint_errors.txt", "eslint");
}
