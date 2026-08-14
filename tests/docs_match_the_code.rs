//! The manual, checked against the code it describes (#520).
//!
//! This class of defect has been fixed by hand four times: #180 named two
//! deleted subcommands, #185 two removed flags, #390 four whole documents, and
//! the sweep in #521 found `omni history` and `omni insight` sitting in shell
//! blocks that exit 1, plus a tool count reading 26 against 25 in the source.
//! Every round was found by a person happening to open one page.
//!
//! Two of the four kinds are mechanical, and this covers those. The fourth,
//! prose describing the pipeline in the wrong order, is not checkable here: the
//! fix for that is for two pages to stop each holding their own copy.
//!
//! **Why the source and not the binary.** Probing `omni <cmd>` for real would be
//! the honest level, and is unsafe: `reset` drops the database, `dashboard`
//! binds a port and blocks, `update` reaches the network. Reading the one
//! `match` that dispatches them keeps the single source of truth without
//! running anything.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every markdown file the manual, the README and CONTRIBUTING are made of.
fn doc_files() -> Vec<PathBuf> {
    let root = repo_root();
    // Both books (#539). The prose is translated and the commands are not, so a
    // shell fence in the Indonesian manual can name a deleted subcommand exactly
    // as easily as the English one, and nothing else looks at those files.
    let mut out: Vec<PathBuf> = ["docs/website/src", "docs/website/src-id"]
        .iter()
        .flat_map(|dir| walkdir::WalkDir::new(root.join(dir)).into_iter())
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| e.path().to_path_buf())
        .collect();
    out.push(root.join("README.md"));
    // CONTRIBUTING.md was the one prose file with a count in it that nothing
    // read, and it held `26 tools` for two releases after the number became 25
    // (#541). It is not part of either book, so the walk above never sees it.
    out.push(root.join("CONTRIBUTING.md"));
    out.sort();
    out
}

