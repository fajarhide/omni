use crate::distillers::Distiller;
use crate::pipeline::{ContentType, OutputSegment};

pub struct InfraDistiller;

impl Distiller for InfraDistiller {
    fn content_type(&self) -> ContentType {
        ContentType::InfraOutput
    }

    fn distill(&self, segments: &[OutputSegment], input: &str) -> String {
        if input.contains("kubectl")
            || (input.contains("READY") && input.contains("STATUS") && input.contains("RESTARTS"))
        {
            distill_kubectl(input)
        } else if input.contains("docker build")
            || (input.contains("Step ") && input.contains(" : "))
        {
            distill_docker(input)
        } else if input.contains("Terraform will perform") {
            distill_terraform(input)
        } else {
            let mut out = String::new();
            for seg in segments.iter().take(20) {
                out.push_str(&seg.content);
                out.push('\n');
            }
            if segments.len() > 20 {
                out.push_str(&format!(
                    "... {} more infra log lines\n",
                    segments.len() - 20
                ));
            }
            out.trim().to_string()
        }
    }
}

fn distill_kubectl(input: &str) -> String {
    let mut running = 0;
    let mut pending = 0;
    let mut failed = 0;
    let mut non_running_pods = Vec::new();
    let mut total = 0;

    for line in input.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        total += 1;
        let p = line.split_whitespace().collect::<Vec<_>>();
        if p.len() >= 3 {
            let status = p[2];
            if status == "Running" || status == "Completed" {
                running += 1;
            } else if status == "Pending"
                || status == "ContainerCreating"
                || status.contains("Wait")
            {
                pending += 1;
                non_running_pods.push(format!("{} ({})", p[0], status));
            } else {
                failed += 1;
                non_running_pods.push(format!("{} ({})", p[0], status));
            }
        }
    }

    let mut out = format!(
        "k8s: {} pods | {} running, {} pending, {} failed",
        total, running, pending, failed
    );

    if !non_running_pods.is_empty() {
        out.push_str("\nNon-running: ");
        out.push_str(&non_running_pods.join(", "));
    }

    out
}

fn distill_docker(input: &str) -> String {
    let mut steps_total = 0;
    let mut cached = 0;
    let mut result = "built".to_string();

    for line in input.lines() {
        if line.starts_with("Step ") {
            steps_total += 1;
        } else if line.contains("Using cache") {
            cached += 1;
        } else if line.contains("Successfully built") {
            result = line.to_string();
        } else if line.contains("failed to build") || line.contains("Error ") {
            result = "failed".to_string();
        }
    }

    format!(
        "docker: {} steps | {} cached | {}",
        steps_total, cached, result
    )
}

fn distill_terraform(input: &str) -> String {
    let mut added = 0;
    let mut changed = 0;
    let mut destroyed = 0;
    let mut resources = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('+') && trimmed.contains("resource") {
            added += 1;
            resources.push(trimmed.to_string());
        }
        if trimmed.starts_with('~') && trimmed.contains("resource") {
            changed += 1;
            resources.push(trimmed.to_string());
        }
        if trimmed.starts_with('-') && trimmed.contains("resource") {
            destroyed += 1;
            resources.push(trimmed.to_string());
        }
    }

    let mut out = format!(
        "terraform: {} to add, {} to change, {} to destroy\n",
        added, changed, destroyed
    );
    let max_res = 5;
    for (i, res) in resources.iter().enumerate() {
        if i < max_res {
            out.push_str(res);
            out.push('\n');
        } else {
            out.push_str(&format!(
                "... {} more resources\n",
                resources.len() - max_res
            ));
            break;
        }
    }
    out.trim().to_string()
}
