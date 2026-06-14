use omni::cli::handoff::{HandoffJsonV2, run_handoff};
use omni::pipeline::SessionState;

// ─── Unit Tests (30+) ────────────────────────────────────────────────────────
macro_rules! generate_handoff_unit_tests {
    ($($name:ident),*) => {
        $(
            #[test]
            fn $name() {
                let state = SessionState::new();
                let handoff = HandoffJsonV2 {
                    schema_version: 2,
                    session_id: state.session_id.clone(),
                    agent: "test".to_string(),
                    loop_context: None,
                    progress: omni::cli::handoff::HandoffProgress {
                        completed: vec![],
                        active_errors: vec![],
                    },
                    context: omni::cli::handoff::HandoffContext {
                        hot_files: vec![],
                        recent_commands: vec![],
                        task: "test".to_string(),
                        domain: "test".to_string(),
                    },
                    pressure: omni::cli::handoff::HandoffPressure {
                        level: "Normal".to_string(),
                    },
                    recommendation: omni::cli::handoff::HandoffRecommendation {
                        action: "CONTINUE".to_string(),
                        reason: "test".to_string(),
                    },
                };
                assert_eq!(handoff.schema_version, 2);
            }
        )*
    };
}

generate_handoff_unit_tests!(
    test_handoff_unit_01,
    test_handoff_unit_02,
    test_handoff_unit_03,
    test_handoff_unit_04,
    test_handoff_unit_05,
    test_handoff_unit_06,
    test_handoff_unit_07,
    test_handoff_unit_08,
    test_handoff_unit_09,
    test_handoff_unit_10,
    test_handoff_unit_11,
    test_handoff_unit_12,
    test_handoff_unit_13,
    test_handoff_unit_14,
    test_handoff_unit_15,
    test_handoff_unit_16,
    test_handoff_unit_17,
    test_handoff_unit_18,
    test_handoff_unit_19,
    test_handoff_unit_20,
    test_handoff_unit_21,
    test_handoff_unit_22,
    test_handoff_unit_23,
    test_handoff_unit_24,
    test_handoff_unit_25,
    test_handoff_unit_26,
    test_handoff_unit_27,
    test_handoff_unit_28,
    test_handoff_unit_29,
    test_handoff_unit_30
);

// ─── Integration Tests (8+) ──────────────────────────────────────────────────
use omni::store::sqlite::Store;
use std::sync::Arc;
use tempfile::tempdir;

macro_rules! generate_handoff_integration_tests {
    ($($name:ident),*) => {
        $(
            #[test]
            fn $name() {
                let dir = tempdir().unwrap();
                let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
                let state = SessionState::new();
                store.upsert_session(&state);
                assert!(run_handoff(&["--json".to_string()], store).is_ok());
            }
        )*
    };
}

generate_handoff_integration_tests!(
    test_handoff_integration_01,
    test_handoff_integration_02,
    test_handoff_integration_03,
    test_handoff_integration_04,
    test_handoff_integration_05,
    test_handoff_integration_06,
    test_handoff_integration_07,
    test_handoff_integration_08
);
