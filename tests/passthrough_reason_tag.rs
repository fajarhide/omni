//! `passthrough_events` has no reason column, so every gate encodes its reason
//! as a `[tag]` suffix on the command. Two of the four gates used to write the
//! bare command, which made a signal file that stripped its whole input and a
//! distiller too weak to keep indistinguishable in the table this project
//! queries to decide what to build (#254).

use omni::hooks::post_tool::process_payload;
use omni::store::sqlite::Store;
use std::sync::Arc;

fn bash_payload(command: &str, content: &str) -> String {
    serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": command },
        "tool_response": { "content": content },
    })
    .to_string()
}

fn store_in(dir: &tempfile::TempDir) -> Arc<Store> {
    Arc::new(Store::open_path(&dir.path().join("omni.db")).expect("temp store"))
}

/// Prose no distiller has a grammar for, past `MIN_DISTILL_TOKENS` so the
/// pipeline runs and lands on the sub-10% measurement rather than an early exit.
///
/// Deliberately comma-free: `format::sniff` calls any text CSV when every line
/// carries the same non-zero comma count, so prose with a steady comma rhythm
/// exits at the format gate and never reaches the gate under test.
fn undistillable_prose() -> String {
    (0..60)
        .map(|i| {
            format!(
                "The quarterly review notes that section {i} remains open pending \
                 a decision from the working group which has not yet met.\n"
            )
        })
        .collect()
}

#[test]
fn tags_the_gate_when_a_distiller_misses_the_guardrail() {
    // Arrange
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(&dir);
    let payload = bash_payload("date", &undistillable_prose());

    // Act
    let _ = process_payload(&payload, Some(store.clone()), None);

    // Assert
    let rows = store.recent_passthroughs(8);
    assert!(
        rows.iter().any(|r| r.contains("[below guardrail]")),
        "a weak distiller must name its gate, got: {rows:?}"
    );
}

#[test]
fn tags_the_gate_when_the_payload_is_structured() {
    // The format sniff already tagged before #254. Kept so the assertion above
    // cannot pass by an empty table if the sub-10% path stops recording.
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(&dir);
    let payload = bash_payload("az vm list -o json", r#"[{"name":"vm-1","id":"/subs/x"}]"#);

    let _ = process_payload(&payload, Some(store.clone()), None);

    let rows = store.recent_passthroughs(8);
    assert!(
        rows.iter().any(|r| r.contains("[structured:json]")),
        "the format gate must name the format, got: {rows:?}"
    );
}

#[test]
fn every_recorded_passthrough_names_a_gate() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(&dir);

    for payload in [
        bash_payload("date", &undistillable_prose()),
        bash_payload("az vm list -o json", r#"[{"name":"vm-1"}]"#),
        bash_payload("kubectl get pods -o yaml", "apiVersion: v1\nkind: List\nitems: []\n"),
    ] {
        let _ = process_payload(&payload, Some(store.clone()), None);
    }

    let rows = store.recent_passthroughs(16);
    assert!(!rows.is_empty(), "no passthrough was recorded at all");
    for row in &rows {
        assert!(
            row.contains('[') && row.ends_with(']'),
            "untagged passthrough row is ambiguous between gates: {row:?}"
        );
    }
}
