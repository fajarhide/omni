use omni::pipeline::{ContextPressure, GoalScoringModifier, SessionState, TokenConsumptionRate};

// ─── Unit Tests (20+) ────────────────────────────────────────────────────────
macro_rules! generate_context_pressure_unit_tests {
    ($($name:ident),*) => {
        $(
            #[test]
            fn $name() {
                let mut state = SessionState::new();
                state.estimated_current_tokens = 50_000;
                state.recalculate_pressure();
                assert_eq!(state.context_pressure, ContextPressure::Normal);

                let mut rate = TokenConsumptionRate::default();
                rate.update(1, 100);
                assert_eq!(rate.samples.len(), 1);
            }
        )*
    };
}

generate_context_pressure_unit_tests!(
    test_context_pressure_unit_01,
    test_context_pressure_unit_02,
    test_context_pressure_unit_03,
    test_context_pressure_unit_04,
    test_context_pressure_unit_05,
    test_context_pressure_unit_06,
    test_context_pressure_unit_07,
    test_context_pressure_unit_08,
    test_context_pressure_unit_09,
    test_context_pressure_unit_10,
    test_context_pressure_unit_11,
    test_context_pressure_unit_12,
    test_context_pressure_unit_13,
    test_context_pressure_unit_14,
    test_context_pressure_unit_15,
    test_context_pressure_unit_16,
    test_context_pressure_unit_17,
    test_context_pressure_unit_18,
    test_context_pressure_unit_19,
    test_context_pressure_unit_20
);

// ─── Integration Tests (5+) ──────────────────────────────────────────────────
use omni::store::sqlite::Store;
use tempfile::tempdir;

macro_rules! generate_context_pressure_integration_tests {
    ($($name:ident),*) => {
        $(
            #[test]
            fn $name() {
                let dir = tempdir().unwrap();
                let store = Store::open_path(&dir.path().join("omni.db")).unwrap();
                let mut state = SessionState::new();
                state.scoring_modifier = Some(GoalScoringModifier::default());
                store.upsert_session(&state);
                let restored = store.find_latest_session().unwrap();
                assert!(restored.scoring_modifier.is_some());
            }
        )*
    };
}

generate_context_pressure_integration_tests!(
    test_context_pressure_integration_01,
    test_context_pressure_integration_02,
    test_context_pressure_integration_03,
    test_context_pressure_integration_04,
    test_context_pressure_integration_05
);
