use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};

pub struct CloudDistiller;

impl Distiller for CloudDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> String {
        // Dispatch to sub-function based on content analysis
        if input.contains("CONTAINER ID") || input.contains("docker ps") {
            distill_docker_ps(input)
        } else if input.contains("Step ") && input.contains(" : ") {
            distill_docker_build(input)
        } else if input.contains("docker logs") || is_docker_logs(input) {
            distill_docker_logs(segments, input)
        } else if is_kubectl_table(input) {
            distill_kubectl(input)
        } else if input.contains("kubectl") {
            distill_kubectl_generic(segments, input)
        } else if input.contains("terraform") || input.contains("Terraform") {
            distill_terraform(input)
        } else if input.contains("helm") || is_helm_table(input) {
            distill_helm(input)
        } else if input.contains("aws ") {
            distill_aws(segments, input)
        } else if is_resource_table(input) {
            // A columnar listing we have no distiller for (`kubectl get pvc|pv|
            // ns|certificates …`). The fallback would keep 20 lines and drop the
            // rest with no marker, so hand back the payload untouched.
            input.to_string()
        } else {
            distill_fallback(segments)
        }
    }
}

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

/// A pod table is fingerprinted by `READY` + `RESTARTS`, which no other
/// `kubectl get` resource prints. `NAMESPACE NAME STATUS` is *not* a
/// fingerprint — it is the common prefix of nearly every `-A` listing (pvc, pv,
/// namespaces, certificates, …), and matching on it routed those into the pod
/// distiller, which then reported healthy objects as errors.
fn is_kubectl_table(input: &str) -> bool {
    input.lines().any(is_pod_header)
}

fn is_pod_header(line: &str) -> bool {
    line.contains("NAME")
        && line.contains("READY")
        && line.contains("STATUS")
        && line.contains("RESTARTS")
}

/// Any tabular listing with a `NAME` column and an otherwise all-caps header.
/// Used only to refuse: we can recognise the shape without knowing the schema.
fn is_resource_table(input: &str) -> bool {
    input.lines().take(5).any(|l| {
        let cols: Vec<&str> = l.split_whitespace().collect();
        cols.len() >= 3
            && cols.contains(&"NAME")
            && cols.iter().all(|c| {
                c.chars()
                    .all(|ch| ch.is_ascii_uppercase() || ch == '-' || ch == '_')
            })
    })
}

fn is_helm_table(input: &str) -> bool {
    input
        .lines()
        .any(|l| l.contains("REVISION") && l.contains("CHART") && l.contains("STATUS"))
}

fn is_docker_logs(input: &str) -> bool {
    // Heuristic: multiple lines with timestamp-like prefix
    let ts_count = input
        .lines()
        .take(20)
        .filter(|l| {
            l.len() > 20
                && (l
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
                    || l.starts_with('['))
        })
        .count();
    ts_count >= 5
}

// ---------------------------------------------------------------------------
// Critical / Noise patterns
// ---------------------------------------------------------------------------

const CRITICAL_PATTERNS: &[&str] = &[
    "error",
    "Error",
    "ERROR",
    "failed",
    "FAILED",
    "CrashLoopBackOff",
    "OOMKilled",
    "ImagePullBackOff",
    "Terminating",
    "Evicted",
    "panic",
    "fatal",
    "FATAL",
    "exception",
    "Exception",
    "BackOff",
];

const NOISE_PATTERNS: &[&str] = &[
    "Pulling from",
    "Pull complete",
    "Extracting",
    "Waiting",
    "Polling",
    ".......",
    "Downloading",
    "Verifying Checksum",
    "Download complete",
    "Already exists",
    "Using cache",
    " --->",
    " ---> ",
];

fn is_critical(line: &str) -> bool {
    CRITICAL_PATTERNS.iter().any(|p| line.contains(p))
}

fn is_noise(line: &str) -> bool {
    NOISE_PATTERNS.iter().any(|p| line.contains(p))
}

// ---------------------------------------------------------------------------
// kubectl table (NAME READY STATUS RESTARTS AGE)
// ---------------------------------------------------------------------------

