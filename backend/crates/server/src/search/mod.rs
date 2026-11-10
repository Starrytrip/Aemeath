mod articles;
use yukina_embedding::{vector_bytes, Embedder};

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use sea_orm::{
    sea_query::{Expr, LikeExpr},
    ColumnTrait, Condition, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Statement, Value,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use yukina_runtime::state::AppState;
use yukina_shared::error::{ApiError, ApiResult};
use yukina_store::entities::{
    document::{Column as DocCol, Entity as Documents},
    document_chunk::{Column as ChunkCol, Entity as Chunks, Relation as ChunkRelation},
};

// SQLite 专用检索 SQL 集中在此，普通业务操作使用实体查询。
fn sql(query: &str, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Sqlite, query, values)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/search", get(search))
        .route("/search/status", get(status))
        .route("/search/articles", get(articles::search))
}

#[derive(Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Keyword,
    Semantic,
    #[default]
    Hybrid,
}
#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default)]
    pub mode: Mode,
    pub limit: Option<usize>,
}
#[derive(Serialize)]
pub struct SearchResponse {
    pub mode: String,
    pub warning: Option<String>,
    pub results: Vec<SearchHit>,
}
#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub document_id: i64,
    pub title: String,
    pub url: String,
    pub heading: String,
    pub excerpt: String,
    pub category: String,
    pub tags: Vec<String>,
    pub score: f64,
}
#[derive(Debug, Clone, FromQueryResult)]
struct Candidate {
    id: i64,
    document_id: i64,
    title: String,
    url: String,
    heading: String,
    content: String,
    category: String,
    tags: String,
}
const COLUMNS: &str = "c.id,c.document_id,d.title,d.url,c.heading,c.content,d.category,d.tags";

async fn status(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let public = Documents::find().filter(DocCol::Visibility.eq("public"));
    let documents = public.clone().count(&state.db).await?;
    let ready = public
        .filter(DocCol::IndexStatus.eq("ready"))
        .filter(DocCol::IndexProfile.eq(state.embeddings.config.profile()))
        .count(&state.db)
        .await?;
    Ok(Json(serde_json::json!({
        "semantic_configured": state.embeddings.enabled(),
        "documents": documents, "semantic_ready": ready
    })))
}
async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    Ok(Json(run(&state.db, &state.embeddings, query).await?))
}

pub async fn run(
    db: &DatabaseConnection,
    embeddings: &Embedder,
    query: SearchQuery,
) -> ApiResult<SearchResponse> {
    let q = query.q.trim();
    if q.is_empty() || q.chars().count() > 200 {
        return Err(ApiError::BadRequest("请输入 1–200 字的搜索内容".into()));
    }
    let limit = query.limit.unwrap_or(10).clamp(1, 30);
    let mut warning = None;
    let mut semantic = Vec::new();
    if query.mode != Mode::Keyword {
        if embeddings.enabled() {
            let profile = embeddings.config.profile();
            let ready = Documents::find()
                .select_only()
                .column(DocCol::Id)
                .filter(DocCol::Visibility.eq("public"))
                .filter(DocCol::IndexStatus.eq("ready"))
                .filter(DocCol::IndexProfile.eq(profile.clone()))
                .into_tuple::<i64>()
                .one(db)
                .await?
                .is_some();
            if ready {
                match embeddings.embed(&[q.to_string()]).await {
                    Ok(vectors) => {
                        semantic = Candidate::find_by_statement(sql(&format!("SELECT {COLUMNS} FROM document_chunks c JOIN documents d ON d.id=c.document_id WHERE d.visibility='public' AND d.index_status='ready' AND d.index_profile=? AND c.embedding_profile=? AND c.embedding IS NOT NULL ORDER BY vec_distance_cosine(c.embedding,?) ASC,c.id ASC LIMIT 100"), vec![profile.clone().into(),profile.into(),vector_bytes(&vectors[0]).into()])).all(db).await?;
                    }
                    Err(_) => warning = Some("语义服务暂不可用，已切换为关键词搜索".into()),
                }
            } else {
                warning = Some("语义索引尚未就绪，当前使用关键词搜索".into());
            }
        } else {
            warning = Some("未配置语义服务，当前使用关键词搜索".into());
        }
    }
    let keywords = if query.mode != Mode::Semantic || semantic.is_empty() {
        keyword_candidates(db, q).await?
    } else {
        Vec::new()
    };
    let mode = if semantic.is_empty() {
        "keyword"
    } else if query.mode == Mode::Semantic {
        "semantic"
    } else {
        "hybrid"
    };
    Ok(SearchResponse {
        mode: mode.into(),
        warning,
        results: fuse(&keywords, &semantic, q, limit),
    })
}

