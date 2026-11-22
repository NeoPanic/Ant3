//! Response-threshold division of labor and crowding avoidance.
use ant3_platform::{
    board::PublishInput,
    division::{response_threshold, stimulus},
    domain::*,
    store::Store,
};
use serde_json::{Value, json};

fn input(title: &str) -> PublishInput {
    PublishInput {
        title: title.into(),
        instructions: format!("Inspect {title}"),
        acceptance: "Evidence is reported".into(),
        verification: None,
        energy_budget: None,
    }
}
fn preset(name: &str) -> Preset {
    Preset {
        name: name.into(),
        version: "1".into(),
        purpose: "Read evidence".into(),
        objective: "Report findings".into(),
        instructions: "Inspect supplied evidence".into(),
        protocol: Protocol::Demo,
        plugin: None,
    }
}
fn fixture() -> (Store, Value) {
    let s = Store::memory().unwrap();
    s.publish(preset("reader")).unwrap();
    let t = s.publish_board_task(&input("root"), "root").unwrap();
    (s, t)
}
fn id(v: &Value) -> &str {
    v["id"].as_str().unwrap()
}
fn provision(s: &Store, t: &Value) -> Value {
    s.provision_board_task(id(t), "reader", "1", "fixture")
        .unwrap();
    s.claim().unwrap().unwrap()
}
fn finish(s: &Store, r: &Value, state: &str) {
    s.finish(
        id(r),
        r["instance_id"].as_str().unwrap(),
        state,
        json!({"output":"Evidence"}),
    )
    .unwrap();
    s.reconcile_board().unwrap();
}
fn experience(s: &Store, name: &str) -> Value {
    s.preset_experience()
        .unwrap()
        .into_iter()
        .find(|e| e["preset_name"] == name)
        .unwrap_or(json!({"successes":0,"failures":0,"response_threshold":response_threshold(0,0)}))
}

