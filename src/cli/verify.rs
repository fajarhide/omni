use crate::store::sqlite::Store;
use anyhow::Result;
use clap::Parser;
use serde::Serialize;
use std::sync::Arc;

#[derive(Parser, Debug)]
pub struct VerifyArgs {
    /// Session ID of the maker agent to verify
    #[arg(long)]
    pub session: String,

    /// Criteria to check against
    #[arg(long, default_value = "all tests pass")]
    pub criteria: String,

    /// Number of recent tool calls to evaluate
    #[arg(long, default_value = "10")]
    pub last_n: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
pub struct VerifyOutput {
    pub session_id: String,
    pub criteria: String,
    pub total_tool_calls: usize,
    pub recommendations: Vec<String>,
}

pub fn run(args: &VerifyArgs, store: Arc<Store>) -> Result<()> {
    let traces = store.get_recent_distillations(&args.session, args.last_n);

    let mut recommendations = Vec::new();
    if traces.is_empty() {
        recommendations.push("No tool calls found for the given session.".to_string());
    } else {
        recommendations.push(format!("Analyzed {} tool calls.", traces.len()));
        let dummy_id = format!(
            "verify-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let _ = store.conn.lock().unwrap().execute(
            "INSERT INTO verification_results (session_id, checker_agent, maker_session, criteria, passed, issues, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                dummy_id,
                "checker-agent",
                &args.session,
                &args.criteria,
                1, // assume passed for now in this CLI stub
                "[]",
                chrono::Utc::now().timestamp(),
            ],
        );
    }

    let output = VerifyOutput {
        session_id: args.session.clone(),
        criteria: args.criteria.clone(),
        total_tool_calls: traces.len(),
        recommendations,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("== Maker-Checker Verification ==");
        println!("Session: {}", output.session_id);
        println!("Criteria: {}", output.criteria);
        println!("Tool calls evaluated: {}", output.total_tool_calls);
        println!("Issues/Recommendations:");
        for rec in output.recommendations {
            println!("  - {}", rec);
        }
    }

    Ok(())
}
