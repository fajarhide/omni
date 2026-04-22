use crate::pipeline::analyzer::{self, DiagnosisCategory};
use crate::store::sqlite::Store;
use colored::*;
use serde_json::json;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

pub fn run_optimize(args: &[String]) -> anyhow::Result<()> {
    let mut limit = 50;
    for i in 0..args.len() {
        if args[i] == "--limit" && i + 1 < args.len() {
            limit = args[i + 1].parse().unwrap_or(limit);
        }
        if let Some(val) = args[i].strip_prefix("--limit=") {
            limit = val.parse().unwrap_or(limit);
        }
    }

    println!("{} Loading SQLite trace store...", "omni".cyan().bold());
    let store = Store::open()?;
    let traces = store.get_recent_traces(limit)?;

    if traces.is_empty() {
        println!("  No recent traces to analyze. Use 'omni <command>' first.");
        return Ok(());
    }

    let mut suboptimal_traces = Vec::new();
    println!(
        "{} Analyzing {} recent execution traces using Quality Signal Engine...",
        "omni".cyan().bold(),
        traces.len()
    );

    for (_session_id, command, raw, distilled) in traces {
        let diagnosis = analyzer::analyze_trace(&raw, &distilled, &command);
        if diagnosis.category == DiagnosisCategory::Suboptimal
            || diagnosis.category == DiagnosisCategory::FailedSignalDropped
        {
            suboptimal_traces.push((command, raw, distilled, diagnosis));
        }
    }

    if suboptimal_traces.is_empty() {
        println!(
            "  {} No suboptimal or failed traces found. Distillation is performing perfectly!",
            "✓".green()
        );
        return Ok(());
    }

    println!(
        "  {} {} suboptimal/failed traces diagnosed.",
        "⚠".yellow(),
        suboptimal_traces.len()
    );

    let api_key = match env::var("ANTHROPIC_API_KEY") {
        Ok(k) => k,
        Err(_) => {
            println!(
                "  {} ANTHROPIC_API_KEY environment variable is required for self-optimization.",
                "✗".red()
            );
            println!("     Export it in your shell to run the AI tuning loop.");
            std::process::exit(1);
        }
    };

    println!(
        "{} Preparing prompt for AI tuning (Anthropic Claude 3.5 Sonnet)...",
        "omni".cyan().bold()
    );

    // Take the worst performing trace
    let (cmd, mut raw, distilled, diag) = suboptimal_traces.remove(0);

    // Safety truncation
    if raw.len() > 10000 {
        raw.truncate(10000);
        raw.push_str("\n...[truncated due to LLM context limits]");
    }

    let prompt = format!(
        "You are the Omni self-optimizing engine. Analyze the following failed/suboptimal command execution trace and output a TOML filter (using [[filters]]) that properly extracts the meaningful signal (errors, completion states) while dropping repetitive noise loops.\n\nCommand: {}\n\nRaw Input:\n```\n{}\n```\n\nDistilled Output:\n```\n{}\n```\n\nDiagnostic Feedback:\n{}\n\nOutput ONLY valid format TOML code within a markdown codeblock, nothing else.",
        cmd,
        raw,
        distilled,
        diag.feedback_notes.join("\n")
    );

    println!("  {} Connecting to Anthropic API...", "▶".blue());

    let payload = json!({
        "model": "claude-3-5-sonnet-20241022",
        "max_tokens": 1500,
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ]
    });

    let res_result = ureq::post("https://api.anthropic.com/v1/messages")
        .set("x-api-key", &api_key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .send_json(payload);

    let response = match res_result {
        Ok(res) => res,
        Err(e) => {
            if let ureq::Error::Status(code, res) = e {
                let err_str = res.into_string().unwrap_or_default();
                eprintln!("\n{} API Error {}: {}", "✗".red(), code, err_str);
            } else {
                eprintln!("\n{} Request Error: {}", "✗".red(), e);
            }
            std::process::exit(1);
        }
    };

    let body_json: serde_json::Value = response.into_json()?;
    let llm_reply = body_json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let extracted_toml = extract_toml(&llm_reply);

    if extracted_toml.is_empty() {
        println!(
            "  {} The LLM did not return a valid TOML configuration.",
            "✗".red()
        );
        println!("  Raw LLM Output:\n{}", llm_reply);
        return Ok(());
    }

    // Append to learned.toml
    let toml_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".omni")
        .join("filters")
        .join("learned.toml");

    if let Some(p) = toml_path.parent() {
        std::fs::create_dir_all(p)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&toml_path)?;

    file.write_all(b"\n")?;
    file.write_all(extracted_toml.as_bytes())?;

    println!(
        "{} Successfully learned and applied new filter from trace!",
        "✨".green()
    );
    println!("   Saved to: {:?}", toml_path);
    println!("   Preview:\n{}", extracted_toml.bright_black());

    Ok(())
}

fn extract_toml(text: &str) -> String {
    let mut toml_lines = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        if line.starts_with("```toml") || line.starts_with("```") {
            if in_block {
                break;
            } else {
                in_block = true;
                continue;
            }
        }
        if in_block {
            toml_lines.push(line);
        }
    }

    if toml_lines.is_empty() {
        if text.contains("[[filters]]") || text.contains("[filters.") {
            return text.trim().to_string();
        }
        return String::new();
    }

    toml_lines.join("\n").trim().to_string()
}
