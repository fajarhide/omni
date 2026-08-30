//! #710. A figure that no longer comes from the artifact cannot ship.
//!
//! The per-class table in the READMEs was measured on a trace corpus
//! `TRACE_RETENTION_DAYS` deletes, and four of its six classes stopped
//! reproducing. Nothing noticed, because nothing compared the copy against a
//! measurement, and the hero line promised the opposite in as many words: "every
//! number replays on your own history".
//!
//! This is a Rust test rather than a script for one reason. The obvious home was
//! `docs/internal/runbooks/`, next to the other checks, and that tree is
//! gitignored and `.gitignore` ignores `*.py` repo-wide, so a Python guard cannot
//! run in CI at all. `tests/docs_match_the_code.rs` is the precedent.
//!
//! The rendering half stays in
//! `docs/internal/runbooks/check-published-figures.py`, which writes the table.
//! Re-implementing that here would be two copies of one string, which this repo
//! has been bitten by three times (#452, #454, #456). So the generator renders and
//! this asserts, and the assertion is about identity rather than about layout: a
//! table quoting the current corpus hash was produced from the current corpus.

use std::path::{Path, PathBuf};

/// Every figure from the corpus deleted before #704 froze one, as the copy writes
/// it. Numbers rather than names, so unlike a client list this is safe to keep in
/// a public repo.
const RETIRED: &[&str] = &[
    "89.6",  // file read, with the ledger. The hero line's second half.
    "39.2",  // file read, filters only
    "56.2",  // other, with the ledger
    "29.1",  // other, filters only
    "69.6",  // aggregate, with the ledger
    "32.6",  // aggregate, filters only
    "23.09", // corpus bytes
    "5,984", // corpus traces
    "3,703", // other, calls
    "10.93", // file read, input MB
    "11.05", // other, input MB
    // #712. The head to head from that same corpus. It was withdrawn from every
    // claim surface rather than restated, and the guard is what keeps it withdrawn:
    // these are the numbers a copy edit would reach for to fill the gap back in.
    "65.8", // headroom dedup over our filters
    "49.4", // lean-ctx compress
    "61.4", // rtk + our ledger
    "61.7", // caveman + our ledger
            // rtk alone (6.2) and caveman alone (6.8) are deliberately absent. The
            // match is a substring, so `6.2` also hits every `0.6.2` in a changelog
            // link or a version string. A two-significant-digit figure cannot be
            // guarded this way, and a check that fires on version numbers gets
            // disabled, which is worse than the gap.
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Surfaces where a figure reads as what OMNI does today.
///
/// The benchmarks page is deliberately absent. It is an archive that labels every
/// run with the corpus it came from, and says so in its own opening: deleting a
/// published number is worse than labelling it. Scanning it would force deleting
/// history to turn a check green. It is held to the other rule below instead.
fn claim_files() -> Vec<PathBuf> {
    let mut out = vec![root().join("README.md")];
    let i18n = root().join("i18n");
    if let Ok(entries) = std::fs::read_dir(&i18n) {
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("README-") && n.ends_with(".md"))
            })
            .collect();
        found.sort();
        out.extend(found);
    }
    out.push(root().join("docs/website/src/index.md"));
    out
}

/// `0.7.10` as `[0, 7, 10]`, so it orders after `0.7.7`. A component that is not a
/// number sorts first rather than panicking: the artifact directory is not a place
/// to be strict about someone's stray file.
fn version_key(path: &Path) -> Vec<u64> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// The newest measurement, which is what copy is allowed to quote.
fn artifact() -> serde_json::Value {
    let dir = root().join("docs/benchmarks");
    let mut paths: Vec<(Vec<u64>, PathBuf)> = std::fs::read_dir(&dir)
        .expect("docs/benchmarks exists; run `make bench`")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| (version_key(&p), p))
        .collect();
    // Greptile on #710. Sorted by version, not lexicographically: `0.7.10.json`
    // sorts *before* `0.7.7.json` as a string, so the moment a two-digit patch
    // exists the checks below start validating the copy against a stale corpus and
    // passing. The artifacts are named by version, so read them as versions.
    paths.sort();
    let newest = &paths.last().expect("at least one docs/benchmarks/*.json").1;
    let text = std::fs::read_to_string(newest).expect("read artifact");
    serde_json::from_str(&text).expect("artifact is JSON")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn no_claim_surface_quotes_a_retired_figure() {
    let mut findings = Vec::new();
    for path in claim_files() {
        let body = read(&path);
        let name = path.strip_prefix(root()).unwrap_or(&path).display();
        for figure in RETIRED {
            // Both decimal conventions. `README-id.md` and `-vi.md` write `89,6%`,
            // so a scan for `89.6` alone reports them clean while they still carry
            // the figure. That is #541 exactly, and it happened live: the first
            // grep over these files missed two of them.
            for written in [figure.to_string(), figure.replace('.', ",")] {
                let n = body.matches(&written).count();
                if n > 0 {
                    findings.push(format!("{name}: retired figure `{written}` x{n}"));
                }
            }
        }
    }
    assert!(
        findings.is_empty(),
        "these came from a corpus that no longer exists (#710); \
         replace them with the current artifact or drop the claim:\n{}",
        findings.join("\n")
    );
}

