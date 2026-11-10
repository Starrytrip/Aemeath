use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};
use serde::Deserialize;
use yukina_shared::{
    config::Settings,
    error::{ApiError, ApiResult},
};
use yukina_store::documents::store::{self, DocumentInput};
use yukina_store::entities::document;

#[derive(Deserialize)]
struct Image {
    path: String,
    url: String,
}
#[derive(Deserialize)]
struct Article {
    id: i64,
    revision: i64,
    source_key: String,
    old_markdown: String,
    old_cover: String,
    markdown: String,
    cover_url: String,
}
#[derive(Deserialize)]
struct Plan {
    images: Vec<Image>,
    articles: Vec<Article>,
}

/// 本机迁移命令：仅访问本地文件和数据库，不启动 HTTP 服务，不改发布状态。
pub async fn run(settings: &Settings, file: &std::path::Path) -> ApiResult<()> {
    let plan: Plan = serde_json::from_slice(&tokio::fs::read(file).await?)
        .map_err(|_| ApiError::BadRequest("图片迁移计划格式不正确".into()))?;
    let db = yukina_store::db::create_connection(settings).await?;
    yukina_store::migrations::run(&db).await?;
    for asset in &plan.images {
        let name = std::path::Path::new(&asset.path)
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("image")
            .to_string();
        let saved = yukina_media_service::images::store_original(
            &db,
            &settings.data_dir,
            name,
            tokio::fs::read(&asset.path).await?,
        )
        .await?;
        if saved.url != asset.url {
            return Err(ApiError::Conflict(
                "原图内容发生变化，请重新生成迁移计划".into(),
            ));
        }
    }
    let tx = db.begin().await?;
    for article in &plan.articles {
        let row = document::Entity::find_by_id(article.id)
            .one(&tx)
            .await?
            .ok_or_else(|| ApiError::Conflict("文章已删除，请重新预览迁移".into()))?;
        if row.revision != article.revision
            || row.source_key != article.source_key
            || row.markdown != article.old_markdown
            || row.cover_url != article.old_cover
        {
            return Err(ApiError::Conflict(
                "文章在迁移期间已被修改，请重新预览迁移".into(),
            ));
        }
        if !yukina_shared::validation::valid_url(&article.cover_url) {
            return Err(ApiError::BadRequest("迁移封面地址不正确".into()));
        }
        store::save(
            &tx,
            DocumentInput {
                source_key: row.source_key,
                title: row.title,
                url: row.url,
                markdown: article.markdown.clone(),
                category: row.category,
                tags: serde_json::from_str(&row.tags).unwrap_or_default(),
                visibility: row.visibility,
                ai_summary: None,
                date: None,
                cover_url: None,
            },
        )
        .await?;
        document::ActiveModel {
            id: Set(row.id),
            cover_url: Set(article.cover_url.clone()),
            ..Default::default()
        }
        .update(&tx)
        .await?;
    }
    tx.commit().await?;
    println!(
        "迁移完成：{} 张去重原图，{} 篇文章；发布状态和正文文字保留。",
        plan.images.len(),
        plan.articles.len()
    );
    Ok(())
}
