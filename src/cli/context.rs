//! `omni context <file>`: which files this one imports, and which import it.
//!
//! Was an MCP tool and nothing else. It sat in the prefix of every request on a
//! Full-tier host at 189 bytes, and `mcp::policy::FULL` admits a tool on one
//! stated rule, "called at least once in the corpus": across 253 recorded
//! sessions `omni_context` is the only advertised tool with **zero** calls ever,
//! and the three places OMNI recommended it appear zero times in 10,578 traces
//! (#609).
//!
//! Moving it here rather than deleting it keeps the capability and stops charging
//! for it. A CLI subcommand costs nothing per request, and an agent that wants it
//! runs it in the shell OMNI already hooks. `omni_run` was the door named here
//! until it was priced off the Full tier too (#609); it still is on a
//! Handoff-first host, which is the tier that has no shell of its own.

use crate::graph;
use crate::pipeline::SessionState;
use anyhow::Result;
use colored::*;

/// Read by both `print_help` and `super::check_flags` (#151).
const FLAGS: super::Flags = &[];

pub fn run_context(args: &[String], session: Option<&SessionState>) -> Result<()> {
    if super::wants_help(args) {
        print_help();
        return Ok(());
    }
    super::check_flags("context", args, FLAGS)?;

    let Some(file_path) = args.iter().skip(2).find(|a| !a.starts_with('-')) else {
        print_help();
        return Ok(());
    };

    let cwd = std::env::current_dir()?;
    println!("{}", report(&cwd, file_path, session)?);
    Ok(())
}

/// The report itself, so the CLI and any future caller cannot render it two ways.
///
/// The session is read here rather than by the caller because the hot-file lookup
/// has to use the path the graph resolved, not the one that was typed. `omni
/// context ./src/main.rs` and `omni context src/main.rs` name the same file, and
/// looking up the raw argument answers "Hot in session: no" for one of them.
pub fn report(
    cwd: &std::path::Path,
    file_path: &str,
    session: Option<&SessionState>,
) -> Result<String> {
    let graph = graph::indexer::build_graph(cwd)?;
    let ctx = graph.context_for(file_path);
    let hot_count = session
        .and_then(|s| s.hot_files.get(&ctx.file_path).copied())
        .unwrap_or(0);

    let list = |items: &[String]| {
        if items.is_empty() {
            "none detected".to_string()
        } else {
            items.iter().take(8).cloned().collect::<Vec<_>>().join(", ")
        }
    };

    Ok(format!(
        "OMNI Context for {}\nImports: {}\nImported by: {}\nHot in session: {}\n",
        ctx.file_path,
        list(&ctx.imports),
        list(&ctx.imported_by),
        if hot_count > 0 {
            format!("yes ({hot_count}x)")
        } else {
            "no".to_string()
        }
    ))
}

fn print_help() {
    println!(
        "\n{} {}: Dependency context for one file",
        "omni".bold().cyan(),
        "context".bold().yellow()
    );
    println!(
        "Which files it imports, which import it, and whether this session keeps touching it."
    );
    println!();
    println!("Usage: omni context <file>");
    println!();
    super::print_flags(FLAGS);
    println!();
}

#[cfg(test)]
mod tests {
    use super::report;
    use crate::pipeline::SessionState;

    /// Greptile on #609. The graph resolves `./src/x.rs` and `src/x.rs` to one
    /// path; looking the session up by the argument as typed answers "Hot in
    /// session: no" for one spelling of the same file.
    #[test]
    fn the_hot_lookup_uses_the_path_the_graph_resolved() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).expect("mkdir");
        std::fs::write(src.join("thing.rs"), "// nothing to import\n").expect("write");

        let mut session = SessionState::new();
        // Recorded the way the tracker records it, resolved rather than as typed.
        let resolved = report(dir.path(), "src/thing.rs", None).expect("report");
        let name = resolved
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("OMNI Context for "))
            .expect("the first line names the file")
            .to_string();
        session.hot_files.insert(name, 4);

        for spelling in ["src/thing.rs", "./src/thing.rs"] {
            let out = report(dir.path(), spelling, Some(&session)).expect("report");
            assert!(
                out.contains("Hot in session: yes (4x)"),
                "{spelling} reported the wrong hot status:\n{out}"
            );
        }
    }
}
