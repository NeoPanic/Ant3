use super::status::capabilities;
use super::*;
pub(super) async fn snapshot(State(app): State<App>) -> PResult<Json<Value>> {
    let mut value = json!({"messages":app.store.recent_messages()?,"builtin_plugins":ant3_platform::organization::packages(),"builtin_plugin_status":ant3_builtin_plugins::status(),"process_plugins":app.store.process_plugins()?,"process_plugin_execution":app.plugins.is_enabled(),"capacity":app.store.capacity()?,"fleets":app.store.fleets()?,"presets":app.store.list("presets")?,"plugins":app.store.list("plugins")?,"instances":app.store.list("instances")?,"runs":app.store.list("runs")?,"capabilities":capabilities(),"models":app.models.describe(),"limits":{"instances":ant3_platform::limits::INSTANCES,"workers":ant3_platform::limits::WORKERS,"runs_displayed":100,"collection_limit":200,"board_tasks_per_root":ant3_platform::limits::BOARD_TASKS_PER_ROOT,"board_split_depth":ant3_platform::limits::BOARD_SPLIT_DEPTH,"board_split_children":ant3_platform::limits::BOARD_SPLIT_CHILDREN},"settings":app.store.settings()?,"version":env!("CARGO_PKG_VERSION")});
    value["board"] = board_value(&app)?;
    Ok(Json(value))
}
pub(super) fn board_value(app: &App) -> PResult<Value> {
    let mut board = app.store.board_snapshot()?;
    board["router"] = match ant3_platform::routing::Router::from_env(&app.models) {
        Ok(router) => router.describe(),
        Err(error) => json!({"mode":"unavailable","error":error.1}),
    };
    board["routing"] = json!(app.store.board_routing_status()?);
    Ok(board)
}
pub(super) async fn board_list(State(app): State<App>) -> PResult<Json<Value>> {
    Ok(Json(board_value(&app)?))
}
pub(super) async fn board_create(
    State(app): State<App>,
    headers: HeaderMap,
    Json(input): Json<ant3_platform::board::PublishInput>,
) -> PResult<(StatusCode, Json<Value>)> {
    Ok((
        StatusCode::CREATED,
        Json(app.store.publish_board_task(&input, key(&headers)?)?),
    ))
}
pub(super) async fn board_show(
    State(app): State<App>,
    Path(id): Path<String>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.board_task(&id)?))
}
pub(super) async fn board_cancel(
    State(app): State<App>,
    Path(id): Path<String>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.cancel_board_task(&id)?))
}
pub(super) async fn board_review(
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<ant3_platform::board::ReviewInput>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.review_board_task(
        &id,
        &input,
        key(&headers)?,
    )?))
}
pub(super) async fn board_traces(
    State(app): State<App>,
    Query(query): Query<HashMap<String, String>>,
) -> PResult<Json<Value>> {
    let root = query
        .get("root_id")
        .ok_or_else(|| fault(400, "root_id_required"))?;
    Ok(Json(app.store.board_traces(root)?))
}
pub(super) async fn board_audits(
    State(app): State<App>,
    Path(id): Path<String>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.board_audits(&id)?))
}
pub(super) async fn board_notes(
    State(app): State<App>,
    Path(id): Path<String>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.board_notes(&id)?))
}
pub(super) async fn board_energy(
    State(app): State<App>,
    Path(id): Path<String>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.board_energy(&id)?))
}
pub(super) async fn swarm_pool(State(app): State<App>) -> PResult<Json<Value>> {
    Ok(Json(app.store.swarm_pool()?))
}
pub(super) async fn preset_experience(State(app): State<App>) -> PResult<Json<Value>> {
    Ok(Json(json!({"experience":app.store.preset_experience()?})))
}
pub(super) async fn publish(
    State(app): State<App>,
    Json(preset): Json<Value>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.publish_checked(preset, |protocol| {
        if *protocol != Protocol::Demo || !ant3_worker::test_model_enabled() {
            app.models.get(protocol)?;
        }
        Ok(())
    })?))
}
pub(super) async fn install(
    State(app): State<App>,
    Json(bundle): Json<Bundle>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.install(bundle)?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcessInstall {
    source_dir: std::path::PathBuf,
}
pub(super) async fn install_process(
    State(app): State<App>,
    Json(input): Json<ProcessInstall>,
) -> PResult<Json<Value>> {
    Ok(Json(app.plugins.install(input.source_dir).await?))
}
pub(super) async fn process_plugins(State(app): State<App>) -> PResult<Json<Value>> {
    Ok(Json(
        json!({"process_plugins":app.store.process_plugins()?,"execution_enabled":app.plugins.is_enabled()}),
    ))
}
pub(super) async fn process_plugin(
    State(app): State<App>,
    Path((id, version)): Path<(String, String)>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.process_plugin(&id, &version)?))
}
pub(super) async fn process_plugin_action(
    State(app): State<App>,
    Path((id, version)): Path<(String, String)>,
    Json(input): Json<Action>,
) -> PResult<Json<Value>> {
    Ok(Json(
        app.plugins.action(&id, &version, &input.action).await?,
    ))
}
pub(super) fn key(headers: &HeaderMap) -> PResult<&str> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| fault(400, "idempotency_key_required"))
}
pub(super) async fn fleets(State(app): State<App>) -> PResult<Json<Value>> {
    Ok(Json(json!({"fleets":app.store.fleets()?})))
}
pub(super) async fn fleet(State(app): State<App>, Path(id): Path<String>) -> PResult<Json<Value>> {
    Ok(Json(app.store.fleet(&id)?))
}
pub(super) async fn operation(
    State(app): State<App>,
    Path(id): Path<String>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.operation(&id)?))
}
pub(super) async fn fleet_action(
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<FleetAction>,
) -> PResult<(StatusCode, Json<Value>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(app.store.fleet_action(&id, &input, key(&headers)?)?),
    ))
}
pub(super) async fn spawn(
    State(app): State<App>,
    headers: HeaderMap,
    Json(input): Json<FleetPlan>,
) -> PResult<(StatusCode, Json<Value>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(app.store.create_fleet(&input, key(&headers)?)?),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Action {
    action: String,
}
pub(super) async fn transition(
    State(app): State<App>,
    Path(agent): Path<String>,
    Json(input): Json<Action>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.transition(&agent, &input.action)?))
}
pub(super) async fn enqueue(
    State(app): State<App>,
    Path(agent): Path<String>,
    headers: HeaderMap,
    Json(input): Json<RunInput>,
) -> PResult<(StatusCode, Json<Value>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(app.store.enqueue(&agent, &input, key(&headers)?)?),
    ))
}
pub(super) async fn session(
    State(app): State<App>,
    Path(agent): Path<String>,
) -> PResult<Json<Value>> {
    if !app.store.instance_exists(&agent)? {
        return Err(fault(404, "instance_not_found"));
    }
    let session = app
        .store
        .load(&agent)
        .await
        .map_err(|_| fault(500, "session_storage_error"))?;
    Ok(Json(serde_json::to_value(session)?))
}
pub(super) async fn cancel(
    State(app): State<App>,
    Path(run): Path<String>,
) -> PResult<Json<Value>> {
    let result = app.store.cancel(&run)?;
    if let Some(token) = app.cancellations.lock().await.get(&run) {
        token.cancel();
    }
    Ok(Json(result))
}
pub(super) async fn preset_delete(
    State(app): State<App>,
    Path((name, version)): Path<(String, String)>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.delete_preset(&name, &version)?))
}

