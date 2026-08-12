//! `omni retrieve <handle>`, the recovery path every marker names.
//!
//! Filed as #452 from a real session: the markers promised `omni_retrieve("…")`,
//! which is an MCP tool, and on a host where that tool is not surfaced the agent
//! was left with a hole in the file it had just read and no way to fill it. The
//! MCP tool works where MCP works; this exists so the promise does not depend on
//! that.
//!
//! It is also the reason the marker text changed. A marker naming a tool call
//! that only some hosts can make is a handle that does not resolve, and #388
//! settled that a handle which does not resolve is the one defect this mechanism
//! cannot have.

use crate::store::sqlite::Store;
use anyhow::Result;
use colored::*;

const FLAGS: super::Flags = &[];

fn print_help() {
    println!(
        "\n{} {}: Print the content a marker archived",
        "omni".bold().cyan(),
        "retrieve".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} {}", "retrieve".cyan(), "<handle>".bright_black());
    println!(
        "\nA handle is the 16 characters inside a marker, for example\n  {}\n",
        "[OMNI: 50 lines already shown, omni retrieve cd900c16a4a94eb2]".bright_black()
    );
}

pub fn run(args: &[String], store: &Store) -> Result<()> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }
    super::check_flags("retrieve", args, FLAGS)?;

    // `args` is the whole argv, so the handle is the first token after the
    // subcommand that is not a flag.
    let handle = args
        .iter()
        .skip_while(|a| *a != "retrieve")
        .skip(1)
        .find(|a| !a.starts_with('-'));

    let Some(handle) = handle else {
        print_help();
        anyhow::bail!("no handle given");
    };

    // Accept what a marker prints and what a model might paste back: bare, in
    // quotes, or still wrapped in the MCP call it used to name.
    let handle = handle
        .trim_start_matches("omni_retrieve(")
        .trim_end_matches(')')
        .trim_matches('"');

    match store.retrieve_rewind(handle) {
        Some(content) => {
            // The frame goes to stderr and the payload to stdout, byte for byte.
            // #463 asked for this surface to be framed like the others, and a
            // header on stdout would have done it by editing archived bytes that
            // a caller is about to paste or parse. Same rule the pipeline holds
            // itself to: what a later step reads is not ours to decorate.
            let lines = content.lines().count();
            eprintln!(
                "{} {} · {} lines · {}",
                "omni retrieve".bold().cyan(),
                handle.bright_black(),
                lines,
                super::stats::format_bytes(content.len() as u64).bright_black()
            );
            print!("{content}");
            if !content.ends_with('\n') {
                println!();
            }
            Ok(())
        }
        // Naming the retention window matters more than the failure does: a
        // handle older than the working tier is gone on purpose, and "not found"
        // alone reads as a bug in the archive.
        None => anyhow::bail!(
            "no archived content for `{handle}`. Handles live as long as the working tier, 30 days; older ones are pruned"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn prints_what_the_marker_archived() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = Store::open_path(&dir.path().join("omni.db")).expect("store");
        let handle = store
            .store_rewind("the original forty lines\n")
            .expect("archived");

        assert!(run(&args(&["omni", "retrieve", &handle]), &store).is_ok());
    }

    /// A model handing back the marker verbatim is the likeliest way this is
    /// called, so the shapes a marker has ever printed all have to work.
    #[test]
    fn accepts_the_handle_in_every_shape_a_marker_has_printed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = Store::open_path(&dir.path().join("omni.db")).expect("store");
        let handle = store.store_rewind("content\n").expect("archived");

        for form in [
            handle.clone(),
            format!("\"{handle}\""),
            format!("omni_retrieve(\"{handle}\")"),
        ] {
            assert!(
                run(&args(&["omni", "retrieve", &form]), &store).is_ok(),
                "form {form} was not understood"
            );
        }
    }

    #[test]
    fn says_why_a_missing_handle_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = Store::open_path(&dir.path().join("omni.db")).expect("store");

        let err = run(&args(&["omni", "retrieve", "0000000000000000"]), &store)
            .expect_err("an unknown handle is an error");

        assert!(err.to_string().contains("30 days"), "{err}");
    }
}