#[test]
fn provider_outages_do_not_retire_a_capable_preset_or_erase_billed_energy() {
    let (s, t) = fixture();
    for (index, code) in [
        "provider_network_error",
        "provider_timeout",
        "provider_http_503",
    ]
    .into_iter()
    .enumerate()
    {
        let task = if index == 0 {
            t.clone()
        } else {
            s.publish_board_task(&input(code), code).unwrap()
        };
        let run = provision(&s, &task);
        s.claim_board_task(id(&run), id(&task)).unwrap();
        s.finish(
            id(&run),
            run["instance_id"].as_str().unwrap(),
            "failed",
            json!({"error":{"code":code},"usage":{"input":100,"output":50}}),
        )
        .unwrap();
        s.reconcile_board().unwrap();
        assert!(
            s.board_energy(id(&task)).unwrap()["spent"]
                .as_i64()
                .unwrap()
                > 0
        );
    }
    assert_eq!(experience(&s, "reader")["failures"], 0);
    assert!(experience(&s, "reader")["energy_spent"].as_i64().unwrap() > 0);
    for _ in 0..3 {
        s.self_select_sweep(true).unwrap();
        s.reconcile_fleets().unwrap();
    }
    assert!(
        !s.swarm_pool().unwrap()["peers"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn opening_legacy_database_repairs_only_provider_penalties_once() {
    let path =
        std::env::temp_dir().join(format!("ant3-experience-{}.sqlite", uuid::Uuid::new_v4()));
    let s = Store::open(&path).unwrap();
    s.publish(preset("reader")).unwrap();
    for code in ["provider_network_error", "tool_failed"] {
        let task = s.publish_board_task(&input(code), code).unwrap();
        let run = provision(&s, &task);
        s.claim_board_task(id(&run), id(&task)).unwrap();
        s.finish(
            id(&run),
            run["instance_id"].as_str().unwrap(),
            "failed",
            json!({"error":{"code":code},"usage":{"input":100,"output":50}}),
        )
        .unwrap();
        s.reconcile_board().unwrap();
    }
    let spent = experience(&s, "reader")["energy_spent"].clone();
    s.transaction(|tx| {
        tx.execute("UPDATE preset_experience SET failures=2", [])?;
        tx.execute_batch("PRAGMA user_version=28;")?;
        Ok(())
    })
    .unwrap();
    drop(s);
    for _ in 0..2 {
        let s = Store::open(&path).unwrap();
        let e = experience(&s, "reader");
        assert_eq!(e["failures"], 1);
        assert_eq!(e["energy_spent"], spent);
        assert_eq!(s.list("runs").unwrap().len(), 2);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn success_lowers_and_definite_failure_raises_the_response_threshold() {
    let (s, t) = fixture();
    assert_eq!(experience(&s, "reader")["response_threshold"], 0.5);
    let r = provision(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish(&s, &r, "completed");
    let e = experience(&s, "reader");
    assert_eq!(e["successes"], 1);
    assert_eq!(e["failures"], 0);
    assert!(e["response_threshold"].as_f64().unwrap() < 0.5);
    // A definite failure on a fresh root raises the threshold back up.
    let other = s.publish_board_task(&input("other"), "other").unwrap();
    let bad = provision(&s, &other);
    s.claim_board_task(id(&bad), id(&other)).unwrap();
    finish(&s, &bad, "failed");
    let e = experience(&s, "reader");
    assert_eq!(e["successes"], 1);
    assert_eq!(e["failures"], 1);
    // Threshold now also reflects cost efficiency (both runs bill unknown
    // usage): base(1,1) scaled by mean-energy efficiency.
    let eff = e["cost_efficiency"].as_f64().unwrap();
    assert_eq!(
        e["response_threshold"],
        ant3_platform::division::cost_adjusted_threshold(response_threshold(1, 1), eff)
    );
}

#[test]
fn interrupted_and_cancelled_runs_are_not_experience() {
    let (s, t) = fixture();
    let r = provision(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    s.finish(
        id(&r),
        r["instance_id"].as_str().unwrap(),
        "completed",
        json!({"error":{"code":"interrupted_outcome_unknown"}}),
    )
    .unwrap();
    s.reconcile_board().unwrap();
    assert_eq!(experience(&s, "reader")["successes"], 0);
    assert_eq!(experience(&s, "reader")["failures"], 0);
    let other = s.publish_board_task(&input("cancelled"), "cancelled").unwrap();
    let c = provision(&s, &other);
    s.claim_board_task(id(&c), id(&other)).unwrap();
    finish(&s, &c, "cancelled");
    assert_eq!(experience(&s, "reader")["failures"], 0);
}

#[test]
fn crowding_dilutes_offering_stimulus_and_spreads_work() {
    let path = std::env::temp_dir().join(format!("ant3-crowd-{}.sqlite", uuid::Uuid::new_v4()));
    let s = Store::open(&path).unwrap();
    s.publish(preset("reader")).unwrap();
    // Two roots, each with a pending task carrying equal trail strength.
    let a = s.publish_board_task(&input("a"), "a").unwrap();
    let b = s.publish_board_task(&input("b"), "b").unwrap();
    let ra = provision(&s, &a);
    s.deposit_board_trace(
        id(&ra),
        &ant3_platform::board::TraceInput { key: id(&a).into(), kind: "finding".into(), note: String::new() },
        "da",
    )
    .unwrap();
    let rb = provision(&s, &b);
    s.deposit_board_trace(
        id(&rb),
        &ant3_platform::board::TraceInput { key: id(&b).into(), kind: "finding".into(), note: String::new() },
        "db",
    )
    .unwrap();
    // Return both tasks to the pool. Root A keeps a queued worker (crowd=1);
    // root B's invitation is released so it has no active worker (crowd=0).
    drop(s);
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("UPDATE board_tasks SET route_state='unassigned',state='pending';")
        .unwrap();
    db.execute("DELETE FROM board_agents WHERE run_id=?", [id(&rb)])
        .unwrap();
    db.execute(
        "UPDATE runs SET state='cancelled' WHERE id=?",
        [id(&rb)],
    )
    .unwrap();
    drop(db);
    let s = Store::open(&path).unwrap();
    // Equal raw strength, but A is crowded: B must be offered first.
    let pending = s.board_pending().unwrap();
    assert_eq!(pending[0]["id"], b["id"]);
    assert_eq!(pending[1]["id"], a["id"]);
    assert_eq!(stimulus(2.0, 1), 1.0);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn energy_efficiency_shapes_the_response_threshold() {
    let (s, t) = fixture();
    let r = provision(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    // Cheap success: 100 billable tokens -> 10 energy.
    s.finish(id(&r), r["instance_id"].as_str().unwrap(), "completed",
        json!({"output":"Evidence","usage":{"input":80,"output":20}})).unwrap();
    s.reconcile_board().unwrap();
    let e = experience(&s, "reader");
    assert_eq!(e["successes"], 1);
    assert_eq!(e["energy_spent"], 10);
    // efficiency = 10/2000 = 0.005 -> clamped 0.25: threshold far below base.
    assert!(e["cost_efficiency"].as_f64().unwrap() <= 0.01);
    assert!(e["response_threshold"].as_f64().unwrap() < response_threshold(1, 0));
    // An expensive peer on a fresh root: unknown usage bills 1500.
    let other = s.publish_board_task(&input("expensive"), "expensive").unwrap();
    let r2 = provision(&s, &other);
    s.claim_board_task(id(&r2), id(&other)).unwrap();
    finish(&s, &r2, "completed");
    // Same preset now mixes cheap+expensive: mean 755/success, eff 0.3775.
    let e = experience(&s, "reader");
    assert_eq!(e["energy_spent"], 1510);
    let eff = e["cost_efficiency"].as_f64().unwrap();
    assert!((eff - 0.3775).abs() < 1e-6);
    let expected = ant3_platform::division::cost_adjusted_threshold(response_threshold(2, 0), eff);
    assert_eq!(e["response_threshold"], expected);
}