/// Pod phases/reasons that mean "not healthy yet, but not broken".
const POD_PENDING: &[&str] = &[
    "Pending",
    "ContainerCreating",
    "PodInitializing",
    "Terminating",
];

/// Pod phases/reasons that mean "broken". Anything outside these three lists is
/// counted as `unknown`, never as an error — a status this distiller has never
/// heard of is not evidence of a failure.
const POD_FAILED: &[&str] = &[
    "CrashLoopBackOff",
    "Error",
    "Failed",
    "ImagePullBackOff",
    "ErrImagePull",
    "OOMKilled",
    "Evicted",
    "CreateContainerConfigError",
    "CreateContainerError",
    "InvalidImageName",
    "RunContainerError",
    "NodeAffinity",
    "Unknown",
];

fn distill_kubectl(input: &str) -> String {
    // Resolve NAME/STATUS from the header instead of assuming column 0 and 2:
    // `kubectl get pods -A` prefixes a NAMESPACE column, which shifted every
    // lookup by one and made READY ("1/1") read as the status.
    let Some((header_idx, header)) = input.lines().enumerate().find(|(_, l)| is_pod_header(l))
    else {
        return input.to_string();
    };
    let cols: Vec<&str> = header.split_whitespace().collect();
    let (Some(name_idx), Some(status_idx)) = (
        cols.iter().position(|c| *c == "NAME"),
        cols.iter().position(|c| *c == "STATUS"),
    ) else {
        return input.to_string();
    };

    let mut running = 0u32;
    let mut pending = 0u32;
    let mut failed = 0u32;
    let mut unknown = 0u32;
    let mut total = 0u32;
    let mut problems: Vec<String> = Vec::new();

    for line in input.lines().skip(header_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        // A row that does not reach the STATUS column is not a row we can read.
        if parts.len() <= status_idx {
            continue;
        }
        total += 1;
        let name = parts[name_idx];
        let status = parts[status_idx];
        if matches!(status, "Running" | "Completed" | "Succeeded") {
            running += 1;
        } else if POD_PENDING.contains(&status) || status.starts_with("Init:") {
            pending += 1;
            problems.push(format!("{} ({})", name, status));
        } else if POD_FAILED.contains(&status) {
            failed += 1;
            problems.push(format!("{} ({})", name, status));
        } else {
            unknown += 1;
            problems.push(format!("{} ({})", name, status));
        }
    }

    // Nothing parsed means the fingerprint matched something that only looks
    // like a pod table. Hand back the payload rather than summarise a guess.
    if total == 0 {
        return input.to_string();
    }

    let mut out = format!(
        "k8s: {} pods | {} running, {} pending, {} error",
        total, running, pending, failed
    );
    if unknown > 0 {
        out.push_str(&format!(", {} unknown", unknown));
    }

    if !problems.is_empty() {
        out.push_str("\nProblems: ");
        let shown: Vec<&str> = problems.iter().take(5).map(|s| s.as_str()).collect();
        out.push_str(&shown.join(", "));
        if problems.len() > 5 {
            out.push_str(&format!(" +{} more", problems.len() - 5));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// kubectl generic (describe, logs, apply, etc.)
// ---------------------------------------------------------------------------

fn distill_kubectl_generic(segments: &[OutputSegment], input: &str) -> String {
    // Extract critical lines first
    let mut critical_lines: Vec<&str> = Vec::new();
    let mut important_lines: Vec<&str> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_critical(trimmed) {
            critical_lines.push(trimmed);
        } else if trimmed.contains("configured")
            || trimmed.contains("created")
            || trimmed.contains("unchanged")
            || trimmed.contains("deleted")
        {
            important_lines.push(trimmed);
        }
    }

    let mut out = String::new();
    if !critical_lines.is_empty() {
        for line in critical_lines.iter().take(10) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !important_lines.is_empty() {
        for line in important_lines.iter().take(10) {
            out.push_str(line);
            out.push('\n');
        }
    }

    if out.trim().is_empty() {
        return distill_fallback(segments);
    }

    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// docker ps
// ---------------------------------------------------------------------------

fn distill_docker_ps(input: &str) -> String {
    let mut running = 0u32;
    let mut exited = 0u32;
    let mut other = 0u32;
    let mut total = 0u32;
    let mut problem_containers: Vec<String> = Vec::new();

    for line in input.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        total += 1;

        // docker ps columns: CONTAINER ID | IMAGE | COMMAND | CREATED | STATUS | PORTS | NAMES
        // STATUS is typically at column index 4 (0-based)
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let status_str = trimmed.to_lowercase();

        if status_str.contains("up ") {
            running += 1;
        } else if status_str.contains("exited") {
            exited += 1;
            // Try to get container name (last column)
            if let Some(name) = parts.last() {
                problem_containers.push(name.to_string());
            }
        } else {
            other += 1;
        }
    }

    let mut out = format!(
        "docker: {} containers | {} running, {} exited",
        total, running, exited
    );
    if other > 0 {
        out.push_str(&format!(", {} other", other));
    }
    if !problem_containers.is_empty() {
        out.push_str("\nExited: ");
        let shown: Vec<&str> = problem_containers
            .iter()
            .take(5)
            .map(|s| s.as_str())
            .collect();
        out.push_str(&shown.join(", "));
        if problem_containers.len() > 5 {
            out.push_str(&format!(" +{} more", problem_containers.len() - 5));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// docker build
// ---------------------------------------------------------------------------

fn distill_docker_build(input: &str) -> String {
    let mut steps_total = 0u32;
    let mut cached = 0u32;
    let mut error_step: Option<(u32, String)> = None;
    let mut success = false;

    for line in input.lines() {
        let l_lower = line.to_lowercase();
        if l_lower.contains("step ") {
            steps_total += 1;
        }
        if line.contains("Using cache") {
            cached += 1;
        }
        if line.contains("Successfully built") || line.contains("Successfully tagged") {
            success = true;
        }
        if is_critical(line) && !line.contains("Successfully") {
            error_step = Some((steps_total, line.trim().to_string()));
        }
    }

    if let Some((step, msg)) = error_step {
        format!(
            "docker build: ✗ failed at step {}/{} — {}",
            step, steps_total, msg
        )
    } else if success {
        let cached_info = if cached > 0 {
            format!(", {} cached", cached)
        } else {
            String::new()
        };
        format!(
            "docker build: ✓ complete ({} layers{})",
            steps_total, cached_info
        )
    } else {
        format!("docker build: {} steps, {} cached", steps_total, cached)
    }
}

// ---------------------------------------------------------------------------
// docker logs
// ---------------------------------------------------------------------------

fn distill_docker_logs(_segments: &[OutputSegment], input: &str) -> String {
    let mut critical_lines: Vec<&str> = Vec::new();

    for line in input.lines() {
        if is_critical(line) && !is_noise(line) {
            critical_lines.push(line.trim());
        }
    }

    if critical_lines.is_empty() {
        let total = input.lines().count();
        // Zero-state guard (#143): "no errors detected" is only truthful about
        // real logs. The dispatch reaches here on a weak content sniff too
        // (`input.contains("docker logs")`, #112), so require the log shape —
        // otherwise a misrouted payload (kubectl exec, a manifest) passes through
        // instead of being falsely certified clean.
        return super::require_parsed(
            is_docker_logs(input),
            input,
            format!("docker logs: {} lines, no errors detected", total),
        );
    }

    let mut out = format!(
        "docker logs: {} errors/warnings found\n",
        critical_lines.len()
    );
    for line in critical_lines.iter().take(10) {
        out.push_str(line);
        out.push('\n');
    }
    if critical_lines.len() > 10 {
        out.push_str(&format!("... +{} more\n", critical_lines.len() - 10));
    }

    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// terraform
// ---------------------------------------------------------------------------

fn distill_terraform(input: &str) -> String {
    let mut added = 0u32;
    let mut changed = 0u32;
    let mut destroyed = 0u32;
    let mut resources: Vec<String> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();

        // terraform plan lines like: "# aws_instance.web will be created"
        if trimmed.contains("will be created") {
            added += 1;
            if let Some(res) = extract_tf_resource(trimmed) {
                resources.push(format!("+ {}", res));
            }
        } else if trimmed.contains("will be updated") || trimmed.contains("must be replaced") {
            changed += 1;
            if let Some(res) = extract_tf_resource(trimmed) {
                resources.push(format!("~ {}", res));
            }
        } else if trimmed.contains("will be destroyed") {
            destroyed += 1;
            if let Some(res) = extract_tf_resource(trimmed) {
                resources.push(format!("- {}", res));
            }
        }

        let t_lower = trimmed.to_lowercase();

        // Also catch the summary line: "Plan: X to add, Y to change, Z to destroy."
        if t_lower.contains("plan:") {
            // Parse "Plan: 3 to add, 1 to change, 0 to destroy."
            for part in trimmed.split(',') {
                let part = part.trim();
                if part.contains("to add") {
                    if let Some(n) = part.split_whitespace().find_map(|w| w.parse::<u32>().ok())
                        && added == 0
                    {
                        added = n;
                    }
                } else if part.contains("to change") {
                    if let Some(n) = part.split_whitespace().find_map(|w| w.parse::<u32>().ok())
                        && changed == 0
                    {
                        changed = n;
                    }
                } else if part.contains("to destroy")
                    && let Some(n) = part.split_whitespace().find_map(|w| w.parse::<u32>().ok())
                    && destroyed == 0
                {
                    destroyed = n;
                }
            }
        }

        // "Apply complete! Resources: X added, Y changed, Z destroyed."
        if t_lower.contains("apply complete!") {
            return format!(
                "terraform: apply complete +{} ~{} -{}",
                added, changed, destroyed
            );
        }
    }

    let mut out = format!(
        "terraform: +{} ~{} -{} resources",
        added, changed, destroyed
    );

    if !resources.is_empty() {
        out.push('\n');
        for res in resources.iter().take(5) {
            out.push_str(res);
            out.push('\n');
        }
        if resources.len() > 5 {
            out.push_str(&format!("... +{} more\n", resources.len() - 5));
        }
    }

    out.trim().to_string()
}

fn extract_tf_resource(line: &str) -> Option<String> {
    // "# aws_instance.web will be created" -> "aws_instance.web"
    let trimmed = line.trim().trim_start_matches('#').trim();
    trimmed.split_whitespace().next().map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// helm
// ---------------------------------------------------------------------------

fn distill_helm(input: &str) -> String {
    let mut deployed = 0u32;
    let mut failed_h = 0u32;
    let mut pending_h = 0u32;
    let mut other_h = 0u32;
    let mut releases: Vec<String> = Vec::new();

    let has_header = input
        .lines()
        .any(|l| l.contains("NAME") && l.contains("STATUS"));

    if has_header {
        for line in input.lines().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                let name = parts[0];
                // Helm table: NAME NAMESPACE REVISION UPDATED STATUS CHART APP VERSION
                // Find STATUS col — typically index 4
                let status = if parts.len() >= 5 { parts[4] } else { parts[3] };
                match status.to_lowercase().as_str() {
                    "deployed" => deployed += 1,
                    "failed" => {
                        failed_h += 1;
                        releases.push(format!("{} (failed)", name));
                    }
                    "pending-install" | "pending-upgrade" => {
                        pending_h += 1;
                        releases.push(format!("{} ({})", name, status));
                    }
                    _ => other_h += 1,
                }
            }
        }
    }

    let mut out = format!(
        "helm: {} deployed, {} failed, {} pending",
        deployed, failed_h, pending_h
    );
    if other_h > 0 {
        out.push_str(&format!(", {} other", other_h));
    }
    if !releases.is_empty() {
        out.push_str("\nIssues: ");
        out.push_str(&releases.join(", "));
    }

    out
}

// ---------------------------------------------------------------------------
// aws
// ---------------------------------------------------------------------------

fn distill_aws(segments: &[OutputSegment], input: &str) -> String {
    let mut critical_lines: Vec<&str> = Vec::new();
    let mut result_lines: Vec<&str> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_critical(trimmed) {
            critical_lines.push(trimmed);
        } else if !is_noise(trimmed)
            && (trimmed.contains("arn:")
                || trimmed.contains("i-")
                || trimmed.contains("sg-")
                || trimmed.contains("vpc-")
                || trimmed.contains("subnet-")
                || trimmed.contains("Successfully")
                || trimmed.contains("completed"))
        {
            result_lines.push(trimmed);
        }
    }

    let mut out = String::new();

    if !critical_lines.is_empty() {
        for line in critical_lines.iter().take(5) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !result_lines.is_empty() {
        for line in result_lines.iter().take(10) {
            out.push_str(line);
            out.push('\n');
        }
    }

    if out.trim().is_empty() {
        return distill_fallback(segments);
    }

    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Fallback: take Critical + Important segments, max 20 lines
// ---------------------------------------------------------------------------

fn distill_fallback(segments: &[OutputSegment]) -> String {
    let mut out = String::new();
    let mut lines = 0;

    for seg in segments {
        if matches!(seg.tier, SignalTier::Critical | SignalTier::Important) {
            for line in seg.content.lines() {
                if lines >= 20 {
                    break;
                }
                out.push_str(line);
                out.push('\n');
                lines += 1;
            }
        }
        if lines >= 20 {
            break;
        }
    }

    if out.trim().is_empty() {
        // Absolute fallback: first 10 lines
        for seg in segments.iter().take(10) {
            out.push_str(&seg.content);
            out.push('\n');
        }
    }

    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// From issue #105: `kubectl get pvc -A` matched the pod fingerprint and
    /// every healthy `Bound` claim was reported as an error.
    const PVC_TABLE: &str = "\
NAMESPACE    NAME                      STATUS   VOLUME         CAPACITY   ACCESS MODES   STORAGECLASS   AGE
datasource   qdrant-storage-qdrant-0   Bound    pvc-0a75d858   5Gi        RWO            default        2y
consul       data-consul-0             Bound    pvc-1b86e969   10Gi       RWO            default        2y";

    const PODS_ALL_NAMESPACES: &str = "\
NAMESPACE   NAME                     READY   STATUS             RESTARTS   AGE
default     api-7fb96c846b-f4jnm     1/1     Running            0          5d
kube-系统   auth-5d4f6c8b99-abc12    0/1     CrashLoopBackOff   15         2h";

    fn distill(input: &str) -> String {
        CloudDistiller.distill(&[], input, None)
    }

    #[test]
    fn passes_non_pod_resource_tables_through_untouched() {
        assert_eq!(distill(PVC_TABLE), PVC_TABLE);
    }

    #[test]
    fn does_not_fingerprint_namespace_name_status_as_pods() {
        assert!(!is_kubectl_table(PVC_TABLE));
    }

    #[test]
    fn resolves_columns_from_header_when_namespace_is_present() {
        let out = distill(PODS_ALL_NAMESPACES);
        assert!(
            out.starts_with("k8s: 2 pods | 1 running, 0 pending, 1 error"),
            "{out}"
        );
        // The name column, not the namespace column.
        assert!(
            out.contains("auth-5d4f6c8b99-abc12 (CrashLoopBackOff)"),
            "{out}"
        );
    }

    #[test]
    fn counts_unrecognised_status_as_unknown_not_error() {
        let input = "\
NAME    READY   STATUS        RESTARTS   AGE
web-0   1/1     SomeNewPhase  0          5d";
        let out = distill(input);
        assert!(out.contains("0 error"), "{out}");
        assert!(out.contains("1 unknown"), "{out}");
    }

    #[test]
    fn returns_input_when_pod_table_has_no_rows() {
        let input = "NAME   READY   STATUS   RESTARTS   AGE\n";
        assert_eq!(distill(input), input);
    }
}
