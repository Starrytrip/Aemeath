use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::error::{ApiError, ApiResult};
use yukina_store::documents::store::{self, DocumentInput};
use yukina_store::entities::{
    background::{self, Entity as Backgrounds},
    document::{self, Column as DocCol, Entity as Documents},
};

// 保持现有接口字段不变，存储统一来自 documents。
#[derive(Serialize)]
struct ArticleMeta {
    id: i64,
    slug: Option<String>,
    description: String,
    ai_summary: String,
    date: String,
    cover_url: String,
    is_pinned: bool,
}
impl From<&document::Model> for ArticleMeta {
    fn from(doc: &document::Model) -> Self {
        Self {
            id: doc.id,
            slug: doc.slug.clone(),
            description: doc.description.clone(),
            ai_summary: doc.ai_summary.clone(),
            date: doc.date.clone(),
            cover_url: doc.cover_url.clone(),
            is_pinned: doc.is_pinned,
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/articles", get(list_articles).post(save_article))
        .route("/articles/{slug}", get(read_article))
        .route("/article-source", get(resolve_article))
        .route("/articles", get(public_articles))
        .route("/backgrounds", get(list_backgrounds).post(save_background))
        .route("/backgrounds/{id}", axum::routing::patch(update_background))
        .route("/backgrounds/reorder", post(reorder))
        .route("/backgrounds/batch-delete", post(batch_delete))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}
async fn public_articles(State(state): State<AppState>) -> ApiResult<Response> {
    let docs = Documents::find()
        .filter(DocCol::Slug.is_not_null())
        .filter(DocCol::Visibility.eq("public"))
        .order_by_desc(DocCol::IsPinned)
        .order_by_desc(DocCol::Date)
        .all(&state.db)
        .await?;
    let items: Vec<_> = docs.into_iter().map(|d|serde_json::json!({"title":d.title,"slug":d.slug,"description":d.description,"date":d.date,"cover_url":d.cover_url,"category":d.category,"tags":serde_json::from_str::<Vec<String>>(&d.tags).unwrap_or_default(),"url":d.url,"words":d.markdown.chars().count(),"updated_at":d.updated_at,"is_pinned":d.is_pinned})).collect();
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(items)).into_response())
}
use yukina_shared::validation::valid_url;
async fn list_articles(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> ApiResult<Json<Vec<ArticleMeta>>> {
    let docs = Documents::find()
        .filter(DocCol::Slug.is_not_null())
        .all(&state.db)
        .await?;
    Ok(Json(docs.iter().map(ArticleMeta::from).collect()))
}
#[derive(Deserialize)]
struct ArticleInput {
    document: DocumentInput,
    slug: String,
    description: String,
    #[serde(default)]
    ai_summary: Option<String>,
    date: String,
    cover_url: String,
    is_pinned: bool,
}
async fn save_article(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(mut input): Json<ArticleInput>,
) -> ApiResult<Json<serde_json::Value>> {
    if input.slug.is_empty()
        || input.slug.len() > 180
        || !input.slug.chars().all(|c| c.is_alphanumeric() || c == '-')
        || input.description.len() > 10000
        || input.ai_summary.as_ref().is_some_and(|v| v.len() > 10000)
        || chrono::NaiveDate::parse_from_str(&input.date, "%Y-%m-%d").is_err()
        || !valid_url(&input.cover_url)
    {
        return Err(ApiError::Unprocessable("Slug、日期或封面地址不正确".into()));
    }
    let tx = state.db.begin().await?;
    if let Some(existing) = Documents::find()
        .filter(DocCol::Slug.eq(&input.slug))
        .one(&tx)
        .await?
    {
        if existing.source_key != input.document.source_key {
            return Err(ApiError::BadRequest("Slug 已存在".into()));
        }
    }
    input.document.url = format!("/reader/?slug={}", input.slug);
    let doc = store::save(&tx, input.document).await?;
    let mut article = document::ActiveModel {
        id: Set(doc.id),
        slug: Set(Some(input.slug)),
        description: Set(input.description),
        date: Set(input.date),
        cover_url: Set(input.cover_url),
        is_pinned: Set(input.is_pinned),
        ..Default::default()
    };
    if let Some(summary) = input.ai_summary {
        article.ai_summary = Set(summary);
    }
    article.update(&tx).await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"id":doc.id})))
}
async fn read_article(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResult<Response> {
    let doc = Documents::find()
        .filter(DocCol::Slug.eq(slug))
        .filter(DocCol::Visibility.eq("public"))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("文章不存在".into()))?;
    render_article(doc)
}

#[derive(Deserialize)]
struct ArticleSourceQuery {
    source_key: String,
}
async fn resolve_article(
    State(state): State<AppState>,
    Query(query): Query<ArticleSourceQuery>,
) -> ApiResult<Response> {
    if query.source_key.is_empty() || query.source_key.len() > 500 {
        return Err(ApiError::BadRequest("文章来源标识不正确".into()));
    }
    let doc = Documents::find()
        .filter(DocCol::SourceKey.eq(query.source_key))
        .filter(DocCol::Slug.is_not_null())
        .filter(DocCol::Visibility.eq("public"))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("文章不存在或已下架".into()))?;
    render_article(doc)
}

