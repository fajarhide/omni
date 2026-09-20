//! Samples `bench-corpus` into the blocks the pipeline actually scores, so a
//! labeller grades what ships (#818).
//!
//! Three things it does that a hand-rolled script gets wrong:
//!
//! * It calls `score_segments` through the profile the hook resolves, so a
//!   payload is split the way production splits it, and the tier beside each
//!   block comes from `semantic::classify_block`, which is the classifier every
//!   scoring path uses. `scorer::classify_line` has no caller in the pipeline.
//! * It runs the shipping redactor over every block before writing it, so a
//!   credential assignment in a recorded payload does not travel into a label
//!   set.
//! * It counts, and refuses to write, what still matches the operator's own
//!   sensitive-name patterns. This corpus is real command output: 27.2% of its
//!   traces carry an employer, client or infrastructure name. Those patterns are
//!   the operator's, read from a file, and are deliberately not in this repo.
//!
//! ```sh
//! OMNI_LABEL_PATTERNS=~/.omni/label-patterns.txt \
//!   cargo run --example sample_blocks -- bench-corpus 40 > sample.jsonl
//! ```
use std::collections::BTreeMap;
use std::io::Write;

use omni::pipeline::registry;
use omni::pipeline::scorer;

fn main() {
    let mut args = std::env::args().skip(1);
    let corpus = args.next().unwrap_or_else(|| "bench-corpus".to_string());
    let per_class: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(40);

    let patterns: Vec<String> = std::env::var("OMNI_LABEL_PATTERNS")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| {
            s.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(str::to_lowercase)
                .collect()
        })
        .unwrap_or_default();

    let traces = std::fs::read_to_string(format!("{corpus}/traces.jsonl"))
        .unwrap_or_else(|e| panic!("{corpus}/traces.jsonl: {e}"));

    // The class comes from the manifest rather than being recomputed here: it is
    // pinned with the corpus (#704), so a sample taken today and one taken after
    // a routing change are stratified the same way.
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{corpus}/manifest.json"))
            .unwrap_or_else(|e| panic!("{corpus}/manifest.json: {e}")),
    )
    .expect("manifest json");
    let mut class_of: BTreeMap<(i64, i64), String> = BTreeMap::new();
    for e in manifest["entries"].as_array().into_iter().flatten() {
        let id = e["id"].as_i64().unwrap_or(-1);
        let seq = e["seq"].as_i64().unwrap_or(-1);
        let class = e["class"].as_str().unwrap_or("[unclassified]").to_string();
        class_of.insert((id, seq), class);
    }

    let mut taken: BTreeMap<String, usize> = BTreeMap::new();
    let mut blocks = 0usize;
    let mut flagged = 0usize;
    let out = std::io::stdout();
    let mut out = out.lock();

    for line in traces.lines() {
        let Ok(trace) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let command = trace["command"].as_str().unwrap_or("");
        let payload = trace["payload"].as_str().unwrap_or("");
        if payload.is_empty() {
            continue;
        }
        let key = (
            trace["id"].as_i64().unwrap_or(-1),
            trace["seq"].as_i64().unwrap_or(-1),
        );
        let class = class_of
            .get(&key)
            .cloned()
            .unwrap_or_else(|| "[unclassified]".to_string());
        let seen = taken.entry(class.clone()).or_default();
        if *seen >= per_class {
            continue;
        }
        *seen += 1;

        let profile = registry::resolve_profile_for_chain(command);
        for (i, seg) in scorer::score_segments(payload, profile.segmentation, None, command)
            .iter()
            .enumerate()
        {
            let text = omni::distillers::system_ops::redact_sensitive_assignments(&seg.content)
                .unwrap_or_else(|| seg.content.clone());
            let lower = text.to_lowercase();
            if patterns.iter().any(|p| lower.contains(p)) {
                flagged += 1;
                continue;
            }
            blocks += 1;
            let row = serde_json::json!({
                "trace": trace["id"],
                "seq": trace["seq"],
                "block": i,
                "class": class,
                "tier": format!("{:?}", seg.tier),
                "text": text,
            });
            let _ = writeln!(out, "{row}");
        }
    }

    eprintln!("classes sampled: {}", taken.len());
    eprintln!("blocks written:  {blocks}");
    if patterns.is_empty() {
        eprintln!(
            "blocks held back: 0, and no patterns were given. This corpus is real command \
             output: set OMNI_LABEL_PATTERNS before any of it leaves the machine."
        );
    } else {
        eprintln!("blocks held back by the operator's patterns: {flagged}");
    }
}
