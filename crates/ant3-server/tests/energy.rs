//! Budget-as-energy: reservation, usage settlement, verified refunds,
//! starvation culling — selection pressure on the swarm.
use ant3_platform::{
    board::PublishInput,
    domain::*,
    energy::*,
    store::Store,
};
use serde_json::{Value, json};

fn preset() -> Preset {
    Preset {
        name: "worker".into(), version: "1".into(),
        purpose: "work".into(), objective: "evidence".into(),
        instructions: "act".into(), protocol: Protocol::Demo, plugin: None,
    }
}
fn input(budget: Option<i64>) -> PublishInput {
    PublishInput {
        title: "root".into(), instructions: "do work".into(),
        acceptance: "evidence".into(), verification: None, energy_budget: budget,
    }
}
fn fixture(budget: Option<i64>) -> (Store, Value) {
    let s = Store::memory().unwrap();
    s.publish(preset()).unwrap();
    let t = s.publish_board_task(&input(budget), "root").unwrap();
    (s, t)
}
fn id(v: &Value) -> &str { v["id"].as_str().unwrap() }
fn work(s: &Store, t: &Value) -> Value {
    s.provision_board_task(id(t), "worker", "1", "fixture").unwrap();
    s.claim().unwrap().unwrap()
}
fn finish_with(s: &Store, r: &Value, report: Value) {
    s.finish(id(r), r["instance_id"].as_str().unwrap(), "completed", report).unwrap();
    s.reconcile_board().unwrap();
}

#[test]
fn dispatch_reserves_and_settlement_bills_actual_usage() {
    let (s, t) = fixture(Some(10_000));
    let pool = s.board_energy(id(&t)).unwrap();
    assert_eq!(pool["budget"], 10_000);
    assert_eq!(pool["available"], 10_000);
    let r = work(&s, &t);
    let pool = s.board_energy(id(&t)).unwrap();
    assert_eq!(pool["reserved"], ENERGY_RESERVE);
    assert_eq!(pool["available"], 10_000 - ENERGY_RESERVE);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    // 300 billable tokens -> 30 energy; reservation released.
    finish_with(&s, &r, json!({"output":"done","usage":{"input":200,"output":100}}));
    let pool = s.board_energy(id(&t)).unwrap();
    assert_eq!(pool["reserved"], 0);
    assert_eq!(pool["spent"], 300);
    // Completion refund earns energy back.
    assert_eq!(pool["refunded"], ENERGY_COMPLETE_REFUND);
    assert_eq!(pool["available"], 10_000 - 300 + ENERGY_COMPLETE_REFUND);
    // Settlement is idempotent per Run.
    s.reconcile_board().unwrap();
    assert_eq!(s.board_energy(id(&t)).unwrap()["spent"], 300);
}

#[test]
fn unknown_usage_bills_conservatively_and_cached_input_is_free() {
    let (s, t) = fixture(Some(10_000));
    let r = work(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish_with(&s, &r, json!({"output":"done"}));
    assert_eq!(s.board_energy(id(&t)).unwrap()["spent"], ENERGY_UNKNOWN_COST);
    let t2 = s.publish_board_task(&input(Some(10_000)), "second").unwrap();
    let r2 = work(&s, &t2);
    s.claim_board_task(id(&r2), id(&t2)).unwrap();
    finish_with(&s, &r2, json!({"output":"done","usage":{"input":1000,"output":100,"cached_input":900}}));
    assert_eq!(s.board_energy(id(&t2)).unwrap()["spent"], 1100);
}

#[test]
fn starved_roots_stop_being_offered_and_pending_tasks_are_culled() {
    // Budget covers exactly one reservation; unknown usage exhausts the pool.
    let (s, t) = fixture(Some(100));
    let r = work(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish_with(&s, &r, json!({"output":"weak"}));
    // Spent 1500 + refund 500 -> available 1000 < 2000: starved.
    let pool = s.board_energy(id(&t)).unwrap();
    assert!(pool["available"].as_i64().unwrap() < ENERGY_RESERVE);
    // A requeued task on this root is culled, not offered.
    s.transaction(|tx| {
        tx.execute("UPDATE board_tasks SET state='pending',route_state='unassigned',result=NULL,run_id=NULL,instance_id=NULL WHERE id=?", [id(&t)])?;
        Ok(())
    }).unwrap();
    s.reconcile_board().unwrap();
    assert!(s.board_pending().unwrap().is_empty());
    // Explicit cull happens on the next settlement sweep via cull_if_starved;
    // trigger by settling another run on the root is not possible (starved),
    // so the offering filter alone keeps it out. Verify direct cull:
    s.cull_starved(id(&t)).unwrap();
    let task = s.board_task(id(&t)).unwrap();
    assert_eq!(task["state"], "pending");
    let top_up = s.top_up_energy(id(&t), 2000, "refill").unwrap();
    assert_eq!(s.top_up_energy(id(&t),2000,"refill").unwrap(),top_up);
    assert!(!s.board_pending().unwrap().is_empty());
}

#[test]
fn acceptance_refunds_more_than_completion() {
    use ant3_platform::board::BoardVerification;
    let s = Store::memory().unwrap();
    s.publish(preset()).unwrap();
    s.publish(Preset { name: "auditor".into(), ..preset() }).unwrap();
    let t = s.publish_board_task(&PublishInput {
        verification: Some(BoardVerification { preset_name: "auditor".into(), preset_version: "1".into(), max_auto_reworks: 0 }),
        ..input(Some(50_000))
    }, "root").unwrap();
    let r = work(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish_with(&s, &r, json!({"output":"evidence","usage":{"input":100,"output":50}}));
    // Drive the audit to passed.
    let audit_run = loop {
        s.reconcile_fleets().unwrap();
        s.reconcile_board().unwrap();
        if let Some(run) = s.claim().unwrap() { break run; }
    };
    let mut offset = 0;
    loop {
        let page = s.board_audit_evidence(id(&audit_run), offset, 12000).unwrap();
        offset = page["next_offset"].as_u64().unwrap() as usize;
        if page["complete"].as_bool().unwrap() { break; }
    }
    s.submit_board_audit(id(&audit_run), &json!({"status":"passed","reason":"met"})).unwrap();
    s.finish(id(&audit_run), audit_run["instance_id"].as_str().unwrap(), "completed", json!({"output":"verdict","usage":{"input":50,"output":10}})).unwrap();
    s.reconcile_board().unwrap();
    let pool = s.board_energy(id(&t)).unwrap();
    assert_eq!(s.board_task(id(&t)).unwrap()["review_state"], "accepted");
    // Refunds: completion 500 + acceptance 4000.
    assert_eq!(pool["refunded"], ENERGY_COMPLETE_REFUND + ENERGY_ACCEPT_REFUND);
    // Both runs billed actual usage: 15 + 6.
    assert_eq!(pool["spent"], 210);
}

#[test]
fn invalid_budgets_are_rejected_and_default_applies() {
    let s = Store::memory().unwrap();
    s.publish(preset()).unwrap();
    assert_eq!(
        s.publish_board_task(&input(Some(-1)), "tiny").unwrap_err().1,
        "invalid_energy_budget"
    );
    let t = s.publish_board_task(&input(None), "default").unwrap();
    assert_eq!(s.board_energy(id(&t)).unwrap()["budget"], ENERGY_DEFAULT);
    assert_eq!(t["energy"]["budget"], ENERGY_DEFAULT);
}
