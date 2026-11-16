use ant3_platform::{
    domain::*,
    store::Store,
    workspaces::{PublishInput, WriteInput},
};

use serde_json::{Value, json};
use std::path::PathBuf;
struct Fixture {
    store: Store,
    root: PathBuf,
    runs: Vec<Value>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn fixture() -> Fixture {
    let store = Store::memory().unwrap();
    let root = std::env::temp_dir().join(format!("h3-workspaces-{}", uuid::Uuid::new_v4()));
    store.set_workspace_root(&root).unwrap();
    store.publish_definition(json!({"name":"writer","version":"1","purpose":"files","objective":"share evidence","instructions":"write and publish files","protocol":"demo","plugins":[{"id":"platform.workspace","version":"1.0.0"}]})).unwrap();
    let instances = store
        .spawn(
            &Spawn {
                preset_name: "writer".into(),
                preset_version: "1".into(),
                count: 2,
            },
            "agents",
        )
        .unwrap();
    for (i, instance) in instances["instance_ids"].as_array().unwrap().iter().enumerate() {
        store.enqueue(instance.as_str().unwrap(), &RunInput{text:"write evidence".into()}, &format!("run-{i}")).unwrap();
    }
    let runs = vec![
        store.claim().unwrap().unwrap(),
        store.claim().unwrap().unwrap(),
    ];
    // Both peers join one board root: snapshot sharing is scoped to shared
    // board work now that Goal teams are gone.
    let task = store
        .publish_board_task(
            &ant3_platform::board::PublishInput {
                title: "evidence".into(),
                instructions: "share files".into(),
                acceptance: "snapshot readable".into(),
                verification: None,
                energy_budget: None,
            },
            "ws-root",
        )
        .unwrap();
    store
        .transaction(|tx| {
            for run in &runs {
                tx.execute(
                    "INSERT INTO board_agents(instance_id,root_id,task_id,run_id,created_at) VALUES(?,?,?,?,0)",
                    rusqlite::params![
                        run["instance_id"].as_str().unwrap(),
                        task["id"].as_str().unwrap(),
                        task["id"].as_str().unwrap(),
                        run["id"].as_str().unwrap()
                    ],
                )?;
            }
            Ok(())
        })
        .unwrap();
    Fixture { store, root, runs }
}
fn rid(run: &Value) -> &str {
    run["id"].as_str().unwrap()
}
#[test]
fn one_agent_writes_and_another_reads_an_immutable_file_snapshot() {
    let f = fixture();
    let a = rid(&f.runs[0]);
    let b = rid(&f.runs[1]);
    let write = WriteInput {
        path: "research/evidence.md".into(),
        content: "证据甲🙂\nsource: supplied material".into(),
    };
    let saved = f.store.workspace_write(a, &write, "write").unwrap();
    assert_eq!(f.store.workspace_write(a, &write, "write").unwrap(), saved);
    let listing = f.store.run_workspace_files(a, "research").unwrap();
    assert_eq!(listing["entries"][0]["path"], write.path);
    let publication = PublishInput {
        name: "research evidence".into(),
        path: Some(write.path.clone()),
        value: None,
    };
    let artifact = f
        .store
        .publish_workspace_artifact(a, &publication, "publish")
        .unwrap();
    assert_eq!(artifact["source_state"], "running");
    assert_eq!(
        f.store
            .read_workspace_artifact(b, artifact["id"].as_str().unwrap())
            .unwrap()["content"],
        write.content
    );
    assert!(f.store.run_workspace_read(b, &write.path).is_err());
    f.store
        .workspace_write(
            a,
            &WriteInput {
                path: write.path.clone(),
                content: "revised live file".into(),
            },
            "edit",
        )
        .unwrap();
    assert_eq!(
        f.store
            .publish_workspace_artifact(a, &publication, "publish")
            .unwrap()["id"],
        artifact["id"]
    );
    assert_eq!(
        f.store
            .workspace_artifact(artifact["id"].as_str().unwrap())
            .unwrap()["content"],
        write.content
    );
    f.store
        .finish(
            a,
            f.runs[0]["instance_id"].as_str().unwrap(),
            "completed",
            json!({"output":"published evidence"}),
        )
        .unwrap();
    assert_eq!(
        f.store
            .read_workspace_artifact(b, artifact["id"].as_str().unwrap())
            .unwrap()["source_state"],
        "completed"
    );
    assert_eq!(
        f.store.workspace_artifacts().unwrap()["artifacts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn structured_json_publication_keeps_value_and_rejects_mutating_the_receipt() {
    let f = fixture();
    let a = rid(&f.runs[0]);
    let input = PublishInput {
        name: "structured findings".into(),
        path: None,
        value: Some(json!({"answer":42,"uncertainties":["source missing"]})),
    };
    let p = f
        .store
        .publish_workspace_artifact(a, &input, "json")
        .unwrap();
    let read = f
        .store
        .read_workspace_artifact(rid(&f.runs[1]), p["id"].as_str().unwrap())
        .unwrap();
    assert_eq!(read["value"], input.value.unwrap());
    assert_eq!(read["kind"], "json");
    assert_eq!(read["integrity_verified"], true);
    let changed = PublishInput {
        name: "different".into(),
        path: None,
        value: Some(json!({"answer":0})),
    };
    assert_eq!(
        f.store
            .publish_workspace_artifact(a, &changed, "json")
            .unwrap_err()
            .1,
        "idempotency_conflict"
    );
}
#[test]
fn file_paths_stay_relative_and_cancelled_runs_cannot_write() {
    let f = fixture();
    let a = rid(&f.runs[0]);
    for path in [
        "../escape.txt",
        "/tmp/escape.txt",
        "nested/../../escape.txt",
    ] {
        assert_eq!(
            f.store
                .workspace_write(
                    a,
                    &WriteInput {
                        path: path.into(),
                        content: "bad".into()
                    },
                    path
                )
                .unwrap_err()
                .1,
            "invalid_workspace_path"
        );
    }
    f.store.cancel(a).unwrap();
    assert_eq!(
        f.store
            .workspace_write(
                a,
                &WriteInput {
                    path: "late.txt".into(),
                    content: "late".into()
                },
                "late"
            )
            .unwrap_err()
            .1,
        "workspace_run_not_authorized"
    );
}