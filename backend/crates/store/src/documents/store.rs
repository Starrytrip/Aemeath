use super::chunking;
use crate::entities::{
    document::{self, Column as DocCol, Entity as Documents},
    document_chunk::{self, Column as ChunkCol, Entity as Chunks},
};
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use yukina_shared::{
    error::{ApiError, ApiResult},
    serial::utc_now,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct DocumentInput {
    pub source_key: String,
    pub title: String,
    pub url: String,
    pub markdown: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// 私有文档只保存在后端，不出现在公开检索和外部 embedding 请求中。
    #[serde(default = "private")]
    pub visibility: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
}
fn private() -> String {
    "private".into()
}

#[derive(Debug, Serialize, FromQueryResult)]
pub struct Document {
    pub id: i64,
    pub source_key: String,
    pub title: String,
    pub url: String,
    pub visibility: String,
    pub revision: i64,
    pub index_status: String,
    pub index_error: Option<String>,
    pub updated_at: String,
}
#[derive(Serialize)]
pub struct DocumentDetail {
    #[serde(flatten)]
    pub document: Document,
    pub markdown: String,
    pub category: String,
    pub tags: Vec<String>,
}

#[derive(Default, Deserialize)]
pub struct ListQuery {
    pub offset: Option<u64>,
    pub limit: Option<u64>,
    pub q: Option<String>,
    pub visibility: Option<String>,
}
pub async fn list(db: &DatabaseConnection, query: ListQuery) -> ApiResult<Vec<Document>> {
    let mut select = Documents::find();
    if let Some(q) = query.q.filter(|q| !q.is_empty()) {
        if q.chars().count() > 200 {
            return Err(ApiError::BadRequest("搜索内容过长".into()));
        }
        select = select.filter(
            Condition::any()
                .add(DocCol::Title.contains(&q))
                .add(DocCol::SourceKey.contains(&q))
                .add(DocCol::Category.contains(&q)),
        );
    }
    if let Some(visibility) = query
        .visibility
        .filter(|v| matches!(v.as_str(), "public" | "private"))
    {
        select = select.filter(DocCol::Visibility.eq(visibility));
    }
    Ok(select
        .select_only()
        .columns([
            DocCol::Id,
            DocCol::SourceKey,
            DocCol::Title,
            DocCol::Url,
            DocCol::Visibility,
            DocCol::Revision,
            DocCol::IndexStatus,
            DocCol::IndexError,
            DocCol::UpdatedAt,
        ])
        .order_by_desc(DocCol::Id)
        .offset(query.offset.unwrap_or(0))
        .limit(query.limit.unwrap_or(200).clamp(1, 200))
        .into_model::<Document>()
        .all(db)
        .await?)
}
pub async fn get(db: &impl ConnectionTrait, id: i64) -> ApiResult<Document> {
    Ok(find(db, id).await?.into())
}
async fn find(db: &impl ConnectionTrait, id: i64) -> ApiResult<document::Model> {
    Documents::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("文档不存在".into()))
}
impl From<document::Model> for Document {
    fn from(row: document::Model) -> Self {
        Self {
            id: row.id,
            source_key: row.source_key,
            title: row.title,
            url: row.url,
            visibility: row.visibility,
            revision: row.revision,
            index_status: row.index_status,
            index_error: row.index_error,
            updated_at: row.updated_at,
        }
    }
}
pub async fn detail(db: &DatabaseConnection, id: i64) -> ApiResult<DocumentDetail> {
    let row = find(db, id).await?;
    Ok(DocumentDetail {
        markdown: row.markdown.clone(),
        category: row.category.clone(),
        tags: serde_json::from_str(&row.tags).unwrap_or_default(),
        document: row.into(),
    })
}

fn validate(input: &DocumentInput) -> ApiResult<()> {
    if input.source_key.trim().is_empty()
        || input.source_key.len() > 500
        || input.title.trim().is_empty()
        || input.title.chars().count() > 200
        || input.markdown.trim().is_empty()
        || input.markdown.len() > 1024 * 1024
        || !matches!(input.visibility.as_str(), "public" | "private")
        || !input.url.starts_with('/')
        || input.url.starts_with("//")
        || input.url.contains('\\')
        || input.url.chars().any(char::is_control)
        || input.url.len() > 1000
        || input.category.len() > 200
        || input.tags.len() > 30
        || input.tags.iter().any(|t| t.len() > 100)
        || input.ai_summary.as_ref().is_some_and(|v| v.len() > 10000)
        || input.date.as_ref().is_some_and(|v| {
            v.len() != 10 || chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_err()
        })
        || input
            .cover_url
            .as_ref()
            .is_some_and(|v| !yukina_shared::validation::valid_url(v))
    {
        return Err(ApiError::Unprocessable(
            "文档字段不正确，正文上限 1 MiB，链接须为站内路径".into(),
        ));
    }
    Ok(())
}

