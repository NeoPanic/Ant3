use super::*;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/acp.py");

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("ant3-acp-{}", uuid::Uuid::new_v4())))
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn context() -> Context {
    Context {
        run_id: "model-run".into(),
        source: "platform".into(),
        event_id: "platform-run".into(),
        task: "agent".into(),
        original_text: String::new(),
        metadata: Value::Null,
    }
}

fn executors() -> AcpExecutors {
    AcpExecutors::parse(
        &serde_json::to_vec(&json!([
            {"id":"first","command":"python3","args":[FIXTURE,"first"],"auth_method":"fixture"},
            {"id":"second","command":"python3","args":[FIXTURE,"second"]}
        ]))
        .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn configured_executors_share_a_tool_and_keep_separate_run_sessions() {
    let workspace = Workspace::new();
    let store = Store::memory().unwrap();
    let config = executors();
    let first_run = config
        .tool(store.clone(), workspace.0.join("first"))
        .unwrap();
    let second_run = config
        .tool(store.clone(), workspace.0.join("second"))
        .unwrap();
    let cancel = CancellationToken::new();
    let context = context();
    assert_eq!(
        first_run.spec().parameters["properties"]["executor"]["enum"],
        json!(["first", "second"])
    );
    let first = first_run
        .execute(
            &context,
            json!({"executor":"first","prompt":"one"}),
            cancel.clone(),
        )
        .await
        .unwrap();
    assert_eq!(first.content["output"], "first: one");
    let session = &first.content["session_id"];
    let follow_up = json!({"executor":"first","prompt":"two","session_id":session});
    assert_eq!(
        second_run
            .execute(&context, follow_up.clone(), cancel.clone())
            .await
            .err()
            .unwrap()
            .code,
        "acp_session_not_found"
    );
    let next = first_run
        .execute(&context, follow_up, cancel.clone())
        .await
        .unwrap();
    assert_eq!(next.content["session_id"], *session);
    assert_eq!(next.content["output"], "first: one, two");
    assert_eq!(
        first_run
            .execute(
                &context,
                json!({"executor":"second","prompt":"two","session_id":session}),
                cancel.clone()
            )
            .await
            .err()
            .unwrap()
            .code,
        "acp_session_not_found"
    );
    let other = first_run
        .execute(
            &context,
            json!({"executor":"second","prompt":"three"}),
            cancel,
        )
        .await
        .unwrap();
    assert_eq!(other.content["output"], "second: three");
    assert_eq!(other.content["stop_reason"], "end_turn");

    let count: i64 = store
        .transaction(|tx| {
            Ok(tx.query_row(
                "SELECT count(*) FROM events WHERE subject=? AND kind='execution.acp_progress'",
                [&context.event_id],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn cancellation_reaches_executor_and_disconnect_has_unknown_outcome() {
    let workspace = Workspace::new();
    std::fs::create_dir(&workspace.0).unwrap();
    let config = executors();
    let executor = &config.0["first"];
    let mut session = Session::open(executor, &workspace.0.canonicalize().unwrap())
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    let outcome = session
        .turn("wait", cancel.clone(), Duration::from_secs(2), |_| {
            cancel.cancel();
            Ok(())
        })
        .await;
    assert!(matches!(outcome, Err(transport::TurnError::Cancelled)));

    let mut session = Session::open(executor, &workspace.0.canonicalize().unwrap())
        .await
        .unwrap();
    let outcome = session
        .turn(
            "disconnect",
            CancellationToken::new(),
            Duration::from_secs(2),
            |_| Ok(()),
        )
        .await;
    assert!(matches!(
        outcome,
        Err(transport::TurnError::Unknown("acp_disconnected"))
    ));
}
