//! Self-selection: idle peers respond by their own thresholds and race to
//! claim; the central classifier stands down; the pool reproduces from
//! verified experience and idle wandering burns colony energy.
use ant3_platform::{
    board::PublishInput,
    domain::*,
    store::Store,
};
use serde_json::{Value, json};

fn preset(name: &str) -> Preset {
    Preset { name: name.into(), version: "1".into(), purpose: "p".into(), objective: "o".into(), instructions: "i".into(), protocol: Protocol::Demo, plugin: None }
}
fn input(title: &str) -> PublishInput {
    PublishInput { title: title.into(), instructions: format!("do {title}"), acceptance: "evidence".into(), verification: None, energy_budget: None }
}
fn id(v: &Value) -> &str { v["id"].as_str().unwrap() }

#[test]
fn idle_peers_self_select_open_work_without_a_classifier() {
    let s = Store::memory().unwrap();
    s.publish(preset("forager")).unwrap();
    let t = s.publish_board_task(&input("gather"), "root").unwrap();
    // Sweeps maintain the pool and let peers respond; no provision_board_task.
    let mut claimed_run = None;
    for _ in 0..12 {
        s.reconcile_fleets().unwrap();
        s.self_select_sweep(true).unwrap();
        s.reconcile_fleets().unwrap();
        s.self_select_sweep(true).unwrap();
        if let Some(run) = s.claim().unwrap() {
            claimed_run = Some(run);
            break;
        }
    }
    let run = claimed_run.expect("a wandering peer should self-select the task");
    let task = s.board_task(id(&t)).unwrap();
    assert_eq!(task["state"], "running");
    assert_eq!(task["route_state"], "self-selected");
    assert!(task["route_reason"].as_str().unwrap().starts_with("self-selected:"));
    assert_eq!(task["run_id"], run["id"]);
    // The peer claims its own self-selected work idempotently and finishes.
    s.claim_board_task(id(&run), id(&t)).unwrap();
    s.claim_board_task(id(&run), id(&t)).unwrap();
    s.finish(id(&run), run["instance_id"].as_str().unwrap(), "completed", json!({"output":"gathered","usage":{"input":100,"output":50}})).unwrap();
    s.reconcile_board().unwrap();
    assert_eq!(s.board_task(id(&t)).unwrap()["state"], "completed");
    // Energy was reserved and billed on the root, not by any classifier.
    let pool = s.board_energy(id(&t)).unwrap();
    assert_eq!(pool["spent"], 150);
    // The swarm event trail records the self-selection.
    let events = s.events(0).unwrap();
    assert!(events.iter().any(|e| e["kind"] == "swarm.self_selected" && e["subject"] == id(&t)));
    assert!(events.iter().any(|e| e["kind"] == "swarm.peer_spawned"));
}

#[test]
fn pool_composition_reproduces_from_experience() {
    let s = Store::memory().unwrap();
    s.publish(preset("veteran")).unwrap();
    s.publish(preset("rookie")).unwrap();
    // Give the veteran verified successes through direct experience records.
    s.transaction(|tx| {
        tx.execute("INSERT INTO preset_experience(preset_name,preset_version,successes,failures,energy_spent) VALUES('veteran','1',10,0,1000)", [])?;
        Ok(())
    }).unwrap();
    for _ in 0..6 {
        s.reconcile_fleets().unwrap();
        s.self_select_sweep(true).unwrap();
    }
    s.reconcile_fleets().unwrap();
    let pool = s.swarm_pool().unwrap();
    // enabled reflects the env flag (off in tests); the pool still exists.
    let peers = pool["peers"].as_array().unwrap();
    let veterans = peers.iter().filter(|p| p["preset_name"] == "veteran").count();
    let rookies = peers.iter().filter(|p| p["preset_name"] == "rookie").count();
    assert!(veterans > rookies, "veterans={veterans} rookies={rookies}");
    assert!(veterans + rookies <= pool["pool_max"].as_u64().unwrap() as usize);
}

#[test]
fn disabled_mode_keeps_the_classifier_path() {
    // No env guard: self-selection off.
    let s = Store::memory().unwrap();
    s.publish(preset("forager")).unwrap();
    let _t = s.publish_board_task(&input("gather"), "root").unwrap();
    for _ in 0..4 {
        s.reconcile_fleets().unwrap();
        s.reconcile_board().unwrap();
    }
    // No wandering pool, no self-selection; task waits for the classifier.
    assert!(s.swarm_pool().unwrap()["peers"].as_array().unwrap().is_empty());
    assert!(s.claim().unwrap().is_none());
    assert_eq!(s.board_pending().unwrap().len(), 1);
}