/// The subcommands `main.rs` actually dispatches, alias arms included.
///
/// Read from the source rather than duplicated here, so this cannot become the
/// second list that drifts from the first, which is the defect it exists to
/// catch. A structural change to `main.rs` that defeats the scan empties the set
/// and the sanity check below fails loudly rather than passing vacuously.
fn real_subcommands() -> BTreeSet<String> {
    let main = std::fs::read_to_string(repo_root().join("src/main.rs")).expect("read main.rs");
    let arm = regex::Regex::new(r#"(?m)^\s*("(?:[a-z][a-z-]*)"(?:\s*\|\s*"[a-z-]+")*)\s*=>"#)
        .expect("valid regex");
    let name = regex::Regex::new(r#""([a-z][a-z-]*)""#).expect("valid regex");

    let found: BTreeSet<String> = arm
        .captures_iter(&main)
        .flat_map(|c| {
            name.captures_iter(c.get(1).expect("group 1").as_str())
                .map(|n| n[1].to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    for known in ["exec", "stats", "doctor", "retrieve", "init"] {
        assert!(
            found.contains(known),
            "the scan of main.rs missed `{known}`, so it is no longer reading the \
             dispatch table and every assertion below would pass vacuously. Found: {found:?}"
        );
    }
    found
}

/// `omni <word>` written inside a shell fence, with the file and line.
///
/// Fence-scoped on purpose. `integrations/loops.md` names `omni handoff` in
/// prose precisely to say it is **not** a subcommand, and a check that read
/// prose would have to be taught about sentences like that one.
fn commands_in_shell_blocks() -> Vec<(String, String, usize)> {
    let call = regex::Regex::new(r"(?:^|\s)omni\s+([a-z][a-z-]{2,})").expect("valid regex");
    let mut out = Vec::new();

    for path in doc_files() {
        let text = std::fs::read_to_string(&path).expect("read doc");
        let shown = path
            .strip_prefix(repo_root())
            .unwrap_or(&path)
            .display()
            .to_string();
        let mut in_shell = false;

        for (i, line) in text.lines().enumerate() {
            if line.starts_with("```") {
                in_shell = matches!(line.trim_end(), "```sh" | "```bash" | "```shell");
                continue;
            }
            if !in_shell || line.trim_start().starts_with('#') {
                continue;
            }
            if let Some(c) = call.captures(line) {
                out.push((c[1].to_string(), shown.clone(), i + 1));
            }
        }
    }
    out
}

/// A documented command a reader can copy, paste, and watch exit 1.
#[test]
fn every_command_in_a_shell_block_is_a_real_subcommand() {
    let real = real_subcommands();
    let broken: Vec<String> = commands_in_shell_blocks()
        .into_iter()
        .filter(|(cmd, _, _)| !real.contains(cmd))
        .map(|(cmd, file, line)| format!("  {file}:{line}  omni {cmd}"))
        .collect();

    assert!(
        broken.is_empty(),
        "the manual tells the reader to run commands that do not exist.\n{}\n\
         If the feature moved to MCP, say so in prose instead of leaving it in a \
         shell block, the way integrations/loops.md handles omni handoff.",
        broken.join("\n")
    );
}

/// Counts in prose, against the thing being counted.
///
/// `26 tools` outlived `omni_learn` in three files at once, because a number in
/// a sentence has nothing pointing at it when the thing it counts is deleted.
#[test]
fn every_documented_count_matches_the_code() {
    let root = repo_root();
    let server =
        std::fs::read_to_string(root.join("src/mcp/server.rs")).expect("read mcp/server.rs");
    let tools: BTreeSet<&str> = regex::Regex::new(r#"name = "(omni_[a-z_]+)""#)
        .expect("valid regex")
        .captures_iter(&server)
        .map(|c| c.get(1).expect("group 1").as_str())
        .collect();

    let distillers = std::fs::read_dir(root.join("src/distillers"))
        .expect("read distillers")
        .filter_map(Result::ok)
        .filter(|e| {
            let p = e.path();
            p.extension().is_some_and(|x| x == "rs") && p.file_stem().is_some_and(|s| s != "mod")
        })
        .count();

    // (regex, what the number has to equal, what it is counting)
    let claims = [
        // `perkakas` because the Indonesian book states the same counts and the
        // English noun never appears in it, so every count in it was unchecked.
        (
            r"(\d+)\s+(?:MCP\s+)?(?:tools|perkakas)",
            tools.len(),
            "MCP tools",
        ),
        (r"(\d+)\s+content filters", distillers, "distillers"),
        // The ledger page states three floors, in both languages. A constant
        // moving without the prose is #541 one file over: a number in text that
        // nothing reads. Each pattern carries enough of the sentence around the
        // number to belong to one floor and nothing else, since `under N bytes`
        // on its own is ordinary prose that any page may write for other reasons.
        (
            r"(?:save|menghemat) (\d+) (?:bytes over its marker|byte di atas penandanya)",
            omni::guard::limits::MIN_LEDGER_RUN_GAIN,
            "bytes a session-origin run must save",
        ),
        (
            r"(?:under|di bawah) (\d+) (?:bytes never reaches|byte tidak pernah)",
            omni::guard::limits::MIN_LEDGER_INPUT,
            "bytes below which the ledger is skipped",
        ),
        (
            r"(?:entire output needs|seluruh keluaran butuh) (\d+) (?:bytes|byte)",
            omni::guard::limits::MIN_WHOLE_OUTPUT_FOLD,
            "bytes a whole-output fold needs",
        ),
    ];

    let mut wrong = Vec::new();
    // A pattern that matches nothing is a check that quietly stopped checking,
    // which is the failure #541 was. Rewording a sentence past its pattern has to
    // be as loud as getting the number wrong.
    let mut seen = vec![0usize; claims.len()];
    for path in doc_files() {
        let text = std::fs::read_to_string(&path).expect("read doc");
        let shown = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();
        for (n, (pattern, actual, what)) in claims.iter().enumerate() {
            let re = regex::Regex::new(pattern).expect("valid regex");
            for (i, line) in text.lines().enumerate() {
                for c in re.captures_iter(line) {
                    seen[n] += 1;
                    let claimed: usize = c[1].parse().expect("digits");
                    if claimed != *actual {
                        wrong.push(format!(
                            "  {shown}:{}  says {claimed} {what}, the code has {actual}",
                            i + 1
                        ));
                    }
                }
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "the manual states counts the code disagrees with.\n{}",
        wrong.join("\n")
    );

    let unmatched: Vec<&str> = claims
        .iter()
        .zip(&seen)
        .filter(|(_, hits)| **hits == 0)
        .map(|((_, _, what), _)| *what)
        .collect();
    assert!(
        unmatched.is_empty(),
        "no document states these any more, so nothing is being checked: {}",
        unmatched.join(", ")
    );
}

/// Every marker the ledger can print, against the page that lists them.
///
/// #522 added a second pair of forms, for a fold that covers the whole reply,
/// and the manual kept listing two of four for a release (#526). The check in
/// this file could not see it: a marker is neither a command nor a count. It is
/// the cheapest of the three to guard, because the templates are string
/// literals in one file.
///
/// Matched on the stable half of each template, before the first `{`, since the
/// rest is a runtime count and a handle.
#[test]
fn every_marker_the_ledger_can_print_is_documented() {
    let root = repo_root();
    let ledger = std::fs::read_to_string(root.join("src/ledger/mod.rs")).expect("read ledger");
    let page = std::fs::read_to_string(root.join("docs/website/src/use/markers.md"))
        .expect("read markers.md");

    let templates: BTreeSet<String> = regex::Regex::new(r#""(\[OMNI: [^"]*)""#)
        .expect("valid regex")
        .captures_iter(&ledger)
        .map(|c| {
            let t = c.get(1).expect("group 1").as_str();
            // "[OMNI: {lines} lines already shown, …" → "[OMNI: "
            // "[OMNI: identical to {lines} …"         → "[OMNI: identical to "
            t.split('{').next().unwrap_or(t).to_string()
        })
        .collect();

    assert!(
        templates.len() >= 2,
        "found {} marker templates in the ledger, so the scan is no longer reading them",
        templates.len()
    );

    let missing: Vec<&String> = templates.iter().filter(|t| !page.contains(*t)).collect();
    assert!(
        missing.is_empty(),
        "the ledger can print markers `use/markers.md` never shows: {missing:?}\n\
         A reader meets these in their own output and has nowhere to look them up."
    );
}

/// The scan itself, because a checker that reads nothing reports no problems.
#[test]
fn the_scan_reaches_the_manual_and_the_readme() {
    let files = doc_files();
    assert!(
        files.len() > 20,
        "only {} doc files found, so the walk is not reaching the manual",
        files.len()
    );
    assert!(
        files.iter().any(|p| p.ends_with("README.md")),
        "the README is not being scanned"
    );
    assert!(
        files.iter().any(|p| p.ends_with("CONTRIBUTING.md")),
        "CONTRIBUTING is not being scanned"
    );
    assert!(
        commands_in_shell_blocks().len() > 10,
        "no commands found in any shell block, so the fence scan is broken"
    );
}
