use crate::pipeline::ContentType;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref RE_GIT_LOG_HASH: Regex = Regex::new(r"^[a-f0-9]{7,40} ").unwrap();
    static ref RE_GIT_LOG_COMMIT: Regex = Regex::new(r"commit [a-f0-9]{40}").unwrap();
    static ref RE_LOG_DATE: Regex =
        Regex::new(r"(\d{4}-\d{2}-\d{2}|\d{2}/\d{2}/\d{4}|\d{2}/[a-zA-Z]{3}/\d{4})").unwrap();
    static ref RE_LOG_SEV: Regex =
        Regex::new(r"(?i)\[?(INFO|ERROR|WARN|WARNING|DEBUG|FATAL)\]?[:\s]").unwrap();
    static ref RE_TABULAR_SPACES: Regex = Regex::new(r" {2,}").unwrap();
}

pub fn classify(input: &str) -> ContentType {
    // Stage 1 restrictions: Only check first 50 lines max for speed
    let mut lines_iter = input.lines().take(50).peekable();
    if lines_iter.peek().is_none() {
        return ContentType::Unknown;
    }

    let trimmed = input.trim();
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        return ContentType::StructuredData;
    }

    let lines: Vec<&str> = lines_iter.collect();

    // 1. GitDiff
    if let Some(&first) = lines.first() {
        let first_trimmed = first.trim();
        if !first_trimmed.is_empty() && first_trimmed.starts_with("diff --git") {
            return ContentType::GitDiff;
        }
    }
    let exempt_git_diff = lines.iter().any(|l| {
        l.starts_with("Compiling ")
            || l.starts_with("Finished ")
            || (l.starts_with("running ") && l.contains(" test"))
            || l.contains("test result: ")
    });

    if !exempt_git_diff {
        let has_a = lines.iter().any(|l| l.starts_with("--- a/"));
        let has_b = lines.iter().any(|l| l.starts_with("+++ b/"));
        if has_a && has_b {
            return ContentType::GitDiff;
        }
        if lines.iter().take(10).any(|l| l.starts_with("@@ -")) {
            return ContentType::GitDiff;
        }
    }

    // 2. GitStatus
    let status_kw = [
        "On branch ",
        "HEAD detached",
        "Changes to be committed",
        "Changes not staged",
        "nothing to commit, working tree clean",
    ];
    if lines
        .iter()
        .any(|l| status_kw.iter().any(|k| l.contains(k)))
    {
        return ContentType::GitStatus;
    }

    // 3. GitLog
    let mut hash_lines = 0;
    for l in &lines {
        if RE_GIT_LOG_HASH.is_match(l) {
            hash_lines += 1;
        }
        if RE_GIT_LOG_COMMIT.is_match(l) {
            return ContentType::GitLog;
        }
    }
    if hash_lines >= 5 {
        return ContentType::GitLog;
    }

    // 5. TestOutput (Checked before BuildOutput due to "FAILED" overlap specificity)
    let test_kw = [
        "test result: ",
        "pytest",
        "Test Suites:",
        "Tests:",
        "--- PASS",
        "--- FAIL",
        "FAIL\t",
        "PASS\t",
        "ok  ",
        "✓",
        "✗",
    ];
    let has_pass = lines.iter().any(|l| l.contains("PASSED"));
    let has_fail = lines.iter().any(|l| l.contains("FAILED"));
    if has_pass && has_fail {
        return ContentType::TestOutput;
    }
    if lines
        .iter()
        .any(|l| l.starts_with("running ") && l.contains(" test"))
    {
        return ContentType::TestOutput;
    }
    if lines.iter().any(|l| test_kw.iter().any(|k| l.contains(k))) {
        return ContentType::TestOutput;
    }

    // 4. BuildOutput
    let build_kw = [
        "error[E",
        "error:",
        "Error:",
        "warning[",
        "warning:",
        "Compiling ",
        "Finished ",
        "error TS",
        "npm error",
        "pip install failed",
    ];
    let has_failed = lines.iter().any(|l| l.contains("FAILED"));
    if lines.iter().any(|l| build_kw.iter().any(|k| l.contains(k))) || has_failed {
        return ContentType::BuildOutput;
    }

    // 5.5 Cloud (docker, kubectl, terraform, helm, aws — lebih spesifik dari InfraOutput)
    let cloud_kw = [
        "docker ps",
        "docker build",
        "docker run",
        "docker images",
        "docker logs",
        "kubectl get",
        "kubectl describe",
        "kubectl apply",
        "kubectl delete",
        "kubectl logs",
        "terraform apply",
        "terraform plan",
        "terraform destroy",
        "helm install",
        "helm upgrade",
        "helm list",
        "helm repo",
        "aws s3",
        "aws ec2",
        "aws ecs",
        "aws lambda",
        "aws cloudformation",
        "CrashLoopBackOff",
        "OOMKilled",
        "ImagePullBackOff",
        "Evicted",
        "Terminating",
    ];
    if lines.iter().any(|l| cloud_kw.iter().any(|k| l.contains(k))) {
        return ContentType::Cloud;
    }
    // kubectl table header (NAME READY STATUS RESTARTS)
    if lines.iter().any(|l| {
        (l.contains("READY") && l.contains("STATUS") && l.contains("RESTARTS"))
            || (l.contains("NAMESPACE") && l.contains("NAME") && l.contains("STATUS"))
    }) {
        return ContentType::Cloud;
    }

    // 5.6 JsTs (vitest, tsc, playwright, eslint)
    let jsts_kw = [
        "vitest",
        "VITE v",
        "Test Files",
        "Tests  ",
        "error TS",
        "tsc --",
        "Found errors",
        "playwright",
        "✓ Passed",
        "✗ Failed",
        "eslint",
        "prettier",
        "PASS src/",
        "FAIL src/",
        "● ",
    ];
    if lines.iter().any(|l| jsts_kw.iter().any(|k| l.contains(k))) {
        return ContentType::JsTs;
    }
    // vitest unicode markers
    if lines.iter().any(|l| l.contains("✓") || l.contains("✗"))
        && lines
            .iter()
            .any(|l| l.contains(".test.") || l.contains(".spec."))
    {
        return ContentType::JsTs;
    }

    // 5.7 SystemOps (ls, grep, find, tree, env)
    {
        let first = lines.first().map(|l| l.trim()).unwrap_or("");
        // ls -la output: starts with "total N"
        let is_ls = first.starts_with("total ");
        // grep output: multiple lines "filename:content" pattern
        let grep_pattern_count = lines
            .iter()
            .filter(|l| {
                l.contains(':')
                    && !l.contains("error")
                    && !l.contains("Error")
                    && !l.starts_with("Step ")
                    && !l.starts_with("DEBUG:")
                    && !l.starts_with("INFO:")
            })
            .count();
        let is_grep = grep_pattern_count >= 3;
        // find output: multiple lines starting with ./ or /
        let find_count = lines
            .iter()
            .filter(|l| l.starts_with("./") || (l.starts_with('/') && !l.contains(':')))
            .count();
        let is_find = find_count >= 3;
        // tree output
        let is_tree = lines
            .iter()
            .any(|l| l.contains("├──") || l.contains("└──") || l.contains("directories"));
        // env output
        let env_count = lines
            .iter()
            .filter(|l| {
                l.contains('=') && l.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            })
            .count();
        let is_env = env_count >= 5;

        if is_ls || is_grep || is_find || is_tree || is_env {
            return ContentType::SystemOps;
        }
    }

    // 6. InfraOutput
    let has_kubectl_header = lines
        .iter()
        .any(|l| l.contains("NAME") && l.contains("READY") && l.contains("STATUS"));
    if has_kubectl_header {
        return ContentType::InfraOutput;
    }

    let infra_kw = [
        "aws ",
        "podman ",
        "Terraform will",
        "Terraform has",
        "Terraform plan",
        r#"Step \d+/\d+"#,
        "Successfully built",
    ];
    if lines.iter().any(|l| infra_kw.iter().any(|k| l.contains(k))) {
        return ContentType::InfraOutput;
    }
    // Handle "Step 1/5" docker pattern manually since kw is exact str
    if lines
        .iter()
        .any(|l| l.starts_with("Step ") && l.contains('/'))
    {
        return ContentType::InfraOutput;
    }

    // 7. LogOutput
    if lines.iter().any(|l| RE_LOG_DATE.is_match(l)) {
        return ContentType::LogOutput;
    }
    if lines.iter().any(|l| RE_LOG_SEV.is_match(l)) {
        return ContentType::LogOutput;
    }

    // 8. TabularData
    if lines.len() >= 3 {
        let aligned_lines = lines
            .iter()
            .filter(|l| RE_TABULAR_SPACES.is_match(l))
            .count();
        if aligned_lines >= 2 {
            return ContentType::TabularData;
        }
    }

    ContentType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_classify_git_diff_output() {
        let input = "diff --git a/src/main.rs b/src/main.rs\nindex 123..456 100644\n--- a/src/main.rs\n+++ b/src/main.rs";
        assert_eq!(classify(input), ContentType::GitDiff);

        let patch = "@@ -1,5 +1,6 @@\n fn main() {}";
        assert_eq!(classify(patch), ContentType::GitDiff);
    }

    #[test]
    fn test_classify_git_status_dirty() {
        let input =
            "On branch feature/omni\nChanges not staged for commit:\n  modified:   src/main.rs";
        assert_eq!(classify(input), ContentType::GitStatus);
    }

    #[test]
    fn test_classify_git_log() {
        let input = "commit 3e51feb6f039a4a4ef493ea4a4ef493ea4a4ef49\nAuthor: John\nDate: Mon";
        assert_eq!(classify(input), ContentType::GitLog);

        let short = "3e51feb Fix bug\na4a4ef4 Add feature\n3e51feb Fix bug\na4a4ef4 Add feature\n3e51feb Fix bug";
        assert_eq!(classify(short), ContentType::GitLog);
    }

    #[test]
    fn test_classify_cargo_build_with_errors() {
        let rust_err = "error[E0432]: unresolved import `std::collections`\n  --> src/main.rs:1:5";
        assert_eq!(classify(rust_err), ContentType::BuildOutput);

        let building = "Compiling omni v0.5.0\nFinished `dev` profile";
        assert_eq!(classify(building), ContentType::BuildOutput);
    }

    #[test]
    fn test_classify_pytest_output_with_failures() {
        let py = "================ test session starts ================\npytest 7.0.1\nFAILED tests/test_core.py";
        assert_eq!(classify(py), ContentType::TestOutput);

        let cargo_test = "running 15 tests\ntest foo ... ok\ntest result: ok. 15 passed";
        assert_eq!(classify(cargo_test), ContentType::TestOutput);

        let cargo_test_diff = "running 1 test\ntest my_test ... FAILED\n\nfailures:\n\n---- my_test stdout ----\nthread 'my_test' panicked at ...\n@@ -1,5 +1,6 @@\n+ foo\n- bar";
        assert_eq!(classify(cargo_test_diff), ContentType::TestOutput);
    }

    #[test]
    fn test_classify_go_test_output() {
        let go_test =
            "ok  \tgithub.com/user/pkg1\t0.123s\nFAIL\tgithub.com/user/pkg2\t0.456s\nFAIL";
        assert_eq!(classify(go_test), ContentType::TestOutput);

        let go_fail = "FAIL\tgithub.com/user/pkg\t0.123s";
        assert_eq!(classify(go_fail), ContentType::TestOutput);
    }

    #[test]
    fn test_classify_kubectl_get_pods() {
        let kube = "NAME                               READY   STATUS    RESTARTS   AGE\nnginx-deployment-7fb96c846b-f4jnm   1/1     Running   0          5d";
        assert_eq!(classify(kube), ContentType::Cloud); // Kubectl specific is now Cloud
    }

    #[test]
    fn test_classify_docker_build_output() {
        let docker =
            "Step 1/5 : FROM alpine:latest\n ---> 49f356fa4eb1\nStep 2/5 : RUN apk add curl";
        assert_eq!(classify(docker), ContentType::InfraOutput); // Still infra output right now

        let terra = "Terraform will perform the following actions:";
        assert_eq!(classify(terra), ContentType::InfraOutput);
    }

    #[test]
    fn test_classify_nginx_access_log() {
        let access = "127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] \"GET /apache_pb.gif HTTP/1.0\" 200 2326\n127.0.0.1 - - [10/Oct/2000:14:55:36 -0700]";
        assert_eq!(classify(access), ContentType::LogOutput); // Contains date pattern

        let app_log = "[INFO] Starting application server...\n[ERROR] Failed to bind port 8080";
        assert_eq!(classify(app_log), ContentType::LogOutput);
    }

    #[test]
    fn test_classify_tabular_data() {
        let table = "Header1      Header2      Header3\nRow1Val1     Row1Val2     Row1Val3\nRow2Val1     Row2Val2     Row2Val3";
        assert_eq!(classify(table), ContentType::TabularData);
    }

    #[test]
    fn test_classify_json_output() {
        let obj = "{\n  \"status\": \"ok\",\n  \"data\": []\n}";
        assert_eq!(classify(obj), ContentType::StructuredData);

        let arr = "[\n  1,\n  2,\n  3\n]";
        assert_eq!(classify(arr), ContentType::StructuredData);
    }

    #[test]
    fn test_classify_unknown_random_text() {
        let text = "Did you hear the tragedy of Darth Plagueis The Wise?\nI thought not.\nIt's not a story the Jedi would tell you.";
        assert_eq!(classify(text), ContentType::Unknown);
    }

    #[test]
    fn test_classify_short_output_to_unknown() {
        let short = "foo";
        // It should match Unknown, unless it looks like structured data e.g. "{}".
        assert_eq!(classify(short), ContentType::Unknown);
    }

    #[test]
    fn bench_classify_1kb() {
        let input = "On branch main\nChanges not staged for commit:\n  modified:   src/main.rs\n"
            .repeat(10); // ~900 bytes
        let start = Instant::now();
        let iters = 10_000;
        for _ in 0..iters {
            std::hint::black_box(classify(&input));
        }
        let elapsed_us = start.elapsed().as_micros();
        let per_iter_us = elapsed_us / iters;

        #[cfg(debug_assertions)]
        let target_us = 2500;
        #[cfg(not(debug_assertions))]
        let target_us = 500;

        assert!(
            per_iter_us < target_us,
            "took {}µs per iter, expected < {}µs",
            per_iter_us,
            target_us
        );
    }

    #[test]
    fn bench_classify_10kb() {
        let input = "[INFO] Loading configuration for environment production\n".repeat(200); // 11,200 bytes
        let start = Instant::now();
        let iters = 1_000;
        for _ in 0..iters {
            std::hint::black_box(classify(&input));
        }
        let elapsed_us = start.elapsed().as_micros();
        let per_iter_us = elapsed_us / iters;

        #[cfg(debug_assertions)]
        let target_us = 25000;
        #[cfg(not(debug_assertions))]
        let target_us = 5000;

        assert!(
            per_iter_us < target_us,
            "took {}µs per iter, expected < {}µs",
            per_iter_us,
            target_us
        );
    }
}