pub async fn save(db: &impl TransactionTrait, mut input: DocumentInput) -> ApiResult<Document> {
    validate(&input)?;
    // 摘要是展示元信息，更新时不重建正文切片和语义索引。
    let ai_summary = input.ai_summary.take();
    let date = input.date.take();
    let cover_url = input.cover_url.take();
    let hash = hex::encode(Sha256::digest(
        serde_json::to_vec(&input).map_err(|_| ApiError::BadRequest("文档格式不正确".into()))?,
    ));
    let mut chunks = chunking::split(&input.markdown);
    if chunks.is_empty() {
        chunks.push(chunking::Chunk {
            heading: String::new(),
            content: input.title.clone(),
        });
    }
    if chunks.len() > 2000 {
        return Err(ApiError::Unprocessable("文档片段过多，请拆成多篇".into()));
    }
    let tx = db.begin().await?;
    // 原子 upsert：内容未变时不更新版本，也不重建切片。
    let conflict = OnConflict::column(DocCol::SourceKey)
        .update_columns([
            DocCol::Title,
            DocCol::Url,
            DocCol::Markdown,
            DocCol::Category,
            DocCol::Tags,
            DocCol::Visibility,
            DocCol::ContentHash,
            DocCol::UpdatedAt,
            DocCol::IndexStatus,
        ])
        .value(
            DocCol::Revision,
            Expr::col((Documents, DocCol::Revision)).add(1),
        )
        .value(DocCol::IndexError, Expr::value(Option::<String>::None))
        .value(DocCol::Attempts, 0)
        .value(DocCol::RetryAt, 0)
        .value(DocCol::IndexProfile, "")
        .action_cond_where(
            Condition::any()
                .add(DocCol::ContentHash.ne(hash.clone()))
                .add(DocCol::Visibility.ne(input.visibility.clone())),
        )
        .to_owned();
    let updated = Documents::insert(document::ActiveModel {
        source_key: Set(input.source_key.clone()),
        title: Set(input.title.clone()),
        url: Set(input.url),
        markdown: Set(input.markdown),
        category: Set(input.category.clone()),
        tags: Set(serde_json::to_string(&input.tags).unwrap()),
        visibility: Set(input.visibility.clone()),
        content_hash: Set(hash),
        revision: Set(1),
        index_status: Set(if input.visibility == "private" {
            "private"
        } else {
            "queued"
        }
        .into()),
        index_profile: Set(String::new()),
        attempts: Set(0),
        retry_at: Set(0),
        index_error: Set(None),
        updated_at: Set(utc_now().to_string()),
        ..Default::default()
    })
    .on_conflict(conflict)
    .exec_without_returning(&tx)
    .await?;
    // SQLite upsert 不依赖 last_insert_rowid（更新已有记录时它可能指向其他文档）。
    let mut document = Documents::find()
        .filter(DocCol::SourceKey.eq(input.source_key))
        .one(&tx)
        .await?
        .ok_or_else(|| ApiError::Internal("文档同步失败".into()))?;
    if updated > 0 {
        Chunks::delete_many()
            .filter(ChunkCol::DocumentId.eq(document.id))
            .exec(&tx)
            .await?;
        for (position, chunk) in chunks.into_iter().enumerate() {
            let search_text = format!(
                "{}\n{}\n{}\n{}\n{}",
                input.title,
                input.category,
                input.tags.join(" "),
                chunk.heading,
                chunk.content
            );
            Chunks::insert(document_chunk::ActiveModel {
                document_id: Set(document.id),
                position: Set(position as i64),
                heading: Set(chunk.heading),
                content: Set(chunk.content),
                search_text: Set(search_text),
                embedding: Set(None),
                embedding_profile: Set(None),
                ..Default::default()
            })
            .exec(&tx)
            .await?;
        }
    }
    if ai_summary.is_some() || date.is_some() || cover_url.is_some() {
        let mut metadata = document::ActiveModel {
            id: Set(document.id),
            ..Default::default()
        };
        if let Some(summary) = ai_summary {
            metadata.ai_summary = Set(summary);
        }
        if let Some(date) = date {
            metadata.date = Set(date);
        }
        if let Some(cover) = cover_url {
            metadata.cover_url = Set(cover);
        }
        document = metadata.update(&tx).await?;
    }
    document = promote_imported_article(&tx, document).await?;
    tx.commit().await?;
    Ok(document.into())
}

