use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Deserialize;
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::error::{ApiError, ApiResult};
use yukina_store::entities::comment::{Column, Entity as Comments};
pub fn router() -> Router<AppState> {
    Router::new().route("/admin/comments", get(list))
}
#[derive(Deserialize)]
struct Input {
    q: Option<String>,
    page_path: Option<String>,
    page: Option<u64>,
}
async fn list(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Query(input): Query<Input>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = input.page.unwrap_or(1).max(1);
    if page > 100000 {
        return Err(ApiError::BadRequest("页码超出范围".into()));
    }
    let mut query = Comments::find().filter(Column::DeletedAt.is_null());
    if let Some(q) = input.q.filter(|q| !q.is_empty()) {
        if q.len() > 200 {
            return Err(ApiError::BadRequest("搜索词过长".into()));
        }
        query = query.filter(
            Condition::any()
                .add(Column::Content.contains(&q))
                .add(Column::Nick.contains(&q)),
        );
    }
    if let Some(path) = input.page_path.filter(|p| !p.is_empty()) {
        query = query.filter(Column::Page.eq(path));
    }
    let total = query.clone().count(&state.db).await?;
    let rows = query
        .order_by_desc(Column::Id)
        .offset((page - 1) * 30)
        .limit(30)
        .all(&state.db)
        .await?;
    let items:Vec<_>=rows.into_iter().map(|r|serde_json::json!({"id":r.id,"page":r.page,"nick":r.nick,"content":r.content,"parent_id":r.parent_id,"created_at":r.created_at})).collect();
    Ok(Json(
        serde_json::json!({"items":items,"total":total,"page":page}),
    ))
}