fn render_article(doc: document::Model) -> ApiResult<Response> {
    let meta = ArticleMeta::from(&doc);
    let html = markdown_html(&doc.markdown);
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(
        serde_json::json!({"title":doc.title,"source_key":doc.source_key,"meta":meta,"html":html,"category":doc.category,"tags":serde_json::from_str::<Vec<String>>(&doc.tags).unwrap_or_default()}),
    )).into_response())
}
pub(crate) fn markdown_html(markdown: &str) -> String {
    // 跳过原始 HTML，并拒绝危险链接协议；正文仍按 Markdown 渲染。
    use pulldown_cmark::{CowStr, Event, Tag};
    let events = pulldown_cmark::Parser::new_ext(
        markdown,
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    )
    .filter_map(|event| match event {
        Event::Html(_) | Event::InlineHtml(_) => None,
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Some(Event::Start(Tag::Link {
            link_type,
            dest_url: if valid_url(&dest_url) || dest_url.starts_with('#') {
                dest_url
            } else {
                CowStr::from("")
            },
            title,
            id,
        })),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Some(Event::Start(Tag::Image {
            link_type,
            dest_url: if valid_url(&dest_url) {
                dest_url
            } else {
                CowStr::from("")
            },
            title,
            id,
        })),
        other => Some(other),
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    html
}
async fn list_backgrounds(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<background::Model>>> {
    Ok(Json(
        Backgrounds::find()
            .order_by_asc(background::Column::SortOrder)
            .order_by_asc(background::Column::Id)
            .all(&state.db)
            .await?,
    ))
}
#[derive(Deserialize)]
struct BackgroundInput {
    url: String,
    poster_url: String,
    media_type: String,
    theme: String,
    device: String,
}
fn validate_background(input: &BackgroundInput) -> ApiResult<()> {
    if input.url.is_empty()
        || !valid_url(&input.url)
        || !valid_url(&input.poster_url)
        || !matches!(input.media_type.as_str(), "image" | "video")
        || !matches!(input.theme.as_str(), "dark" | "light" | "all")
        || !matches!(input.device.as_str(), "desktop" | "mobile")
    {
        return Err(ApiError::Unprocessable("背景配置不正确".into()));
    }
    Ok(())
}
async fn update_background(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    Json(input): Json<BackgroundInput>,
) -> ApiResult<Json<background::Model>> {
    validate_background(&input)?;
    use sea_orm::{ActiveModelTrait, IntoActiveModel};
    let mut row = Backgrounds::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("背景不存在".into()))?
        .into_active_model();
    row.url = Set(input.url);
    row.poster_url = Set(input.poster_url);
    row.media_type = Set(input.media_type);
    row.theme = Set(input.theme);
    row.device = Set(input.device);
    Ok(Json(row.update(&state.db).await?))
}
async fn save_background(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(input): Json<BackgroundInput>,
) -> ApiResult<Json<serde_json::Value>> {
    validate_background(&input)?;
    use sea_orm::ActiveModelTrait;
    let row = background::ActiveModel {
        url: Set(input.url),
        poster_url: Set(input.poster_url),
        media_type: Set(input.media_type),
        theme: Set(input.theme),
        device: Set(input.device),
        sort_order: Set(chrono::Utc::now().timestamp_millis()),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;
    Ok(Json(serde_json::json!(row)))
}
#[derive(Deserialize)]
struct Ids {
    ids: Vec<i64>,
}
async fn batch_delete(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(input): Json<Ids>,
) -> ApiResult<Json<serde_json::Value>> {
    if input.ids.len() > 500 {
        return Err(ApiError::BadRequest("一次最多移除500项".into()));
    }
    Backgrounds::delete_many()
        .filter(background::Column::Id.is_in(input.ids))
        .exec(&state.db)
        .await?;
    Ok(Json(serde_json::json!({"deleted":true})))
}
async fn reorder(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(input): Json<Ids>,
) -> ApiResult<Json<serde_json::Value>> {
    if input.ids.len() > 500 {
        return Err(ApiError::BadRequest("一次最多排序500项".into()));
    }
    let selected: std::collections::HashSet<_> = input.ids.iter().copied().collect();
    if selected.len() != input.ids.len() {
        return Err(ApiError::BadRequest("排序清单不能包含重复项".into()));
    }
    let tx = state.db.begin().await?;
    let rows = Backgrounds::find()
        .order_by_asc(background::Column::SortOrder)
        .order_by_asc(background::Column::Id)
        .all(&tx)
        .await?;
    if rows.iter().filter(|row| selected.contains(&row.id)).count() != selected.len() {
        return Err(ApiError::NotFound("部分背景已不存在，请刷新后重试".into()));
    }
    // 筛选视图只交换所选项的排序位置，隐藏项在完整列表中的位置保持一致。
    let mut replacements = input.ids.into_iter();
    for (order, row) in rows.into_iter().enumerate() {
        let id = if selected.contains(&row.id) {
            replacements.next().unwrap()
        } else {
            row.id
        };
        Backgrounds::update_many()
            .col_expr(
                background::Column::SortOrder,
                sea_orm::sea_query::Expr::value(order as i64),
            )
            .filter(background::Column::Id.eq(id))
            .exec(&tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(serde_json::json!({"saved":true})))
}