/// Astro 文章同步记录也是文章；只补缺失标识，不覆盖后台编辑过的内容和元信息。
pub(super) async fn promote_imported_article(
    db: &impl ConnectionTrait,
    doc: document::Model,
) -> ApiResult<document::Model> {
    if !doc.source_key.starts_with("astro-posts:") || doc.slug.is_some() {
        return Ok(doc);
    }
    let candidate = doc
        .url
        .strip_prefix("/posts/")
        .unwrap_or("")
        .trim_matches('/')
        .replace('/', "-");
    let base = if !candidate.is_empty()
        && candidate.len() <= 150
        && candidate.chars().all(|c| c.is_alphanumeric() || c == '-')
    {
        candidate
    } else {
        format!("imported-{}", doc.id)
    };
    let mut slug = base.clone();
    let mut suffix = 0;
    while Documents::find()
        .filter(DocCol::Slug.eq(&slug))
        .one(db)
        .await?
        .is_some()
    {
        suffix += 1;
        slug = format!("{base}-{}-{suffix}", doc.id);
    }
    document::ActiveModel {
        id: Set(doc.id),
        slug: Set(Some(slug)),
        ..Default::default()
    }
    .update(db)
    .await
    .map_err(Into::into)
}

pub async fn remove(db: &DatabaseConnection, id: i64) -> ApiResult<()> {
    let tx = db.begin().await?;
    get(&tx, id).await?;
    Chunks::delete_many()
        .filter(ChunkCol::DocumentId.eq(id))
        .exec(&tx)
        .await?;
    Documents::delete_by_id(id).exec(&tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn reindex(db: &DatabaseConnection, id: i64) -> ApiResult<()> {
    let result = Documents::update_many()
        .col_expr(DocCol::Revision, Expr::col(DocCol::Revision).add(1))
        .col_expr(DocCol::IndexStatus, Expr::value("queued"))
        .col_expr(DocCol::Attempts, Expr::value(0))
        .col_expr(DocCol::RetryAt, Expr::value(0))
        .col_expr(DocCol::IndexError, Expr::value(Option::<String>::None))
        .filter(DocCol::Id.eq(id))
        .filter(DocCol::Visibility.eq("public"))
        .exec(db)
        .await?;
    if result.rows_affected == 0 {
        return Err(ApiError::NotFound("没有可索引的公开文档".into()));
    }
    Ok(())
}

pub async fn reconcile(db: &DatabaseConnection, keys: Vec<String>) -> ApiResult<usize> {
    if keys.len() > 10000
        || keys
            .iter()
            .any(|key| !key.starts_with("astro-posts:") || key.len() > 500)
    {
        return Err(ApiError::BadRequest("同步清单格式不正确".into()));
    }
    let keys: std::collections::HashSet<_> = keys.into_iter().collect();
    let tx = db.begin().await?;
    let rows = Documents::find()
        .select_only()
        .columns([DocCol::Id, DocCol::SourceKey])
        .filter(DocCol::SourceKey.starts_with("astro-posts:"))
        .filter(DocCol::Visibility.eq("public"))
        .into_tuple::<(i64, String)>()
        .all(&tx)
        .await?;
    let mut hidden = 0;
    for (id, key) in rows {
        if !keys.contains(&key) {
            Documents::update_many()
                .col_expr(DocCol::Visibility, Expr::value("private"))
                .col_expr(DocCol::Revision, Expr::col(DocCol::Revision).add(1))
                .col_expr(DocCol::IndexStatus, Expr::value("private"))
                .filter(DocCol::Id.eq(id))
                .exec(&tx)
                .await?;
            Chunks::update_many()
                .col_expr(ChunkCol::Embedding, Expr::value(Option::<Vec<u8>>::None))
                .col_expr(
                    ChunkCol::EmbeddingProfile,
                    Expr::value(Option::<String>::None),
                )
                .filter(ChunkCol::DocumentId.eq(id))
                .exec(&tx)
                .await?;
            hidden += 1;
        }
    }
    tx.commit().await?;
    Ok(hidden)
}
