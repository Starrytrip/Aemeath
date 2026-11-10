use axum::{
    extract::{Query, State},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use sea_orm::{DatabaseConnection, FromQueryResult, Value};
use serde::{Deserialize, Serialize};
use yukina_runtime::state::AppState;
use yukina_shared::error::{ApiError, ApiResult};

#[derive(Deserialize)]
pub struct ArticleSearchQuery {
    pub q: String,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Serialize)]
pub struct ArticleSearchResponse {
    pub items: Vec<ArticleSearchItem>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Serialize)]
pub struct ArticleSearchItem {
    pub id: i64,
    pub slug: String,
    pub url: String,
    pub title: String,
    pub description: String,
    pub excerpt: String,
    pub date: String,
    pub cover_url: String,
    pub category: String,
    pub tags: Vec<String>,
    pub is_pinned: bool,
}

#[derive(FromQueryResult)]
struct ArticleRow {
    id: i64,
    slug: String,
    title: String,
    description: String,
    date: String,
    cover_url: String,
    category: String,
    tags: String,
    is_pinned: bool,
    excerpt: String,
}

#[derive(FromQueryResult)]
struct CountRow {
    total: i64,
}

fn like_pattern(term: &str) -> String {
    format!(
        "%{}%",
        term.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

pub(super) async fn search(
    State(state): State<AppState>,
    Query(query): Query<ArticleSearchQuery>,
) -> ApiResult<Response> {
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(run(&state.db, query).await?),
    )
        .into_response())
}

pub(super) async fn run(
    db: &DatabaseConnection,
    query: ArticleSearchQuery,
) -> ApiResult<ArticleSearchResponse> {
    let q = query.q.trim();
    let terms: Vec<_> = q.split_whitespace().collect();
    if q.is_empty() || q.chars().count() > 200 || terms.len() > 12 {
        return Err(ApiError::BadRequest("请输入1–200字、最多12个搜索词".into()));
    }
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let offset = query.offset.unwrap_or(0);
    if offset > 100_000 {
        return Err(ApiError::BadRequest("搜索分页超出范围".into()));
    }
    let mut predicates = vec![
        "d.visibility='public'".to_string(),
        "d.slug IS NOT NULL".to_string(),
    ];
    let mut values: Vec<Value> = Vec::new();
    for term in terms {
        let pattern = like_pattern(term);
        let body = if term.chars().count() >= 3 {
            "d.id IN (SELECT c.document_id FROM document_fts JOIN document_chunks c ON c.id=document_fts.rowid WHERE document_fts MATCH ?)"
        } else {
            "EXISTS (SELECT 1 FROM document_chunks c WHERE c.document_id=d.id AND c.search_text LIKE ? ESCAPE '\\')"
        };
        predicates.push(format!("(d.title LIKE ? ESCAPE '\\' OR d.description LIKE ? ESCAPE '\\' OR d.category LIKE ? ESCAPE '\\' OR d.tags LIKE ? ESCAPE '\\' OR {body})"));
        for _ in 0..4 {
            values.push(pattern.clone().into());
        }
        values.push(
            if term.chars().count() >= 3 {
                format!("\"{}\"", term.replace('"', "\"\""))
            } else {
                pattern
            }
            .into(),
        );
    }
    // Only records with article fields appear in article search.
    let from = format!("FROM documents d WHERE {}", predicates.join(" AND "));
    let total = CountRow::find_by_statement(super::sql(
        &format!("SELECT count(*) AS total {from}"),
        values.clone(),
    ))
    .one(db)
    .await?
    .map_or(0, |row| row.total as u64);
    values.push(like_pattern(q).into());
    values.push(limit.into());
    values.push(offset.into());
    let rows = ArticleRow::find_by_statement(super::sql(&format!("SELECT d.id,d.slug,d.title,d.description,d.date,d.cover_url,d.category,d.tags,d.is_pinned,COALESCE((SELECT c.content FROM document_chunks c WHERE c.document_id=d.id ORDER BY c.position LIMIT 1),'') AS excerpt {from} ORDER BY CASE WHEN d.title LIKE ? ESCAPE '\\' THEN 0 ELSE 1 END,d.is_pinned DESC,d.date DESC,d.id DESC LIMIT ? OFFSET ?"), values)).all(db).await?;
    Ok(ArticleSearchResponse {
        total,
        limit,
        offset,
        items: rows
            .into_iter()
            .map(|row| ArticleSearchItem {
                id: row.id,
                url: format!("/reader/?slug={}", row.slug),
                slug: row.slug,
                title: row.title,
                description: row.description,
                excerpt: row.excerpt.chars().take(320).collect(),
                date: row.date,
                cover_url: row.cover_url,
                category: row.category,
                tags: serde_json::from_str(&row.tags).unwrap_or_default(),
                is_pinned: row.is_pinned,
            })
            .collect(),
    })
}
