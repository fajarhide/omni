//! Prints the tier `classify_line` assigns to every line of a sampled corpus.
//!
//! Calls the shipping function rather than a copy of its substring list, so the
//! labelling pilot compares against what actually runs in the hook (#818).
//!
//! echo sample.json | cargo run --example classify_lines > tiers.json

use std::io::Read;

use omni::pipeline::SignalTier;
use omni::pipeline::scorer::classify_line;

fn tier_name(t: SignalTier) -> &'static str {
    match t {
        SignalTier::Critical => "Critical",
        SignalTier::Important => "Important",
        SignalTier::Context => "Context",
        SignalTier::Noise => "Noise",
    }
}

fn main() {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw).expect("stdin");
    let samples: serde_json::Value = serde_json::from_str(&raw).expect("sample json");

    let mut out = Vec::new();
    for trace in samples.as_array().expect("array of traces") {
        let mut tiers = Vec::new();
        for line in trace["lines"].as_array().expect("lines") {
            tiers.push(tier_name(classify_line(line.as_str().unwrap_or(""))));
        }
        out.push(serde_json::json!({ "id": trace["id"], "tiers": tiers }));
    }
    println!("{}", serde_json::to_string(&out).expect("serialize"));
}
