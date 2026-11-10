//! 文章统计：阅读量与点赞（点赞按访客去重）

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use yukina_runtime::state::AppState;
use yukina_shared::error::{ApiError, ApiResult};
use yukina_shared::request::normalize_page;
use yukina_shared::serial::utc_now;
use yukina_store::entities::post_like::{
    ActiveModel as LikeActive, Column as LikeCol, Entity as Likes,
};
use yukina_store::entities::post_stat::{
    ActiveModel as StatActive, Column as StatCol, Entity as Stats, Model as StatModel,
};

/// 访客标识请求头（前端生成并持久化在 localStorage）。
const VISITOR_HEADER: &str = "x-visitor-id";

/// 统计查询参数。
#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: String,
}

/// 统计写操作请求体。
#[derive(Debug, Deserialize)]
pub struct PageBody {
    pub page: String,
}

/// 统计响应。
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub page: String,
    pub views: i64,
    pub likes: i64,
    /// 当前访客是否已点赞
    pub liked: bool,
}

/// 读取并校验访客标识。
fn visitor_id(headers: &HeaderMap) -> ApiResult<String> {
    let value = headers
        .get(VISITOR_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ApiError::BadRequest("缺少访客标识".into()))?;
    if !(6..=64).contains(&value.chars().count())
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ApiError::BadRequest("访客标识格式不正确".into()));
    }
    Ok(value.to_string())
}

/// 点赞记录的唯一键：`page + 访客标识` 的 SHA-256 摘要。
pub(crate) fn like_key(page: &str, visitor: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(page.as_bytes());
    hasher.update(b"|");
    hasher.update(visitor.as_bytes());
    hex::encode(hasher.finalize())
}

/// 读取页面统计，不存在时创建一条空记录。
async fn ensure_stat(state: &AppState, page: &str) -> ApiResult<StatModel> {
    if let Some(stat) = Stats::find()
        .filter(StatCol::Page.eq(page))
        .one(&state.db)
        .await?
    {
        return Ok(stat);
    }

    let now = utc_now();
    let inserted = StatActive {
        page: Set(page.to_string()),
        views: Set(0),
        likes: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&state.db)
    .await;

    match inserted {
        Ok(stat) => Ok(stat),
        // 并发首次访问可能触发唯一约束，此时回查已插入的记录
        Err(err) => Stats::find()
            .filter(StatCol::Page.eq(page))
            .one(&state.db)
            .await?
            .ok_or(ApiError::from(err)),
    }
}

/// 当前访客是否已点赞。
async fn is_liked(state: &AppState, key: &str) -> ApiResult<bool> {
    Ok(Likes::find()
        .filter(LikeCol::LikeKey.eq(key))
        .one(&state.db)
        .await?
        .is_some())
}

/* ===== handlers ===== */

/// `GET /api/v1/stats?page=/xxx` — 查询阅读量与点赞数。
pub async fn get_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> ApiResult<Json<StatsResponse>> {
    let page = normalize_page(&query.page)?;
    crate::routes::talks::ensure_visible(&state.db, &page).await?;
    let stat = ensure_stat(&state, &page).await?;
    let liked = match headers
        .get(VISITOR_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(visitor) => is_liked(&state, &like_key(&page, visitor)).await?,
        None => false,
    };
    Ok(Json(StatsResponse {
        views: stat.views,
        likes: stat.likes,
        page,
        liked,
    }))
}

/// `POST /api/v1/stats/views` — 阅读量 +1，并返回最新统计。
pub async fn add_view(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PageBody>,
) -> ApiResult<Json<StatsResponse>> {
    let page = normalize_page(&body.page)?;
    crate::routes::talks::ensure_visible(&state.db, &page).await?;
    let stat = ensure_stat(&state, &page).await?;
    let views = stat.views + 1;

    StatActive {
        id: Set(stat.id),
        views: Set(views),
        updated_at: Set(utc_now()),
        ..Default::default()
    }
    .update(&state.db)
    .await?;

    let liked = match headers
        .get(VISITOR_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(visitor) => is_liked(&state, &like_key(&page, visitor)).await?,
        None => false,
    };

    Ok(Json(StatsResponse {
        page,
        views,
        likes: stat.likes,
        liked,
    }))
}

/// `POST /api/v1/stats/like` — 点赞 / 取消点赞（幂等切换）。
pub async fn toggle_like(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PageBody>,
) -> ApiResult<Json<StatsResponse>> {
    let page = normalize_page(&body.page)?;
    crate::routes::talks::ensure_visible(&state.db, &page).await?;
    let visitor = visitor_id(&headers)?;
    let key = like_key(&page, &visitor);
    let stat = ensure_stat(&state, &page).await?;

    // Take the SQLite write lock before deciding the toggle, so concurrent visitors
    // cannot overwrite each other's counts or race on the unique visitor key.
    let tx = state.db.begin().await?;
    let removed = Likes::delete_many()
        .filter(LikeCol::LikeKey.eq(&key))
        .exec(&tx)
        .await?;
    let liked = removed.rows_affected == 0;
    if liked {
        LikeActive {
            like_key: Set(key),
            page: Set(page.clone()),
            created_at: Set(utc_now()),
            ..Default::default()
        }
        .insert(&tx)
        .await?;
    }
    let likes = Likes::find()
        .filter(LikeCol::Page.eq(&page))
        .count(&tx)
        .await? as i64;
    StatActive {
        id: Set(stat.id),
        likes: Set(likes),
        updated_at: Set(utc_now()),
        ..Default::default()
    }
    .update(&tx)
    .await?;
    tx.commit().await?;

    Ok(Json(StatsResponse {
        page,
        views: stat.views,
        likes,
        liked,
    }))
}
