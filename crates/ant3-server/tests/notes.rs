//! Root blackboard: CAS writes, evidence-backed conflict adjudication.
use ant3_platform::{
    board::{PublishInput, TraceInput},
    domain::*,
    notes::NoteInput,
    store::Store,
};
use serde_json::Value;

fn preset() -> Preset {
    Preset { name: "worker".into(), version: "1".into(), purpose: "p".into(), objective: "o".into(), instructions: "i".into(), protocol: Protocol::Demo, plugin: None }
}
fn fixture() -> (Store, Value) {
    let s = Store::memory().unwrap();
    s.publish(preset()).unwrap();
    let t = s.publish_board_task(&PublishInput { title: "root".into(), instructions: "i".into(), acceptance: "a".into(), verification: None, energy_budget: None }, "root").unwrap();
    (s, t)
}
fn id(v: &Value) -> &str { v["id"].as_str().unwrap() }
fn work(s: &Store, t: &Value) -> Value {
    s.provision_board_task(id(t), "worker", "1", "f").unwrap();
    let run = s.claim().unwrap().unwrap();
    s.claim_board_task(id(&run), id(t)).unwrap();
    run
}
fn peer(s: &Store, root: &Value) -> Value {
    // A recruited peer: board membership without a routed invitation.
    let batch = s.spawn(&Spawn { preset_name: "worker".into(), preset_version: "1".into(), count: 1 }, &format!("peer-{}", uuid::Uuid::new_v4())).unwrap();
    let agent = batch["instance_ids"][0].as_str().unwrap().to_owned();
    let run = s.enqueue(&agent, &RunInput { text: "peer work".into() }, &format!("peer-run-{}", uuid::Uuid::new_v4())).unwrap();
    let claimed = s.claim().unwrap().unwrap();
    s.transaction(|tx| {
        tx.execute("INSERT INTO board_agents(instance_id,root_id,task_id,run_id,created_at) VALUES(?,?,?,?,0)",
            rusqlite::params![agent, id(root), id(root), id(&run)])?;
        Ok(())
    }).unwrap();
    claimed
}
fn note(key: &str, body: &str, rev: i64, support: Vec<&str>) -> NoteInput {
    NoteInput { key: key.into(), body: body.into(), expected_revision: rev, support: support.into_iter().map(String::from).collect() }
}

#[test]
fn cas_create_and_update_keep_superseded_history() {
    let (s, t) = fixture();
    let r = work(&s, &t);
    let created = s.write_board_note(id(&r), &note("finding.speed", "10ms", 0, vec![]), "n1").unwrap();
    assert_eq!(created["adjudication"], "created");
    assert_eq!(created["revision"], 1);
    // Idempotent replay.
    assert_eq!(s.write_board_note(id(&r), &note("finding.speed", "10ms", 0, vec![]), "n1").unwrap(), created);
    let updated = s.write_board_note(id(&r), &note("finding.speed", "12ms after cache", 1, vec![]), "n2").unwrap();
    assert_eq!(updated["adjudication"], "cas_update");
    assert_eq!(updated["revision"], 2);
    let view = s.board_notes(id(&t)).unwrap();
    let notes = view["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0]["revision"], 2);
    assert_eq!(notes[0]["status"], "active");
    assert_eq!(notes[1]["status"], "superseded");
    assert_eq!(notes[1]["superseded_by"], 2);
}

#[test]
fn conflicts_are_adjudicated_by_trace_backed_evidence() {
    let (s, t) = fixture();
    let r1 = work(&s, &t);
    // Weakly supported incumbent: no trace backing.
    s.write_board_note(id(&r1), &note("finding.root_cause", "network", 0, vec![]), "n1").unwrap();
    // Challenger deposits a strong trace and cites it.
    let r2 = peer(&s, &t);
    s.deposit_board_trace(id(&r2), &TraceInput { key: "evidence.profile".into(), kind: "finding".into(), note: String::new() }, "t1").unwrap();
    let won = s.write_board_note(id(&r2), &note("finding.root_cause", "lock contention", 0, vec!["evidence.profile"]), "n2").unwrap();
    assert_eq!(won["adjudication"], "superseded_incumbent");
    assert_eq!(won["status"], "active");
    assert_eq!(won["revision"], 2);
    // Weaker challenger loses; recorded as superseded history.
    let r3 = peer(&s, &t);
    let lost = s.write_board_note(id(&r3), &note("finding.root_cause", "disk", 0, vec![]), "n3").unwrap();
    assert_eq!(lost["adjudication"], "incumbent_kept");
    assert_eq!(lost["status"], "superseded");
    assert_eq!(lost["active_revision"], 2);
    // Tie keeps the incumbent (first-writer-wins).
    let r4 = peer(&s, &t);
    s.deposit_board_trace(id(&r4), &TraceInput { key: "evidence.profile2".into(), kind: "finding".into(), note: String::new() }, "t2").unwrap();
    let tie = s.write_board_note(id(&r4), &note("finding.root_cause", "memory", 0, vec!["evidence.profile2"]), "n4").unwrap();
    // Equal decayed support: first-writer-wins keeps the incumbent.
    assert_eq!(tie["adjudication"], "incumbent_kept", "tie={tie} incumbent_score_expected=2");
    // Adjudication events are auditable.
    let events = s.events(0).unwrap();
    let adjudicated: Vec<_> = events.iter().filter(|e| e["kind"] == "board.note_adjudicated").collect();
    assert_eq!(adjudicated.len(), 3);
    assert_eq!(adjudicated[0]["data"]["rule"], "stronger_evidence");
    assert_eq!(adjudicated[2]["data"]["rule"], "tie_first_writer_wins");
}

#[test]
fn note_writes_are_quota_limited_and_scoped_to_the_root() {
    let (s, t) = fixture();
    let r = work(&s, &t);
    for i in 0..notes::NOTE_WRITES_PER_RUN {
        s.write_board_note(id(&r), &note(&format!("k{i}"), "body", 0, vec![]), &format!("n{i}")).unwrap();
    }
    assert_eq!(
        s.write_board_note(id(&r), &note("overflow", "body", 0, vec![]), "nx").unwrap_err().1,
        "board_note_quota"
    );
    // Another root cannot see these notes.
    let other = s.publish_board_task(&PublishInput { title: "other".into(), instructions: "i".into(), acceptance: "a".into(), verification: None, energy_budget: None }, "other").unwrap();
    assert!(s.board_notes(id(&other)).unwrap()["notes"].as_array().unwrap().is_empty());
    // Wrong expected revision on create conflicts.
    let r2 = peer(&s, &other);
    assert_eq!(
        s.write_board_note(id(&r2), &note("k", "body", 5, vec![]), "n0").unwrap_err().1,
        "note_revision_conflict"
    );
}
