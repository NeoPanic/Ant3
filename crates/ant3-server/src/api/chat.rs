use super::*;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::unfold;
use std::{collections::VecDeque, convert::Infallible, time::Duration};

#[derive(Deserialize)]
pub(super) struct FeedQuery {
    root_id: Option<String>,
    after: Option<i64>,
    before: Option<i64>,
}
pub(super) async fn history(
    State(app): State<App>,
    Query(q): Query<FeedQuery>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.chat_events(
        q.root_id.as_deref(),
        None,
        q.before,
    )?))
}
pub(super) async fn stream(
    State(app): State<App>,
    shutdown: Option<axum::Extension<ant3::CancellationToken>>,
    headers: HeaderMap,
    Query(q): Query<FeedQuery>,
) -> Response {
    let cursor = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .or(q.after)
        .unwrap_or(0)
        .max(0);
    let stop = shutdown.map(|s| s.0).unwrap_or_default();
    let state = (app.store, q.root_id, cursor, VecDeque::<Value>::new(), stop);
    let feed = unfold(
        state,
        |(store, root, mut cursor, mut queue, stop)| async move {
            loop {
                if stop.is_cancelled() {
                    return None;
                }
                if let Some(event) = queue.pop_front() {
                    let seq = event["seq"].as_i64().unwrap_or(cursor);
                    if queue
                        .front()
                        .is_none_or(|next| next["seq"].as_i64() != Some(seq))
                    {
                        cursor = seq;
                    }
                    let output = Event::default()
                        .event("activity")
                        .id(cursor.to_string())
                        .data(event.to_string());
                    return Some((
                        Ok::<_, Infallible>(output),
                        (store, root, cursor, queue, stop),
                    ));
                }
                match store.chat_events(root.as_deref(), Some(cursor), None) {
                    Ok(chunk) => {
                        queue.extend(chunk["events"].as_array().into_iter().flatten().cloned());
                        if !queue.is_empty() {
                            continue;
                        }
                        cursor = chunk["cursor"].as_i64().unwrap_or(cursor);
                    }
                    Err(_) => {
                        let error = Event::default()
                            .event("read-error")
                            .data("无法读取执行动态，正在重试");
                        tokio::select! {_=stop.cancelled()=>return None,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
                        return Some((Ok(error), (store, root, cursor, queue, stop)));
                    }
                }
                tokio::select! {_=stop.cancelled()=>return None,_=tokio::time::sleep(Duration::from_millis(250))=>{}}
            }
        },
    );
    Sse::new(feed)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
        .into_response()
}