async fn keyword_candidates(db: &DatabaseConnection, q: &str) -> ApiResult<Vec<Candidate>> {
    let terms: Vec<&str> = q.split_whitespace().take(12).collect();
    if terms.iter().all(|t| t.chars().count() >= 3) {
        // 引号化所有搜索词，用户输入不会成为 FTS 运算符或 SQL 片段。
        let expression = terms
            .iter()
            .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" OR ");
        Ok(Candidate::find_by_statement(sql(&format!("SELECT {COLUMNS} FROM document_fts JOIN document_chunks c ON c.id=document_fts.rowid JOIN documents d ON d.id=c.document_id WHERE document_fts MATCH ? AND d.visibility='public' ORDER BY bm25(document_fts),c.id LIMIT 100"),vec![expression.into()])).all(db).await?)
    } else {
        // trigram 不索引不足三字的词；短中文词使用转义后的子串匹配。
        let mut predicates = Condition::any();
        for term in terms {
            let pattern = format!(
                "%{}%",
                term.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            predicates = predicates.add(
                Expr::col((Chunks, ChunkCol::SearchText)).like(LikeExpr::new(pattern).escape('\\')),
            );
        }
        Ok(Chunks::find()
            .select_only()
            .columns([
                ChunkCol::Id,
                ChunkCol::DocumentId,
                ChunkCol::Heading,
                ChunkCol::Content,
            ])
            .columns([DocCol::Title, DocCol::Url, DocCol::Category, DocCol::Tags])
            .join(sea_orm::JoinType::InnerJoin, ChunkRelation::Document.def())
            .filter(DocCol::Visibility.eq("public"))
            .filter(predicates)
            .order_by_asc(ChunkCol::Id)
            .limit(100)
            .into_model::<Candidate>()
            .all(db)
            .await?)
    }
}

fn fuse(keyword: &[Candidate], semantic: &[Candidate], q: &str, limit: usize) -> Vec<SearchHit> {
    let mut chunks: HashMap<i64, (Candidate, f64)> = HashMap::new();
    for candidates in [keyword, semantic] {
        for (rank, candidate) in candidates.iter().enumerate() {
            let entry = chunks
                .entry(candidate.id)
                .or_insert_with(|| (candidate.clone(), 0.0));
            entry.1 += 1.0 / (60.0 + rank as f64 + 1.0);
        }
    }
    let mut ranked: Vec<_> = chunks.into_values().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.id.cmp(&b.0.id)));
    let mut seen = std::collections::HashSet::new();
    ranked
        .into_iter()
        .filter(|(c, _)| seen.insert(c.document_id))
        .take(limit)
        .map(|(c, score)| {
            let position = q
                .split_whitespace()
                .filter_map(|term| c.content.find(term))
                .min()
                .unwrap_or(0);
            let start = c.content[..position].chars().count().saturating_sub(50);
            let excerpt = c.content.chars().skip(start).take(240).collect();
            SearchHit {
                document_id: c.document_id,
                title: c.title,
                url: c.url,
                heading: c.heading,
                excerpt,
                category: c.category,
                tags: serde_json::from_str(&c.tags).unwrap_or_default(),
                score,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
