//! Emergence evaluation baseline: single Agent vs peer swarm on the same
//! task, acceptance and deterministic execution contract.
//!
//! Run with `cargo test -p ant3-server --test evaluation -- --nocapture` to
//! print the JSON comparison report. The harness does not presume the swarm
//! is better; it reports quality, cost proxies and information-reuse metrics
//! so regressions and emergence claims stay measurable. Real-model, equal-
//! token-budget evaluation builds on this contract once usage accounting lands.
use ant3_platform::{
    board::{PublishInput, ReviewDecision, ReviewInput, SplitInput},
    domain::*,
    store::Store,
};
use serde_json::{Value, json};
use std::time::Instant;

const CHILDREN: usize = 3;

fn preset() -> Preset {
    Preset {
        name: "reader".into(),
        version: "1".into(),
        purpose: "Read evidence".into(),
        objective: "Report findings".into(),
        instructions: "Inspect supplied evidence".into(),
        protocol: Protocol::Demo,
        plugin: None,
    }
}
fn root_input() -> PublishInput {
    PublishInput {
        title: "Evidence synthesis".into(),
        instructions: "Collect three independent evidence summaries and integrate them.".into(),
        acceptance: "Integrated report cites every child contribution".into(),
    }
}
fn child_input(i: usize) -> PublishInput {
    PublishInput {
        title: format!("Evidence part {i}"),
        instructions: format!("Summarize evidence source {i}."),
        acceptance: "Summary reports source content".into(),
    }
}
fn id(v: &Value) -> &str {
    v["id"].as_str().unwrap()
}
fn provision(s: &Store, t: &Value) -> Value {
    s.provision_board_task(id(t), "reader", "1", "evaluation fixture")
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
fn accept(s: &Store, t: &Value, run: &str) {
    s.review_board_task(
        id(t),
        &ReviewInput {
            run_id: run.into(),
            decision: ReviewDecision::Accepted,
            feedback: String::new(),
        },
        "eval-accept",
    )
    .unwrap();
}

#[derive(Default)]
struct Metrics {
    completed: bool,
    accepted: bool,
    runs: usize,
    claims: usize,
    splits: usize,
    reuse_deposits: usize,
    root_trace_strength: f64,
    wall_ms: u128,
}
fn measure(s: &Store, root: &Value, started: Instant) -> Metrics {
    let mut m = Metrics {
        wall_ms: started.elapsed().as_millis(),
        ..Default::default()
    };
    let task = s.board_task(id(root)).unwrap();
    m.completed = task["state"] == "completed";
    m.accepted = task["review_state"] == "accepted";
    m.runs = s.list("runs").unwrap().len();
    m.root_trace_strength = task["trace"]["strength"].as_f64().unwrap_or(0.0);
    let mut cursor = 0;
    loop {
        let page = s.events(cursor).unwrap();
        if page.is_empty() {
            break;
        }
        cursor = page.last().unwrap()["seq"].as_i64().unwrap();
        for e in page {
            match e["kind"].as_str().unwrap() {
                "board.task_claimed" => m.claims += 1,
                "board.task_split" => m.splits += 1,
                "board.trace_deposited" if e["data"]["reason"] == "reuse" => {
                    m.reuse_deposits += 1
                }
                _ => {}
            }
        }
    }
    m
}
fn report(arm: &str, m: &Metrics) -> Value {
    json!({
        "arm": arm,
        "completed": m.completed,
        "accepted": m.accepted,
        "runs": m.runs,
        "claims": m.claims,
        "splits": m.splits,
        "reuse_deposits": m.reuse_deposits,
        "root_trace_strength": m.root_trace_strength,
        "wall_ms": m.wall_ms,
    })
}

/// One Agent does the whole task in a single Run: no split, no peer results.
#[test]
fn single_agent_baseline_completes_without_information_reuse() {
    let s = Store::memory().unwrap();
    s.publish(preset()).unwrap();
    let started = Instant::now();
    let root = s.publish_board_task(&root_input(), "root").unwrap();
    let run = provision(&s, &root);
    s.claim_board_task(id(&run), id(&root)).unwrap();
    finish(&s, &run, "Integrated report from a single Agent.");
    accept(&s, &root, id(&run));
    let m = measure(&s, &root, started);
    eprintln!("EVAL {}", report("single", &m));
    assert!(m.completed && m.accepted);
    assert_eq!(m.runs, 1);
    assert_eq!(m.splits, 0);
    assert_eq!(m.reuse_deposits, 0);
    // Completion + acceptance still reinforce the result trail.
    assert!(m.root_trace_strength >= traces::TRACE_COMPLETION_AMOUNT + traces::TRACE_ACCEPTED_AMOUNT - 1e-3);
}

/// The swarm splits, peers execute children, and a continuation integrates
/// by reading peer results through the board — reuse must leave traces.
#[test]
fn peer_swarm_integrates_through_reused_trails() {
    let s = Store::memory().unwrap();
    s.publish(preset()).unwrap();
    let started = Instant::now();
    let root = s.publish_board_task(&root_input(), "root").unwrap();
    let splitter = provision(&s, &root);
    s.claim_board_task(id(&splitter), id(&root)).unwrap();
    let children = s
        .split_board_task(
            id(&splitter),
            &SplitInput {
                tasks: (0..CHILDREN).map(child_input).collect(),
            },
            "eval-split",
        )
        .unwrap();
    finish(&s, &splitter, "Split into independent evidence work.");
    let mut child_ids = vec![];
    for child in children["task_ids"].as_array().unwrap() {
        let child_id = child.as_str().unwrap().to_string();
        let child_task = s.board_task(&child_id).unwrap();
        let run = provision(&s, &child_task);
        s.claim_board_task(id(&run), &child_id).unwrap();
        finish(&s, &run, &format!("Evidence summary for {child_id}."));
        child_ids.push(child_id);
    }
    // Continuation: an ordinary peer integrates by reading child results.
    let root_task = s.board_task(id(&root)).unwrap();
    assert_eq!(root_task["state"], "pending");
    let integrator = provision(&s, &root_task);
    s.claim_board_task(id(&integrator), id(&root)).unwrap();
    for child_id in &child_ids {
        let page = s.read_board_result(id(&integrator), child_id, 0, 16000).unwrap();
        assert!(!page["content"].as_str().unwrap().is_empty());
    }
    finish(&s, &integrator, "Integrated report citing all child contributions.");
    accept(&s, &root, id(&integrator));
    let m = measure(&s, &root, started);
    eprintln!("EVAL {}", report("swarm", &m));
    assert!(m.completed && m.accepted);
    assert_eq!(m.runs, CHILDREN + 2);
    assert_eq!(m.splits, 1);
    // Every child contribution was reused through the environment exactly once.
    assert_eq!(m.reuse_deposits, CHILDREN);
    // Each child trail carries completion + reuse; the root carries the
    // splitter completion, integrator completion and acceptance.
    for child_id in &child_ids {
        let strength = s.board_traces(id(&root)).unwrap()["traces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["key"] == child_id.as_str())
            .map(|t| t["strength"].as_f64().unwrap())
            .unwrap_or(0.0);
        assert!(strength >= traces::TRACE_COMPLETION_AMOUNT + traces::TRACE_REUSE_AMOUNT - 1e-3);
    }
}

/// Side-by-side report under identical task and acceptance. Cost proxies are
/// reported, not optimized: the swarm trades more Runs for parallel evidence
/// and measurable information reuse. No arm is presumed superior.
#[test]
fn equal_task_comparison_report() {
    let solo = {
        let s = Store::memory().unwrap();
        s.publish(preset()).unwrap();
        let started = Instant::now();
        let root = s.publish_board_task(&root_input(), "root").unwrap();
        let run = provision(&s, &root);
        s.claim_board_task(id(&run), id(&root)).unwrap();
        finish(&s, &run, "Integrated report from a single Agent.");
        accept(&s, &root, id(&run));
        measure(&s, &root, started)
    };
    let swarm = {
        let s = Store::memory().unwrap();
        s.publish(preset()).unwrap();
        let started = Instant::now();
        let root = s.publish_board_task(&root_input(), "root").unwrap();
        let splitter = provision(&s, &root);
        s.claim_board_task(id(&splitter), id(&root)).unwrap();
        let children = s
            .split_board_task(
                id(&splitter),
                &SplitInput { tasks: (0..CHILDREN).map(child_input).collect() },
                "eval-split",
            )
            .unwrap();
        finish(&s, &splitter, "Split into independent evidence work.");
        let mut child_ids = vec![];
        for child in children["task_ids"].as_array().unwrap() {
            let child_id = child.as_str().unwrap().to_string();
            let child_task = s.board_task(&child_id).unwrap();
            let run = provision(&s, &child_task);
            s.claim_board_task(id(&run), &child_id).unwrap();
            finish(&s, &run, &format!("Evidence summary for {child_id}."));
            child_ids.push(child_id);
        }
        let root_task = s.board_task(id(&root)).unwrap();
        let integrator = provision(&s, &root_task);
        s.claim_board_task(id(&integrator), id(&root)).unwrap();
        for child_id in &child_ids {
            s.read_board_result(id(&integrator), child_id, 0, 16000).unwrap();
        }
        finish(&s, &integrator, "Integrated report citing all child contributions.");
        accept(&s, &root, id(&integrator));
        measure(&s, &root, started)
    };
    let comparison = json!({
        "task": "evidence-synthesis",
        "acceptance": "identical",
        "children": CHILDREN,
        "single": report("single", &solo),
        "swarm": report("swarm", &swarm),
        "emergence": {
            "information_reuse": swarm.reuse_deposits > solo.reuse_deposits,
            "reuse_per_child": swarm.reuse_deposits as f64 / CHILDREN as f64,
            "run_cost_ratio": swarm.runs as f64 / solo.runs as f64,
        },
        "notes": "Deterministic contract baseline; real-model equal-budget evaluation pending usage accounting. Individual-loss robustness not yet supported: failed children block the root.",
    });
    eprintln!("EVAL_COMPARISON {comparison}");
    assert!(solo.completed && solo.accepted);
    assert!(swarm.completed && swarm.accepted);
    assert!(swarm.reuse_deposits > solo.reuse_deposits);
}