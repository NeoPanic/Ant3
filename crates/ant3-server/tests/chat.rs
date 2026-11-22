use ant3_platform::{board::PublishInput, domain::*, models::Models, store::Store};
use ant3_server::api::{App, router};
use serde_json::{Value, json};
use tower::ServiceExt;

fn fixture() -> (Store, Value, Value) {
    let store = Store::memory().unwrap();
    store
        .publish(Preset {
            name: "reader".into(),
            version: "1".into(),
            purpose: "Read".into(),
            objective: "Inspect".into(),
            instructions: "Inspect".into(),
            protocol: Protocol::Demo,
            plugin: None,
        })
        .unwrap();
    let task = store
        .publish_board_task(
            &PublishInput {
                title: "群聊任务".into(),
                instructions: "检查证据".into(),
                acceptance: "返回依据".into(),
                verification: None,
                energy_budget: None,
            },
            "chat-task",
        )
        .unwrap();
    let run = store
        .provision_board_task(task["id"].as_str().unwrap(), "reader", "1", "test")
        .unwrap();
    (store, task, run)
}
fn id(v: &Value) -> &str {
    v["id"].as_str().or_else(|| v["run_id"].as_str()).unwrap()
}

#[test]
fn conversation_projects_existing_tasks_and_distinct_members_without_changing_claims() {
    let (store, task, run) = fixture();
    let work = store.claim().unwrap().unwrap();
    store.claim_board_task(id(&work), id(&task)).unwrap();
    store
        .update_board_progress(id(&work), "已经找到证据", "progress")
        .unwrap();
    store
        .record(
            "execution.text_delta",
            id(&work),
            json!({"generation":"g1","offset":0,"text":"蚂蚁"}),
        )
        .unwrap();
    store
        .record(
            "execution.model_completed",
            id(&work),
            json!({"data":{"text":"蚂蚁成果","reasoning":"private reasoning"}}),
        )
        .unwrap();
    store
        .record(
            "execution.tool_started",
            id(&work),
            json!({"data":{"name":"web.fetch","arguments":{"secret":"hidden"}}}),
        )
        .unwrap();
    let feed = store.chat_events(Some(id(&task)), None, None).unwrap();
    let events = feed["events"].as_array().unwrap();
    assert!(
        events.iter().any(
            |e| e["kind"] == "board.progress_updated" && e["data"]["message"] == "已经找到证据"
        )
    );
    let text = events
        .iter()
        .find(|e| e["kind"] == "execution.text_delta")
        .unwrap();
    assert_eq!(text["root_id"], task["id"]);
    assert_eq!(text["instance_id"], run["instance_id"]);
    assert!(!feed.to_string().contains("private reasoning"));
    assert!(!feed.to_string().contains("hidden"));
    let cursor = feed["cursor"].as_i64().unwrap();
    store
        .record(
            "execution.text_delta",
            id(&work),
            json!({"generation":"g1","offset":6,"text":"继续"}),
        )
        .unwrap();
    let next = store
        .chat_events(Some(id(&task)), Some(cursor), None)
        .unwrap();
    assert_eq!(next["events"].as_array().unwrap().len(), 1);
    assert!(
        store
            .chat_events(Some("unrelated-root"), None, None)
            .unwrap()["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.board_task(id(&task)).unwrap()["run_id"], work["id"]);
}

#[test]
fn history_pages_do_not_lose_or_repeat_events() {
    let (store, task, run) = fixture();
    for n in 0..215 {
        store
            .record(
                "execution.text_delta",
                id(&run),
                json!({"generation":"g","offset":n,"text":"x"}),
            )
            .unwrap();
    }
    let latest = store.chat_events(Some(id(&task)), None, None).unwrap();
    assert_eq!(latest["has_more"], true);
    let boundary = latest["events"][0]["seq"].as_i64().unwrap();
    let older = store
        .chat_events(Some(id(&task)), None, Some(boundary))
        .unwrap();
    let all = older["events"]
        .as_array()
        .unwrap()
        .iter()
        .chain(latest["events"].as_array().unwrap());
    let seqs = all.map(|e| e["seq"].as_i64().unwrap()).collect::<Vec<_>>();
    assert!(seqs.windows(2).all(|p| p[0] < p[1]));
    assert_eq!(seqs.len(), 216);
}

#[tokio::test]
async fn stream_is_authenticated_and_resumes_from_last_event_id() {
    let (store, task, run) = fixture();
    let first = store.chat_events(None, None, None).unwrap()["cursor"]
        .as_i64()
        .unwrap();
    store
        .record(
            "execution.text_delta",
            id(&run),
            json!({"generation":"g","offset":0,"text":"实时"}),
        )
        .unwrap();
    let token = "a".repeat(64);
    let plugins = ant3_plugins::ProcessHost::disabled(store.clone());
    let stop = ant3::CancellationToken::new();
    let app = router(App {
        store,
        token: token.clone(),
        cancellations: Default::default(),
        models: Models::default(),
        plugins,
    })
    .layer(axum::Extension(stop.clone()));
    let request = |auth: bool| {
        let mut r = axum::http::Request::builder()
            .uri(format!("/api/chat/stream?root_id={}&after=0", id(&task)))
            .header("Host", "127.0.0.1:3000")
            .header("Last-Event-ID", first.to_string());
        if auth {
            r = r.header("Authorization", format!("Bearer {token}"));
        }
        r.body(axum::body::Body::empty()).unwrap()
    };
    assert_eq!(
        app.clone().oneshot(request(false)).await.unwrap().status(),
        401
    );
    let response = app.oneshot(request(true)).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut stream = response.into_body().into_data_stream();
    use futures_util::StreamExt;
    let chunk = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(chunk.to_vec()).unwrap();
    assert!(text.contains("execution.text_delta"));
    assert!(text.contains("实时"));
    assert!(!text.contains("board.task_published"));
    stop.cancel();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn provider_publishes_text_before_completion_and_preserves_it_on_cancellation() {
    use ant3::{CancellationToken, LiveOutput, ModelInput};
    use ant3_platform::models::Provider;
    use std::{convert::Infallible, time::Duration};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let endpoint=axum::Router::new().route("/v1/chat/completions",axum::routing::post(|axum::Json(body):axum::Json<Value>| async move {
        assert_eq!(body["stream"],true);
        let chunks=futures_util::stream::unfold(0,|n|async move {
            if n>1 {return None;}
            let text=if n==0 {"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"蚂蚁正在工作\"},\"finish_reason\":null}]}\n\n".to_string()}
            else {tokio::time::sleep(Duration::from_millis(250)).await;"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":6}}\n\ndata: [DONE]\n\n".to_string()};
            Some((Ok::<_,Infallible>(text),n+1))
        });
        ([("content-type","text/event-stream")],axum::body::Body::from_stream(chunks))
    }));
    let server = tokio::spawn(async move {
        axum::serve(listener, endpoint).await.unwrap();
    });
    let provider = Provider::new(
        Protocol::OpenaiCompatible,
        "fixture".into(),
        format!("http://{address}/v1"),
        String::new(),
    )
    .unwrap();
    for cancel_early in [false, true] {
        let live = LiveOutput::default();
        let cancel = CancellationToken::new();
        let completion = provider.complete_streamed(
            ModelInput {
                system: "test".into(),
                text: "test".into(),
                tools: vec![],
                turns: vec![],
            },
            &live,
            cancel.clone(),
        );
        tokio::pin!(completion);
        tokio::select! {
            _=&mut completion=>panic!("reply completed before the delayed terminal frame"),
            _=tokio::time::sleep(Duration::from_millis(100))=>{},
        }
        assert_eq!(live.text_so_far(), "蚂蚁正在工作");
        if cancel_early {
            cancel.cancel();
            assert_eq!(completion.await.err().unwrap().code, "cancelled");
        } else {
            let reply = completion.await.unwrap();
            assert_eq!(reply.text, "蚂蚁正在工作");
            assert_eq!(reply.usage.unwrap().input, 12);
        }
        assert_eq!(live.text_so_far(), "蚂蚁正在工作");
    }
    server.abort();
}

#[tokio::test]
#[ignore = "takes 62 seconds to cross the former HTTP total-request deadline"]
async fn active_stream_survives_sixty_seconds_and_has_room_for_a_long_result() {
    use ant3::{CancellationToken, LiveOutput, ModelInput};
    use ant3_platform::models::Provider;
    use std::{convert::Infallible, time::Duration};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let endpoint = axum::Router::new().route("/v1/chat/completions", axum::routing::post(
        |axum::Json(body): axum::Json<Value>| async move {
            assert_eq!(body["max_completion_tokens"], 8192);
            let chunks = futures_util::stream::unfold(0, |n| async move {
                if n > 7 { return None; }
                let frame = if n < 7 {
                    if n > 0 { tokio::time::sleep(Duration::from_secs(10)).await; }
                    "data: {\"choices\":[{\"delta\":{\"content\":\"文\"}}]}\n\n"
                } else {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"
                };
                Some((Ok::<_, Infallible>(frame), n + 1))
            });
            ([("content-type", "text/event-stream")], axum::body::Body::from_stream(chunks))
        }
    ));
    let server = tokio::spawn(async move {
        axum::serve(listener, endpoint).await.unwrap();
    });
    let provider = Provider::new(
        Protocol::OpenaiCompatible,
        "fixture".into(),
        format!("http://{address}/v1"),
        String::new(),
    )
    .unwrap();
    let live = LiveOutput::default();
    let reply = provider
        .complete_streamed(
            ModelInput {
                system: "test".into(),
                text: "test".into(),
                tools: vec![],
                turns: vec![],
            },
            &live,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(reply.text, "文".repeat(7));
    assert_eq!(live.text_so_far(), reply.text);
    server.abort();
}
