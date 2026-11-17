use ant3_platform::{
    board::PublishInput,
    store::Store,
};
use serde_json::json;

#[test]
fn settings_defaults_merge_partial_updates_and_reload() {
    let store = Store::memory().unwrap();
    let defaults = store.settings().unwrap();
    assert_eq!(defaults["traces"]["half_life_minutes"], 30);
    assert_eq!(defaults["traces"]["max_strength"], 100.0);
    assert_eq!(defaults["self_select"]["pool_max"], 8);
    assert_eq!(defaults["self_select"]["colony_budget"], 200_000);
    assert_eq!(defaults["energy"]["default_budget"], 100_000);

    let updated = store
        .update_settings(json!({"traces":{"perception_threshold":2.5},"self_select":{"pool_max":12}}))
        .unwrap();
    assert_eq!(updated["traces"]["perception_threshold"], 2.5);
    assert_eq!(updated["traces"]["half_life_minutes"], 30);
    assert_eq!(updated["self_select"]["pool_max"], 12);
    assert_eq!(updated["energy"]["accept_refund"], 4_000);
    assert_eq!(store.settings().unwrap(), updated);
    assert_eq!(store.settings_config().unwrap().self_select.pool_max, 12);
}

#[test]
fn invalid_settings_are_rejected_without_changing_stored_values() {
    let store = Store::memory().unwrap();
    for patch in [
        json!({"self_select":{"pool_max":0}}),
        json!({"self_select":{"idle_burn_per_min":-1}}),
        json!({"traces":{"half_life_minutes":0}}),
        json!({"traces":{"perception_threshold":101.0}}),
        json!({"energy":{"tokens_per_energy":0}}),
        json!({"unknown_section":{}}),
    ] {
        assert_eq!(
            store.update_settings(patch).unwrap_err().1,
            "invalid_settings"
        );
    }
    let unchanged = store.settings().unwrap();
    assert_eq!(unchanged["self_select"]["pool_max"], 8);
    assert_eq!(unchanged["traces"]["half_life_minutes"], 30);
}

#[test]
fn saved_settings_drive_new_pools_and_trace_views() {
    let store = Store::memory().unwrap();
    store
        .update_settings(json!({
            "energy":{"default_budget":5_000},
            "traces":{"half_life_minutes":1,"max_strength":40.0}
        }))
        .unwrap();
    let task = store
        .publish_board_task(
            &PublishInput {
                title: "configured".into(),
                instructions: "inspect".into(),
                acceptance: "evidence".into(),
                verification: None,
                energy_budget: None,
            },
            "root",
        )
        .unwrap();
    assert_eq!(task["energy"]["budget"], 5_000);
    let traces = store.board_traces(task["id"].as_str().unwrap()).unwrap();
    assert_eq!(traces["half_life_ms"], 60_000);
    assert_eq!(traces["max_strength"], 40.0);
}

#[test]
fn colony_budget_accepts_the_full_configured_ceiling() {
    let store = Store::memory().unwrap();
    let updated = store
        .update_settings(json!({"self_select":{"colony_budget":1_073_741_824_i64}}))
        .unwrap();
    assert_eq!(updated["self_select"]["colony_budget"], 1_073_741_824_i64);
    assert_eq!(
        store.settings_config().unwrap().self_select.colony_budget,
        1_073_741_824
    );
    assert_eq!(
        store
            .update_settings(json!({"self_select":{"colony_budget":1_073_741_825_i64}}))
            .unwrap_err()
            .1,
        "invalid_settings"
    );
}
