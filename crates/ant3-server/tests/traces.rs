//! Pheromone trace field: bounded agent deposits, verified-event
//! reinforcement, once-per-Run reuse, lazy evaporation and root isolation.
use ant3_platform::{
    board::{PublishInput, ReviewDecision, ReviewInput, SplitInput, TraceInput},
    domain::*,
    store::Store,
};
use serde_json::{Value, json};

fn input(title: &str) -> PublishInput {
    PublishInput {
        title: title.into(),
        instructions: format!("Inspect {title}"),
        acceptance: "Evidence is reported".into(),
    }
}
fn fixture() -> (Store, Value) {
    let s = Store::memory().unwrap();
    s.publish(Preset {
        name: "reader".into(),
        version: "1".into(),
        purpose: "Read evidence".into(),
        objective: "Report findings".into(),
        instructions: "Inspect supplied evidence".into(),
        protocol: Protocol::Demo,
        plugin: None,
    })
    .unwrap();
    let t = s.publish_board_task(&input("root"), "root").unwrap();
    (s, t)
}
fn id(v: &Value) -> &str {
    v["id"].as_str().unwrap()
}
fn provision(s: &Store, t: &Value) -> Value {
    s.provision_board_task(id(t), "reader", "1", "matches evidence inspection")
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
fn deposit(s: &Store, run: &str, key: &str, kind: &str, command: &str) -> Value {
    s.deposit_board_trace(
        run,
        &TraceInput {
            key: key.into(),
            kind: kind.into(),
            note: "signal".into(),
        },
        command,
    )
    .unwrap()
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "expected ~{expected}, got {actual}"
    );
}
fn strength(s: &Store, root: &str, key: &str) -> f64 {
    s.board_traces(root).unwrap()["traces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == key)
        .map(|t| t["strength"].as_f64().unwrap())
        .unwrap_or(0.0)
}

#[test]
fn agent_deposits_are_bounded_idempotent_and_quota_limited() {
    let (s, t) = fixture();
    let r = provision(&s, &t);
    let run = id(&r);
    let first = deposit(&s, run, "source.arxiv", "finding", "d1");
    near(first["strength"].as_f64().unwrap(), 2.0);
    assert_eq!(first["kind"], "finding");
    assert_eq!(first["deposits"], 1);
    // Same command key and payload replays the original receipt.
    assert_eq!(deposit(&s, run, "source.arxiv", "finding", "d1"), first);
    // Same command key with a different payload conflicts.
    assert_eq!(
        s.deposit_board_trace(
            run,
            &TraceInput { key: "source.arxiv".into(), kind: "path".into(), note: String::new() },
            "d1"
        )
        .unwrap_err()
        .1,
        "idempotency_conflict"
    );
    // Platform-only kinds and malformed keys are rejected.
    for kind in ["reuse", "completion", "accepted", "result"] {
        assert_eq!(
            s.deposit_board_trace(
                run,
                &TraceInput { key: "k".into(), kind: kind.into(), note: String::new() },
                "bad"
            )
            .unwrap_err()
            .1,
            "invalid_board_trace"
        );
    }
    // Eight deposits per Run, then quota.
    for i in 2..=8 {
        deposit(&s, run, &format!("trail{i}"), "path", &format!("d{i}"));
    }
    assert_eq!(
        s.deposit_board_trace(
            run,
            &TraceInput { key: "ninth".into(), kind: "path".into(), note: String::new() },
            "d9"
        )
        .unwrap_err()
        .1,
        "board_trace_quota"
    );
    let view = s.read_board_traces(run).unwrap();
    assert_eq!(view["root_id"], t["id"]);
    assert_eq!(view["traces"].as_array().unwrap().len(), 8);
    assert_eq!(view["half_life_ms"], traces::TRACE_HALF_LIFE_MS);
    // Sorted by strength descending; equal strengths keep a stable order.
    let strengths: Vec<f64> = view["traces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["strength"].as_f64().unwrap())
        .collect();
    assert!(strengths.windows(2).all(|w| w[0] >= w[1]));
}

#[test]
fn verified_events_reinforce_result_trails() {
    let (s, t) = fixture();
    let r = provision(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish(&s, &r, "completed");
    // Completion deposits a fixed weight on the task's result trail.
    near(strength(&s, id(&t), id(&t)), traces::TRACE_COMPLETION_AMOUNT);
    assert_eq!(
        s.board_task(id(&t)).unwrap()["trace"]["kind"],
        "result"
    );
    // Human acceptance is a stronger, verified reinforcement.
    s.review_board_task(
        id(&t),
        &ReviewInput {
            run_id: id(&r).into(),
            decision: ReviewDecision::Accepted,
            feedback: String::new(),
        },
        "accept",
    )
    .unwrap();
    near(
        strength(&s, id(&t), id(&t)),
        traces::TRACE_COMPLETION_AMOUNT + traces::TRACE_ACCEPTED_AMOUNT,
    );
    // Failed work deposits nothing.
    let other = s.publish_board_task(&input("other"), "other").unwrap();
    let bad = provision(&s, &other);
    s.claim_board_task(id(&bad), id(&other)).unwrap();
    finish(&s, &bad, "failed");
    near(strength(&s, id(&other), id(&other)), 0.0);
}

#[test]
fn peer_reuse_reinforces_once_per_run_and_never_self_reinforces() {
    let (s, t) = fixture();
    let parent = provision(&s, &t);
    s.claim_board_task(id(&parent), id(&t)).unwrap();
    let children = s
        .split_board_task(
            id(&parent),
            &SplitInput { tasks: vec![input("child")] },
            "split",
        )
        .unwrap();
    finish(&s, &parent, "completed");
    let child_id = children["task_ids"][0].as_str().unwrap();
    let child = s.board_task(child_id).unwrap();
    let peer = provision(&s, &child);
    let peer_run = id(&peer);
    // The parent's settled result is readable by the peer; reading it
    // reinforces the parent trail exactly once for this Run.
    let before = strength(&s, id(&t), id(&t));
    near(before, traces::TRACE_COMPLETION_AMOUNT);
    s.read_board_result(peer_run, id(&t), 0, 16000).unwrap();
    let after = strength(&s, id(&t), id(&t));
    near(after, before + traces::TRACE_REUSE_AMOUNT);
    s.read_board_result(peer_run, id(&t), 0, 16000).unwrap();
    near(strength(&s, id(&t), id(&t)), after);
    // Reading a task without a settled result deposits nothing.
    s.read_board_result(peer_run, child_id, 0, 16000).unwrap();
    near(strength(&s, id(&t), child_id), 0.0);
    // The deposit ledger records the reuse with its source Run.
    let events = s.events(0).unwrap();
    assert!(events.iter().any(|e| e["kind"] == "board.trace_deposited"
        && e["data"]["reason"] == "reuse"
        && e["data"]["source_run_id"] == peer_run));
}

#[test]
fn traces_are_scoped_to_the_root_task() {
    let (s, t) = fixture();
    let r = provision(&s, &t);
    deposit(&s, id(&r), "local.finding", "finding", "d1");
    let other = s.publish_board_task(&input("other-root"), "other-root").unwrap();
    let other_run = provision(&s, &other);
    let view = s.read_board_traces(id(&other_run)).unwrap();
    assert_eq!(view["root_id"], other["id"]);
    assert!(view["traces"].as_array().unwrap().is_empty());
    near(strength(&s, id(&t), "local.finding"), 2.0);
}

#[test]
fn strength_evaporates_lazily_and_reordering_follows_decay() {
    let path = std::env::temp_dir().join(format!("ant3-traces-{}.sqlite", uuid::Uuid::new_v4()));
    let s = Store::open(&path).unwrap();
    s.publish(Preset {
        name: "reader".into(),
        version: "1".into(),
        purpose: "Read".into(),
        objective: "Find evidence".into(),
        instructions: "Read evidence".into(),
        protocol: Protocol::Demo,
        plugin: None,
    })
    .unwrap();
    let t = s.publish_board_task(&input("root"), "root").unwrap();
    let r = provision(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    deposit(&s, id(&r), "old", "finding", "d1");
    deposit(&s, id(&r), "fresh", "finding", "d2");
    near(strength(&s, id(&t), "old"), 2.0);
    drop(s);
    // Age the first trail by one half-life directly in storage.
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute(
        "UPDATE board_traces SET updated_at=updated_at-? WHERE key='old'",
        [traces::TRACE_HALF_LIFE_MS],
    )
    .unwrap();
    drop(db);
    let s = Store::open(&path).unwrap();
    let view = s.board_traces(id(&t)).unwrap();
    let old = view["traces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["key"] == "old")
        .unwrap();
    near(old["strength"].as_f64().unwrap(), 1.0);
    assert_eq!(view["traces"][0]["key"], "fresh");
    // A fresh deposit evaporates the old strength first, then reinforces.
    let again = s
        .deposit_board_trace(
            id(&r),
            &TraceInput { key: "old".into(), kind: "finding".into(), note: String::new() },
            "d3",
        )
        .unwrap();
    near(again["strength"].as_f64().unwrap(), 3.0);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn trace_field_capacity_is_bounded_for_agent_keys() {
    let path = std::env::temp_dir().join(format!("ant3-trace-cap-{}.sqlite", uuid::Uuid::new_v4()));
    let s = Store::open(&path).unwrap();
    s.publish(Preset {
        name: "reader".into(),
        version: "1".into(),
        purpose: "Read".into(),
        objective: "Find evidence".into(),
        instructions: "Read evidence".into(),
        protocol: Protocol::Demo,
        plugin: None,
    })
    .unwrap();
    let t = s.publish_board_task(&input("root"), "root").unwrap();
    let r = provision(&s, &t);
    drop(s);
    let db = rusqlite::Connection::open(&path).unwrap();
    for i in 0..traces::TRACES_PER_ROOT {
        db.execute(
            "INSERT INTO board_traces(root_id,key,kind,strength,deposits,created_at,updated_at) VALUES(?,?,?,1.0,1,0,0)",
            rusqlite::params![id(&t), format!("filler{i}"), "finding"],
        )
        .unwrap();
    }
    drop(db);
    let s = Store::open(&path).unwrap();
    assert_eq!(
        s.deposit_board_trace(
            id(&r),
            &TraceInput { key: "overflow".into(), kind: "finding".into(), note: String::new() },
            "d1"
        )
        .unwrap_err()
        .1,
        "board_trace_capacity"
    );
    // Existing keys can still be reinforced at capacity.
    assert!(
        s.deposit_board_trace(
            id(&r),
            &TraceInput { key: "filler0".into(), kind: "finding".into(), note: String::new() },
            "d2"
        )
        .is_ok()
    );
    std::fs::remove_file(path).unwrap();
}
#[test]
fn successful_run_recruits_its_signposted_paths_once() {
    let (s, t) = fixture();
    let r = provision(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    deposit(&s, id(&r), "route.arxiv", "path", "d1");
    near(strength(&s, id(&t), "route.arxiv"), 2.0);
    finish(&s, &r, "completed");
    // Success amplifies the path this Run signposted: agent 2 + recruitment 2.
    near(
        strength(&s, id(&t), "route.arxiv"),
        traces::TRACE_AGENT_AMOUNT + traces::TRACE_RECRUITMENT_AMOUNT,
    );
    // Reconciliation is idempotent; no double recruitment.
    s.reconcile_board().unwrap();
    near(
        strength(&s, id(&t), "route.arxiv"),
        traces::TRACE_AGENT_AMOUNT + traces::TRACE_RECRUITMENT_AMOUNT,
    );
    let events = s.events(0).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e["kind"] == "board.trace_deposited"
                && e["data"]["reason"] == "recruitment"
                && e["data"]["key"] == "route.arxiv")
            .count(),
        1
    );
    // Failed Runs recruit nothing.
    let other = s.publish_board_task(&input("other"), "other").unwrap();
    let bad = provision(&s, &other);
    s.claim_board_task(id(&bad), id(&other)).unwrap();
    deposit(&s, id(&bad), "route.deadend", "path", "d1");
    finish(&s, &bad, "failed");
    near(strength(&s, id(&other), "route.deadend"), traces::TRACE_AGENT_AMOUNT);
}

#[test]
fn pending_offering_prefers_stronger_trails_then_fifo() {
    let path = std::env::temp_dir().join(format!("ant3-trace-offer-{}.sqlite", uuid::Uuid::new_v4()));
    let s = Store::open(&path).unwrap();
    s.publish(Preset {
        name: "reader".into(),
        version: "1".into(),
        purpose: "Read".into(),
        objective: "Find evidence".into(),
        instructions: "Read evidence".into(),
        protocol: Protocol::Demo,
        plugin: None,
    })
    .unwrap();
    let a = s.publish_board_task(&input("a"), "a").unwrap();
    let b = s.publish_board_task(&input("b"), "b").unwrap();
    // FIFO without traces.
    let pending = s.board_pending().unwrap();
    assert_eq!(pending[0]["id"], a["id"]);
    assert_eq!(pending[1]["id"], b["id"]);
    // Signpost a trail on task A, then return it to the unassigned pool.
    let r = provision(&s, &a);
    deposit(&s, id(&r), id(&a), "finding", "d1");
    drop(s);
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute(
        "UPDATE board_tasks SET route_state='unassigned' WHERE id=?",
        [id(&a)],
    )
    .unwrap();
    drop(db);
    let s = Store::open(&path).unwrap();
    let pending = s.board_pending().unwrap();
    assert_eq!(pending[0]["id"], a["id"]);
    assert_eq!(pending[1]["id"], b["id"]);
    std::fs::remove_file(path).unwrap();
}