pub(super) async fn settings(State(app): State<App>) -> PResult<Json<Value>> {
    Ok(Json(app.store.settings()?))
}

pub(super) async fn update_settings(
    State(app): State<App>,
    Json(input): Json<Value>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.update_settings(input)?))
}

pub(super) async fn events(
    State(app): State<App>,
    Query(query): Query<HashMap<String, String>>,
) -> PResult<Json<Value>> {
    let after = query
        .get("after")
        .map(|s| s.parse::<i64>())
        .transpose()
        .map_err(|_| fault(400, "invalid_cursor"))?
        .unwrap_or(0);
    let tail = query.get("tail").is_some_and(|v| v == "1" || v == "true");
    Ok(Json(json!({
        "events": if tail { app.store.latest_events(200)? } else { app.store.events(after)? }
    })))
}

pub(super) async fn run(State(app): State<App>, Path(id): Path<String>) -> PResult<Json<Value>> {
    Ok(Json(app.store.run(&id)?))
}

pub(super) async fn send_message(
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<ant3_platform::messaging::MessageInput>,
) -> PResult<(StatusCode, Json<Value>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(app.store.send_message(
            ant3_platform::messaging::Sender::Operator,
            &id,
            &input,
            key(&headers)?,
        )?),
    ))
}
pub(super) async fn messages(
    State(app): State<App>,
    Query(query): Query<ant3_platform::messaging::MessageQuery>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.messages(&query)?))
}
pub(super) async fn message(
    State(app): State<App>,
    Path(id): Path<String>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.message(&id)?))
}
pub(super) async fn message_action(
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<ant3_platform::messaging::MessageAction>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.message_action(
        &id,
        &input,
        key(&headers)?,
    )?))
}

#[derive(Deserialize, Default)]
pub(super) struct WorkspaceQuery {
    #[serde(default)]
    path: String,
}
pub(super) async fn workspace_catalog(State(app): State<App>) -> PResult<Json<Value>> {
    Ok(Json(app.store.workspace_catalog()?))
}
pub(super) async fn workspace_files(
    State(app): State<App>,
    Path(id): Path<String>,
    Query(q): Query<WorkspaceQuery>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.workspace_files(&id, &q.path)?))
}
pub(super) async fn workspace_file(
    State(app): State<App>,
    Path(id): Path<String>,
    Query(q): Query<WorkspaceQuery>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.workspace_read(&id, &q.path)?))
}
pub(super) async fn workspace_artifacts(State(app): State<App>) -> PResult<Json<Value>> {
    Ok(Json(app.store.workspace_artifacts()?))
}
pub(super) async fn workspace_artifact(
    State(app): State<App>,
    Path(id): Path<String>,
) -> PResult<Json<Value>> {
    Ok(Json(app.store.workspace_artifact(&id)?))
}
