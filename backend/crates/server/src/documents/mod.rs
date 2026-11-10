use yukina_store::documents::store;
pub mod indexing;

use axum::{
    extract::{DefaultBodyLimit, Path, State},
    routing::{get, post},
    Json, Router,
};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::error::ApiResult;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/documents", get(list))
        .route("/documents/{id}", get(detail).delete(remove))
        .route("/documents/{id}/reindex", post(reindex))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}

/// 批量同步接口：仅允许管理员调用，不使用通用 API 限流额度。
pub fn sync_router() -> Router<AppState> {
    Router::new()
        .route("/documents", post(save))
        .route("/documents/reconcile", post(reconcile))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}

#[derive(serde::Deserialize)]
struct ReconcileInput {
    source_keys: Vec<String>,
}

async fn reconcile(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(input): Json<ReconcileInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let hidden = store::reconcile(&state.db, input.source_keys).await?;
    Ok(Json(serde_json::json!({"hidden": hidden})))
}

async fn list(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    axum::extract::Query(query): axum::extract::Query<store::ListQuery>,
) -> ApiResult<Json<Vec<store::Document>>> {
    Ok(Json(store::list(&state.db, query).await?))
}
async fn detail(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<store::DocumentDetail>> {
    Ok(Json(store::detail(&state.db, id).await?))
}
async fn save(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(input): Json<store::DocumentInput>,
) -> ApiResult<Json<store::Document>> {
    Ok(Json(store::save(&state.db, input).await?))
}
async fn remove(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    store::remove(&state.db, id).await?;
    Ok(Json(serde_json::json!({"deleted": true})))
}
async fn reindex(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    store::reindex(&state.db, id).await?;
    Ok(Json(
        serde_json::json!({"queued": true, "semantic_configured": state.embeddings.enabled()}),
    ))
}
