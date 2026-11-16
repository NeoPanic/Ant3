use super::auth::{guard, login, logout};
use super::handlers::*;
use super::*;
pub fn router(app: App) -> Router {
    Router::new()
        .route(
            "/board.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/features/board/board.js"
                    )),
                )
            }),
        )
        .route("/api/board", get(board_list))
        .route("/api/board/tasks", post(board_create))
        .route("/api/board/tasks/{id}", get(board_show))
        .route("/api/board/tasks/{id}/cancel", post(board_cancel))
        .route("/api/board/tasks/{id}/review", post(board_review))
        .route("/api/board/tasks/{id}/audits", get(board_audits))
        .route("/api/board/tasks/{id}/energy", get(board_energy))
        .route("/api/board/tasks/{id}/notes", get(board_notes))
        .route("/api/board/traces", get(board_traces))
        .route("/api/presets/experience", get(preset_experience))
        .route("/api/v1/board", get(board_list))
        .route("/api/v1/board/tasks", post(board_create))
        .route("/api/v1/board/tasks/{id}", get(board_show))
        .route("/api/v1/board/tasks/{id}/cancel", post(board_cancel))
        .route("/api/v1/board/tasks/{id}/review", post(board_review))
        .route("/api/v1/board/tasks/{id}/audits", get(board_audits))
        .route("/api/v1/board/tasks/{id}/energy", get(board_energy))
        .route("/api/v1/board/tasks/{id}/notes", get(board_notes))
        .route("/api/v1/board/traces", get(board_traces))
        .route("/api/v1/presets/experience", get(preset_experience))
        .route(
            "/studio.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/features/studio/studio.css"
                    )),
                )
            }),
        )
        .route(
            "/studio.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/features/studio/studio.js"
                    )),
                )
            }),
        )
        .route(
            "/",
            get(|| async {
                Html(include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../web/index.html"
                )))
            }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/app/app.js"
                    )),
                )
            }),
        )
        .route(
            "/app.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/app/app.css"
                    )),
                )
            }),
        )
        .route(
            "/presets.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/features/presets/presets.js"
                    )),
                )
            }),
        )
        .route(
            "/plugins.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/features/plugins/plugins.js"
                    )),
                )
            }),
        )
        .route(
            "/fleets.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/features/agents/fleets.js"
                    )),
                )
            }),
        )
        .route(
            "/mark.svg",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/svg+xml")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../docs/brand/mark.svg"
                    )),
                )
            }),
        )
        .route(
            "/health",
            get(|| async {
                Json(json!({"status":"ok","product":"Ant³","stage":"kernel-preview"}))
            }),
        )
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/v1/snapshot", get(snapshot))
        .route(
            "/messages.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/features/messages/messages.js"
                    )),
                )
            }),
        )
        .route("/api/v1/agent-instances/{id}/messages", post(send_message))
        .route("/api/v1/messages", get(messages))
        .route("/api/v1/messages/{id}", get(message))
        .route("/api/v1/messages/{id}/actions", post(message_action))
        .route("/api/v1/presets", post(publish))
        .route("/api/v1/plugins", post(install))
        .route(
            "/api/v1/process-plugins",
            get(process_plugins).post(install_process),
        )
        .route(
            "/api/v1/process-plugins/{id}/{version}",
            get(process_plugin),
        )
        .route(
            "/api/v1/process-plugins/{id}/{version}/actions",
            post(process_plugin_action),
        )
        .route("/api/v1/agent-fleets", get(fleets).post(spawn))
        .route("/api/v1/agent-fleets/{id}", get(fleet))
        .route("/api/v1/agent-fleets/{id}/actions", post(fleet_action))
        .route("/api/v1/operations/{id}", get(operation))
        .route("/api/v1/agent-instances/{id}/actions", post(transition))
        .route("/api/v1/agent-instances/{id}/runs", post(enqueue))
        .route("/api/v1/agent-instances/{id}/session", get(session))
        .route("/api/v1/runs/{id}", get(run))
        .route("/api/v1/runs/{id}/cancel", post(cancel))
        .route("/api/v1/workspaces", get(workspace_catalog))
        .route(
            "/api/v1/agent-instances/{id}/workspace",
            get(workspace_files),
        )
        .route(
            "/api/v1/agent-instances/{id}/workspace/file",
            get(workspace_file),
        )
        .route("/api/v1/workspace-artifacts", get(workspace_artifacts))
        .route("/api/v1/workspace-artifacts/{id}", get(workspace_artifact))
        .route(
            "/workspaces.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../web/src/features/workspaces/workspaces.js"
                    )),
                )
            }),
        )
        .route("/api/v1/events", get(events))
        .layer(DefaultBodyLimit::max(128 * 1024))
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .with_state(app)
}
