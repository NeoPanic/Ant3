//! Independent board audits: fresh reviewer, frozen evidence, auto-accept on
//! passed, bounded rework on failed, unknown never passes, humans keep say.
use ant3_platform::{
    board::{BoardVerification, PublishInput, ReviewDecision, ReviewInput},
    domain::*,
    store::Store,
};
use serde_json::{Value, json};

fn preset(name: &str) -> Preset {
    Preset {
        name: name.into(),
        version: "1".into(),
        purpose: "Inspect evidence".into(),
        objective: "Report or judge findings".into(),
        instructions: "Use available tools; cite evidence".into(),
        protocol: Protocol::Demo,
        plugin: None,
    }
}
fn input(title: &str, verification: Option<BoardVerification>) -> PublishInput {
    PublishInput {
        title: title.into(),
        instructions: format!("Inspect {title}"),
        acceptance: "Evidence is reported with sources".into(),
        verification,
    }
}
fn policy() -> BoardVerification {
    BoardVerification {
        preset_name: "auditor".into(),
        preset_version: "1".into(),
        max_auto_reworks: 1,
    }
}
fn fixture(verification: Option<BoardVerification>) -> (Store, Value) {
    let s = Store::memory().unwrap();
    s.publish(preset("worker")).unwrap();
    s.publish(preset("auditor")).unwrap();
    let t = s
        .publish_board_task(&input("root", verification), "root")
        .unwrap();
    (s, t)
}
fn id(v: &Value) -> &str {
    v["id"].as_str().unwrap()
}
fn work(s: &Store, t: &Value) -> Value {
    s.provision_board_task(id(t), "worker", "1", "fixture")
        .unwrap();
    s.claim().unwrap().unwrap()
}
fn finish(s: &Store, r: &Value, output: &str) {
    s.finish(
        id(r),
        r["instance_id"].as_str().unwrap(),
        "completed",
        json!({"output":output}),
    )
    .unwrap();
    s.reconcile_board().unwrap();
}
/// Drive the pending audit to a dispatched reviewer Run.
fn dispatch_audit(s: &Store) -> Value {
    // First sweep allocates the reviewer fleet; instances become ready on
    // the next reconcile, then the audit Run is queued.
    for _ in 0..4 {
        s.reconcile_fleets().unwrap();
        s.reconcile_board().unwrap();
        if let Some(run) = s.claim().unwrap() {
            return run;
        }
    }
    panic!("auditor run never queued");
}
fn read_all(s: &Store, run: &str) {
    let mut offset = 0;
    loop {
        let page = s.board_audit_evidence(run, offset, 12000).unwrap();
        offset = page["next_offset"].as_u64().unwrap() as usize;
        if page["complete"].as_bool().unwrap() {
            break;
        }
    }
}

