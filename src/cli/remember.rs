use crate::store::sqlite::Store;
use anyhow::Result;
use clap::Parser;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    name = "remember",
    about = "Store important knowledge to OMNI's persistent memory"
)]
pub struct RememberArgs {
    /// What do you want to remember
    pub content: String,

    /// Category: decision, pattern, gotcha, fact (default: fact)
    #[arg(short, long, default_value = "fact")]
    pub category: String,

    /// Tags opsional, comma-separated
    #[arg(short, long)]
    pub tags: Option<String>,

    /// Scope to current project only (default: global)
    #[arg(long, default_value_t = true)]
    pub project_scoped: bool,
}

pub fn run(args: &[String], store: Arc<Store>) -> Result<()> {
    let mut clap_args = vec!["omni-remember".to_string()];
    clap_args.extend_from_slice(args);
    let parsed = RememberArgs::parse_from(clap_args);

    if parsed.content.trim().len() < 10 {
        anyhow::bail!("Memory entry more short (min 10 character) — write more specific");
    }
    if parsed.content.len() > 2000 {
        anyhow::bail!("Memory entry more long (max 2000 character) — write more specific");
    }
    let valid_categories = ["decision", "pattern", "gotcha", "fact"];
    if !valid_categories.contains(&parsed.category.as_str()) {
        anyhow::bail!("Not valid categories. Try: decision, pattern, gotcha, fact");
    }

    let project_path = if parsed.project_scoped {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "global".to_string())
    } else {
        "global".to_string()
    };

    let project_hash = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(project_path.as_bytes());
        let enc = hex::encode(h.finalize());
        crate::util::text::safe_slice(&enc, 16).to_string()
    };

    let tags: Vec<String> = parsed
        .tags
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let prefix_len = 20.min(parsed.content.len());
    let key = format!(
        "[{}] {}",
        parsed.category,
        crate::util::text::safe_slice(&parsed.content, prefix_len)
    );

    store.upsert_project_knowledge(
        &project_hash,
        &key,
        &parsed.content,
        0.9, // high confidence — user explicitly set this
    );

    let display_len = 60.min(parsed.content.len());
    println!(
        "✓ Saved as [{}]: {}",
        parsed.category,
        crate::util::text::safe_slice(&parsed.content, display_len)
    );
    if !tags.is_empty() {
        println!("  Tags: {}", tags.join(", "));
    }
    println!("  Scope: {}", project_path);

    Ok(())
}