#[test]
fn the_archive_names_the_current_corpus() {
    let data = artifact();
    let sha = data["corpus"]["sha256"]
        .as_str()
        .expect("artifact carries corpus.sha256");
    let short = &sha[..16];
    let page = root().join("docs/website/src/develop/benchmarks.md");
    assert!(
        read(&page).contains(short),
        "benchmarks.md does not name the current corpus `{short}`, \
         so the archive is a release behind the measurement"
    );
}

#[test]
fn the_generated_table_was_produced_from_the_current_corpus() {
    let data = artifact();
    let sha = data["corpus"]["sha256"].as_str().expect("corpus.sha256");
    let short = &sha[..16];
    let readme = read(&root().join("README.md"));
    let region = readme
        .split_once("<!-- omni:corpus-table:start -->")
        .and_then(|(_, rest)| rest.split_once("<!-- omni:corpus-table:end -->"))
        .map(|(inner, _)| inner.to_string())
        .expect("README carries the generated corpus-table region");
    assert!(
        region.contains(short),
        "the README's generated table does not name corpus `{short}`; \
         regenerate it with docs/internal/runbooks/check-published-figures.py"
    );

    // Greptile on #710. The hash alone is not enough: a percentage edited by hand
    // keeps the hash and passes, which leaves the table free to disagree with the
    // measurement it names. So every number in the region has to be a number the
    // artifact holds.
    //
    // Values, not layout. Asserting the exact rendering would put a second copy of
    // the generator's format string here, and two copies of one string is the shape
    // that produced #452, #454 and #456. This compares the set instead, so the
    // generator stays free to change the table's shape.
    let mut allowed: Vec<String> = vec![
        data["version"].as_str().unwrap_or_default().to_string(),
        short.to_string(),
    ];
    let c = &data["corpus"];
    allowed.push(commas(c["traces"].as_u64().unwrap_or(0)));
    allowed.push(format!(
        "{:.2}",
        c["bytes"].as_u64().unwrap_or(0) as f64 / 1e6
    ));
    allowed.push(c["sessions"].as_u64().unwrap_or(0).to_string());
    for row in data["result"]["by_class"]
        .as_object()
        .expect("by_class is an object")
        .values()
    {
        allowed.push(commas(row["calls"].as_u64().unwrap_or(0)));
        allowed.push(format!(
            "{:.2}",
            row["input_bytes"].as_u64().unwrap_or(0) as f64 / 1e6
        ));
        for key in [
            "filters_pct",
            "with_ledger_pct",
            "repetition_available_pct",
            "capture_rate_pct",
        ] {
            // One decimal, because that is how the figure is born: the replay
            // prints `{:>9.1}%`, so `0.0` reaches the artifact as 0.0 and the
            // table as "0.0". Rust's default float formatting writes that as
            // "0", which is not a number the table contains.
            allowed.push(format!("{:.1}", row[key].as_f64().unwrap_or(-1.0)));
        }
    }

    // The hash is hex, so digit runs inside it (`63218`, `78`) read as numbers that
    // no artifact field holds. It is checked above as a whole string; drop it before
    // scanning rather than teaching the scanner about hex.
    let scannable = region.replace(short, "");
    let mut foreign = Vec::new();
    for token in numbers(&scannable) {
        if !allowed.contains(&token) {
            foreign.push(token);
        }
    }
    assert!(
        foreign.is_empty(),
        "the generated table carries numbers the artifact does not: {foreign:?}\n\
         regenerate it rather than editing it by hand"
    );
}

/// `1056` as `1,056`, matching how the generator writes a count.
fn commas(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Every numeric token in the text, digits plus the separators a figure uses.
///
/// Trailing punctuation is trimmed so `8.42 MB,` yields `8.42`, and a token that is
/// only separators is dropped.
fn numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ((ch == '.' || ch == ',') && !current.is_empty()) {
            current.push(ch);
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out.into_iter()
        .map(|t| t.trim_end_matches(['.', ',']).to_string())
        .filter(|t| t.chars().any(|c| c.is_ascii_digit()))
        .collect()
}
