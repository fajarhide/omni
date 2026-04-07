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
    fn content_type(&self) -> ContentType;
    fn distill(&self, segments: &[OutputSegment], input: &str) -> String;
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
                let segments = scorer::score_segments(input, &$ctype, None);
                let distiller = get_distiller(&$ctype);
                let output = distiller.distill(&segments, input);

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
    
    snapshot_test!(
        test_jsts_vitest,
        "vitest_mixed.txt",
        ContentType::JsTs
    );
    snapshot_test!(
        test_jsts_tsc,
        "tsc_errors.txt",
        ContentType::JsTs
    );
    snapshot_test!(
        test_jsts_playwright,
        "playwright_fail.txt",
        ContentType::JsTs
    );
    snapshot_test!(
        test_jsts_eslint,
        "eslint_errors.txt",
        ContentType::JsTs
    );
}
