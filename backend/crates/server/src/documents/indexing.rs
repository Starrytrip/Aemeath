//! 单工作进程持久队列。一次处理一篇，失败退避三次；更新/删除使旧任务失效。
use sea_orm::{
    sea_query::Expr, ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait, RelationTrait, TransactionTrait,
};
use std::time::Duration;
use yukina_embedding::vector_bytes;
use yukina_runtime::state::AppState;
use yukina_shared::error::ApiResult;
use yukina_store::entities::{
    document::{Column as DocCol, Entity as Documents},
    document_chunk::{Column as ChunkCol, Entity as Chunks},
};

pub fn start(state: AppState) {
    if !state.embeddings.enabled() {
        tracing::info!("未配置 embedding 服务，文档全文检索可用，语义索引等待配置");
        return;
    }
    tokio::spawn(async move {
        let profile = state.embeddings.config.profile();
        // 单实例重启回收中断任务；模型或维度变化后自动重排索引。
        if let Err(error) = Documents::update_many()
            .col_expr(DocCol::IndexStatus, Expr::value("queued"))
            .col_expr(DocCol::Attempts, Expr::value(0))
            .col_expr(DocCol::RetryAt, Expr::value(0))
            .filter(DocCol::Visibility.eq("public"))
            .filter(
                Condition::any()
                    .add(DocCol::IndexStatus.eq("indexing"))
                    .add(DocCol::IndexProfile.ne(profile)),
            )
            .exec(&state.db)
            .await
        {
            tracing::error!(%error, "恢复索引队列失败");
            return;
        }
        loop {
            if let Err(error) = tick(&state).await {
                tracing::warn!(%error, "文档索引任务失败");
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });
}

pub async fn tick(state: &AppState) -> ApiResult<()> {
    let now = chrono::Utc::now().timestamp();
    let Some((id, revision, attempts)) = Documents::find()
        .select_only()
        .columns([DocCol::Id, DocCol::Revision, DocCol::Attempts])
        .filter(DocCol::Visibility.eq("public"))
        .filter(DocCol::IndexStatus.eq("queued"))
        .filter(DocCol::RetryAt.lte(now))
        .order_by_asc(DocCol::Id)
        .into_tuple::<(i64, i64, i64)>()
        .one(&state.db)
        .await?
    else {
        return Ok(());
    };
    let claimed = Documents::update_many()
        .col_expr(DocCol::IndexStatus, Expr::value("indexing"))
        .col_expr(DocCol::Attempts, Expr::col(DocCol::Attempts).add(1))
        .filter(DocCol::Id.eq(id))
        .filter(DocCol::Revision.eq(revision))
        .filter(DocCol::IndexStatus.eq("queued"))
        .filter(DocCol::Visibility.eq("public"))
        .exec(&state.db)
        .await?;
    if claimed.rows_affected == 0 {
        return Ok(());
    }
    let profile = state.embeddings.config.profile();
    let update = Documents::update_many()
        .col_expr(DocCol::IndexProfile, Expr::value(profile.clone()))
        .filter(DocCol::Id.eq(id))
        .filter(DocCol::Revision.eq(revision))
        .filter(DocCol::IndexStatus.eq("indexing"))
        .filter(DocCol::Visibility.eq("public"));
    match index(state, id, revision, &profile).await {
        Ok(()) => {
            update
                .col_expr(DocCol::IndexStatus, Expr::value("ready"))
                .col_expr(DocCol::IndexError, Expr::value(Option::<String>::None))
                .exec(&state.db)
                .await?;
        }
        Err(error) => {
            let status = if attempts + 1 >= 3 {
                "failed"
            } else {
                "queued"
            };
            update
                .col_expr(DocCol::IndexStatus, Expr::value(status))
                .col_expr(DocCol::IndexError, Expr::value(error.to_string()))
                .col_expr(DocCol::RetryAt, Expr::value(now + 30 * (attempts + 1)))
                .exec(&state.db)
                .await?;
        }
    }
    Ok(())
}

async fn index(state: &AppState, id: i64, revision: i64, profile: &str) -> ApiResult<()> {
    let mut offset = 0_u64;
    loop {
        let rows = Chunks::find()
            .select_only()
            .columns([ChunkCol::Id, ChunkCol::Heading, ChunkCol::Content])
            .column(DocCol::Title)
            .join(
                sea_orm::JoinType::InnerJoin,
                yukina_store::entities::document_chunk::Relation::Document.def(),
            )
            .filter(DocCol::Id.eq(id))
            .filter(DocCol::Revision.eq(revision))
            .filter(DocCol::Visibility.eq("public"))
            .order_by_asc(ChunkCol::Position)
            .limit(8)
            .offset(offset)
            .into_tuple::<(i64, String, String, String)>()
            .all(&state.db)
            .await?;
        if rows.is_empty() {
            break;
        }
        let inputs = rows
            .iter()
            .map(|(_, heading, content, title)| format!("{title}\n{heading}\n{content}"))
            .collect::<Vec<_>>();
        let vectors = state.embeddings.embed(&inputs).await?;
        let tx = state.db.begin().await?;
        // 将版本和可见性检查放进 UPDATE 子查询，避免检查后文档发生变化。
        let current_document = Documents::find()
            .select_only()
            .column(DocCol::Id)
            .filter(DocCol::Id.eq(id))
            .filter(DocCol::Revision.eq(revision))
            .filter(DocCol::Visibility.eq("public"))
            .into_query();
        for ((chunk_id, _, _, _), vector) in rows.iter().zip(vectors) {
            Chunks::update_many()
                .col_expr(ChunkCol::Embedding, Expr::value(vector_bytes(&vector)))
                .col_expr(ChunkCol::EmbeddingProfile, Expr::value(profile))
                .filter(ChunkCol::Id.eq(*chunk_id))
                .filter(ChunkCol::DocumentId.in_subquery(current_document.clone()))
                .exec(&tx)
                .await?;
        }
        tx.commit().await?;
        offset += rows.len() as u64;
    }
    Ok(())
}
