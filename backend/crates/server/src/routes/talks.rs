use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::{
    error::{ApiError, ApiResult},
    serial::utc_now,
};
use yukina_store::entities::{
    post_like, post_stat,
    talk::{self, Column, Entity as Talks},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/talks", get(public_list))
        .route("/admin/talks", get(admin_list).post(create))
        .route(
            "/admin/talks/{id}",
            axum::routing::put(update).delete(remove),
        )
        .layer(DefaultBodyLimit::max(128 * 1024))
}
#[derive(Debug, Deserialize)]
pub struct TalkInput {
    pub content: String,
    pub images: Vec<String>,
    pub published: bool,
    pub is_pinned: bool,
    #[serde(default)]
    pub revision: i64,
}
impl TalkInput {
    fn validate(&self) -> ApiResult<()> {
        if (self.content.trim().is_empty() && self.images.is_empty())
            || self.content.chars().count() > 5000
            || self.revision < 0
            || self.revision == i64::MAX
            || self.images.len() > 9
            || self
                .images
                .iter()
                .any(|url| url.is_empty() || !yukina_shared::validation::valid_url(url))
        {
            return Err(ApiError::Unprocessable(
                "说说需填写文字或图片，文字最多5000字、图片最多9张，图片仅支持HTTPS或站内地址"
                    .into(),
            ));
        }
        Ok(())
    }
    fn images_json(&self) -> ApiResult<String> {
        serde_json::to_string(&self.images)
            .map_err(|_| ApiError::Internal("图片数据保存失败".into()))
    }
}
#[derive(Default, Deserialize)]
pub struct ListQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub id: Option<i64>,
}
#[derive(Serialize)]
pub struct Talk {
    id: i64,
    content: String,
    images: Vec<String>,
    published: bool,
    is_pinned: bool,
    revision: i64,
    created_at: String,
    updated_at: String,
    published_at: Option<String>,
    likes: i64,
    liked: bool,
}
#[derive(Serialize)]
pub struct TalkList {
    items: Vec<Talk>,
    total: u64,
    page: u64,
    page_size: u64,
}
fn date(value: chrono::NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
fn output(row: talk::Model) -> ApiResult<Talk> {
    Ok(Talk {
        id: row.id,
        content: row.content,
        images: serde_json::from_str(&row.images)
            .map_err(|_| ApiError::Internal("说说图片数据损坏".into()))?,
        published: row.published,
        is_pinned: row.is_pinned,
        revision: row.revision,
        created_at: date(row.created_at),
        updated_at: date(row.updated_at),
        published_at: row.published_at.map(date),
        likes: 0,
        liked: false,
    })
}
fn visible() -> sea_orm::Select<Talks> {
    Talks::find().filter(Column::DeletedAt.is_null())
}
async fn find(db: &DatabaseConnection, id: i64) -> ApiResult<talk::Model> {
    visible()
        .filter(Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("说说不存在或已删除".into()))
}
/// Shared comment/stat endpoints must not expose an unpublished or deleted talk.
pub async fn ensure_visible(db: &DatabaseConnection, page: &str) -> ApiResult<()> {
    let Some(rest) = page.strip_prefix("/talks/") else {
        return Ok(());
    };
    if rest.is_empty() {
        return Ok(());
    }
    let id = rest
        .trim_end_matches('/')
        .parse::<i64>()
        .map_err(|_| ApiError::NotFound("说说不存在".into()))?;
    if id <= 0 || page != format!("/talks/{id}/") {
        return Err(ApiError::NotFound("说说不存在".into()));
    }
    let row = find(db, id).await?;
    if !row.published {
        return Err(ApiError::NotFound("说说尚未发布或已下架".into()));
    }
    Ok(())
}
async fn list(
    db: &DatabaseConnection,
    input: ListQuery,
    public: bool,
    visitor: Option<&str>,
) -> ApiResult<TalkList> {
    let page = input.page.unwrap_or(1).max(1);
    let page_size = input.page_size.unwrap_or(12).clamp(1, 50);
    if page > 100000 {
        return Err(ApiError::BadRequest("页码超出范围".into()));
    }
    let mut query = visible();
    if public {
        query = query.filter(Column::Published.eq(true));
    }
    if let Some(id) = input.id {
        query = query.filter(Column::Id.eq(id));
    }
    let total = query.clone().count(db).await?;
    let rows = query
        .order_by_desc(Column::IsPinned)
        .order_by_desc(Column::PublishedAt)
        .order_by_desc(Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size)
        .all(db)
        .await?;
    let paths: Vec<_> = rows
        .iter()
        .map(|row| format!("/talks/{}/", row.id))
        .collect();
    let stats: HashMap<_, _> = post_stat::Entity::find()
        .filter(post_stat::Column::Page.is_in(paths.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|row| (row.page, row.likes))
        .collect();
    let liked: HashSet<String> = if let Some(visitor) = visitor {
        let keys: Vec<_> = paths
            .iter()
            .map(|page| crate::handler::stats::like_key(page, visitor))
            .collect();
        post_like::Entity::find()
            .filter(post_like::Column::LikeKey.is_in(keys))
            .all(db)
            .await?
            .into_iter()
            .map(|row| row.page)
            .collect()
    } else {
        HashSet::new()
    };
    let mut items = Vec::new();
    for row in rows {
        let path = format!("/talks/{}/", row.id);
        let mut item = output(row)?;
        item.likes = *stats.get(&path).unwrap_or(&0);
        item.liked = liked.contains(&path);
        items.push(item);
    }
    Ok(TalkList {
        items,
        total,
        page,
        page_size,
    })
}
async fn public_list(
    State(state): State<AppState>,
    Query(input): Query<ListQuery>,
    headers: HeaderMap,
) -> ApiResult<Json<TalkList>> {
    let visitor = headers
        .get("x-visitor-id")
        .and_then(|value| value.to_str().ok());
    Ok(Json(list(&state.db, input, true, visitor).await?))
}
async fn admin_list(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Query(input): Query<ListQuery>,
) -> ApiResult<Json<TalkList>> {
    Ok(Json(list(&state.db, input, false, None).await?))
}
async fn create(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(input): Json<TalkInput>,
) -> ApiResult<Json<Talk>> {
    input.validate()?;
    let now = utc_now();
    let images = input.images_json()?;
    let row = talk::ActiveModel {
        content: Set(input.content.trim().to_string()),
        images: Set(images),
        published: Set(input.published),
        is_pinned: Set(input.is_pinned),
        revision: Set(1),
        created_at: Set(now),
        updated_at: Set(now),
        published_at: Set(input.published.then_some(now)),
        deleted_at: Set(None),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;
    Ok(Json(output(row)?))
}
async fn update(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    Json(input): Json<TalkInput>,
) -> ApiResult<Json<Talk>> {
    input.validate()?;
    let existing = find(&state.db, id).await?;
    let now = utc_now();
    let published_at = if input.published {
        existing.published_at.or(Some(now))
    } else {
        existing.published_at
    };
    let result = Talks::update_many()
        .set(talk::ActiveModel {
            content: Set(input.content.trim().to_string()),
            images: Set(input.images_json()?),
            published: Set(input.published),
            is_pinned: Set(input.is_pinned),
            revision: Set(input.revision + 1),
            updated_at: Set(now),
            published_at: Set(published_at),
            ..Default::default()
        })
        .filter(Column::Id.eq(id))
        .filter(Column::Revision.eq(input.revision))
        .filter(Column::DeletedAt.is_null())
        .exec(&state.db)
        .await?;
    if result.rows_affected == 0 {
        return Err(ApiError::Conflict("这条说说已被修改，请刷新后重试".into()));
    }
    Ok(Json(output(find(&state.db, id).await?)?))
}
async fn remove(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    find(&state.db, id).await?;
    Talks::update_many()
        .col_expr(
            Column::DeletedAt,
            sea_orm::sea_query::Expr::value(utc_now()),
        )
        .col_expr(Column::Published, sea_orm::sea_query::Expr::value(false))
        .filter(Column::Id.eq(id))
        .exec(&state.db)
        .await?;
    Ok(Json(serde_json::json!({"deleted":true})))
}
