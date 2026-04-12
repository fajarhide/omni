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
    if (base == "cargo" || base == "go" || base == "npm" || base == "yarn" || base == "pnpm")
        && (cmd_lower.contains("test") || cmd_lower.contains("check"))
    {
        return ToolProfile {
            segmentation: SegmentationMode::TestGroup,
            collapse: CollapseMode::Test,
        };
    }

    // 3. Build Tools — Build collapse
    if matches!(
        base,
        "cargo"
            | "rustc"
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

    ToolProfile::default()
}