#[test]
fn passed_audit_auto_accepts_with_verifier_attribution() {
    let (s, t) = fixture(Some(policy()));
    let r = work(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish(&s, &r, "Evidence with sources.");
    // Root completed and an audit opened; not yet human-accepted.
    let task = s.board_task(id(&t)).unwrap();
    assert_eq!(task["state"], "completed");
    assert_eq!(task["review_state"], "pending");
    assert_eq!(task["audits"].as_array().unwrap().len(), 1);
    assert_eq!(task["audits"][0]["state"], "pending");
    // A fresh reviewer instance is dispatched — never the producing worker.
    let audit_run = dispatch_audit(&s);
    assert_ne!(audit_run["instance_id"], r["instance_id"]);
    let job = s.board_audit_job(id(&audit_run)).unwrap().unwrap();
    assert_eq!(job["state"], "running");
    // Passed requires complete evidence reading.
    assert_eq!(
        s.submit_board_audit(id(&audit_run), &json!({"status":"passed","reason":"criteria met"}))
            .unwrap_err()
            .1,
        "audit_evidence_incomplete"
    );
    read_all(&s, id(&audit_run));
    s.submit_board_audit(id(&audit_run), &json!({"status":"passed","reason":"criteria met"}))
        .unwrap();
    s.finish(
        id(&audit_run),
        audit_run["instance_id"].as_str().unwrap(),
        "completed",
        json!({"output":"verdict submitted"}),
    )
    .unwrap();
    s.reconcile_board().unwrap();
    let task = s.board_task(id(&t)).unwrap();
    assert_eq!(task["review_state"], "accepted");
    assert_eq!(task["audits"][0]["state"], "passed");
    let review = &task["reviews"][0];
    assert_eq!(review["decision"], "accepted");
    assert_eq!(review["author_kind"], "verifier");
    // Acceptance deposit landed on the trail.
    let strength = task["trace"]["strength"].as_f64().unwrap();
    assert!(strength >= traces::TRACE_COMPLETION_AMOUNT + traces::TRACE_ACCEPTED_AMOUNT - 1e-3);
}

#[test]
fn failed_audit_requeues_once_then_falls_back_to_human_review() {
    let (s, t) = fixture(Some(policy()));
    let r = work(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish(&s, &r, "Weak evidence.");
    let audit_run = dispatch_audit(&s);
    read_all(&s, id(&audit_run));
    s.submit_board_audit(id(&audit_run), &json!({"status":"failed","reason":"missing sources"}))
        .unwrap();
    s.finish(id(&audit_run), audit_run["instance_id"].as_str().unwrap(), "completed", json!({"output":"done"})).unwrap();
    s.reconcile_board().unwrap();
    // Rework: task reopened with verifier feedback attached.
    let task = s.board_task(id(&t)).unwrap();
    assert_eq!(task["state"], "pending");
    assert_eq!(task["review_state"], "changes_requested");
    assert_eq!(task["reviews"][0]["author_kind"], "verifier");
    assert!(task["reviews"][0]["feedback"].as_str().unwrap().contains("missing sources"));
    assert_eq!(task["audits"][0]["state"], "changes-requested");
    // Second attempt fails again: budget (1) exhausted, human review resumes.
    let r2 = work(&s, &task);
    s.claim_board_task(id(&r2), id(&t)).unwrap();
    finish(&s, &r2, "Still weak.");
    let audit_run2 = dispatch_audit(&s);
    read_all(&s, id(&audit_run2));
    s.submit_board_audit(id(&audit_run2), &json!({"status":"failed","reason":"still missing"}))
        .unwrap();
    s.finish(id(&audit_run2), audit_run2["instance_id"].as_str().unwrap(), "completed", json!({"output":"done"})).unwrap();
    s.reconcile_board().unwrap();
    let task = s.board_task(id(&t)).unwrap();
    assert_eq!(task["state"], "completed");
    assert_eq!(task["review_state"], "pending");
    // Human can still accept.
    s.review_board_task(
        id(&t),
        &ReviewInput { run_id: id(&r2).into(), decision: ReviewDecision::Accepted, feedback: String::new() },
        "human-final",
    )
    .unwrap();
    assert_eq!(s.board_task(id(&t)).unwrap()["review_state"], "accepted");
}

#[test]
fn unknown_verdict_never_passes_and_keeps_human_review() {
    let (s, t) = fixture(Some(policy()));
    let r = work(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish(&s, &r, "Ambiguous evidence.");
    let audit_run = dispatch_audit(&s);
    // Unknown does not require complete reads.
    s.submit_board_audit(id(&audit_run), &json!({"status":"unknown","reason":"cannot establish sources"}))
        .unwrap();
    s.finish(id(&audit_run), audit_run["instance_id"].as_str().unwrap(), "completed", json!({"output":"done"})).unwrap();
    s.reconcile_board().unwrap();
    let task = s.board_task(id(&t)).unwrap();
    assert_eq!(task["state"], "completed");
    assert_eq!(task["review_state"], "pending");
    assert_eq!(task["audits"][0]["state"], "inconclusive");
}

#[test]
fn incomplete_auditor_run_is_a_failed_audit_not_an_acceptance() {
    let (s, t) = fixture(Some(policy()));
    let r = work(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish(&s, &r, "Evidence.");
    let audit_run = dispatch_audit(&s);
    // Reviewer fails without submitting a verdict.
    s.finish(id(&audit_run), audit_run["instance_id"].as_str().unwrap(), "failed", json!({"error":{"code":"model_error"}})).unwrap();
    s.reconcile_board().unwrap();
    let task = s.board_task(id(&t)).unwrap();
    assert_eq!(task["review_state"], "pending");
    assert_eq!(task["audits"][0]["state"], "failed");
    assert_eq!(task["audits"][0]["error"], "audit_run_incomplete");
}

#[test]
fn tasks_without_policy_keep_the_human_only_flow() {
    let (s, t) = fixture(None);
    let r = work(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish(&s, &r, "Evidence.");
    let task = s.board_task(id(&t)).unwrap();
    assert_eq!(task["review_state"], "pending");
    assert!(task["audits"].as_array().unwrap().is_empty());
    assert!(task["verification"].is_null());
    // No auditor run is queued.
    s.reconcile_board().unwrap();
    assert!(s.claim().unwrap().is_none());
}

#[test]
fn verdicts_are_immutable_and_evidence_is_scoped() {
    let (s, t) = fixture(Some(policy()));
    let r = work(&s, &t);
    s.claim_board_task(id(&r), id(&t)).unwrap();
    finish(&s, &r, "Evidence.");
    let audit_run = dispatch_audit(&s);
    read_all(&s, id(&audit_run));
    let verdict = json!({"status":"passed","reason":"criteria met"});
    s.submit_board_audit(id(&audit_run), &verdict).unwrap();
    // Same verdict replays; a different one conflicts.
    s.submit_board_audit(id(&audit_run), &verdict).unwrap();
    assert_eq!(
        s.submit_board_audit(id(&audit_run), &json!({"status":"failed","reason":"changed mind"}))
            .unwrap_err()
            .1,
        "audit_verdict_immutable"
    );
    // The producing worker's Run cannot read audit evidence or submit.
    assert_eq!(
        s.board_audit_evidence(id(&r), 0, 100).unwrap_err().1,
        "audit_run_not_authorized"
    );
    assert!(s.board_audit_job(id(&r)).unwrap().is_none());
}

#[test]
fn invalid_policies_and_verdicts_are_rejected() {
    let s = Store::memory().unwrap();
    s.publish(preset("worker")).unwrap();
    assert_eq!(
        s.publish_board_task(
            &input("bad", Some(BoardVerification { preset_name: "no such!".into(), preset_version: "1".into(), max_auto_reworks: 0 })),
            "bad"
        )
        .unwrap_err()
        .1,
        "invalid_board_verification"
    );
    assert_eq!(
        s.publish_board_task(
            &input("bad2", Some(BoardVerification { preset_name: "auditor".into(), preset_version: "1".into(), max_auto_reworks: 3 })),
            "bad2"
        )
        .unwrap_err()
        .1,
        "invalid_board_verification"
    );
}