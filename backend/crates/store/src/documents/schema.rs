//! 统一内容表及派生检索索引。旧文章表迁移、FTS5 和触发器使用 SQLite 原生 DDL。
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    Statement, TransactionTrait,
};
use yukina_shared::error::{ApiError, ApiResult};

pub async fn migrate(db: &DatabaseConnection) -> ApiResult<()> {
    for sql in [
        "CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT, source_key TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL, url TEXT NOT NULL, markdown TEXT NOT NULL,
            category TEXT NOT NULL, tags TEXT NOT NULL,
            visibility TEXT NOT NULL, content_hash TEXT NOT NULL,
            revision INTEGER NOT NULL DEFAULT 1, index_status TEXT NOT NULL,
            index_profile TEXT NOT NULL DEFAULT '', attempts INTEGER NOT NULL DEFAULT 0,
            retry_at INTEGER NOT NULL DEFAULT 0, index_error TEXT,
            updated_at TEXT NOT NULL)",
        "CREATE TABLE IF NOT EXISTS document_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT, document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
            position INTEGER NOT NULL, heading TEXT NOT NULL, content TEXT NOT NULL,
            search_text TEXT NOT NULL, embedding BLOB, embedding_profile TEXT,
            UNIQUE(document_id, position))",
        "CREATE INDEX IF NOT EXISTS idx_document_chunks_document ON document_chunks(document_id)",
        "CREATE INDEX IF NOT EXISTS idx_documents_queue ON documents(index_status, retry_at)",
        "CREATE VIRTUAL TABLE IF NOT EXISTS document_fts USING fts5(search_text, content='document_chunks', content_rowid='id', tokenize='trigram')",
        "CREATE TRIGGER IF NOT EXISTS document_chunks_ai AFTER INSERT ON document_chunks BEGIN
            INSERT INTO document_fts(rowid, search_text) VALUES (new.id, new.search_text); END",
        "CREATE TRIGGER IF NOT EXISTS document_chunks_ad AFTER DELETE ON document_chunks BEGIN
            INSERT INTO document_fts(document_fts, rowid, search_text) VALUES ('delete', old.id, old.search_text); END",
        "CREATE TRIGGER IF NOT EXISTS document_chunks_au AFTER UPDATE OF search_text ON document_chunks BEGIN
            INSERT INTO document_fts(document_fts, rowid, search_text) VALUES ('delete', old.id, old.search_text);
            INSERT INTO document_fts(rowid, search_text) VALUES (new.id, new.search_text); END",
    ] { db.execute_unprepared(sql).await?; }
    merge_articles(db).await?;
    Ok(())
}

/// 补齐单表字段，并在同一事务内迁移旧 articles 数据后移除旧表。
/// 普通资料 slug 保持 NULL，Astro 导入文章补齐标识；原 ID、正文和检索索引保留。
async fn merge_articles(db: &DatabaseConnection) -> ApiResult<()> {
    let tx = db.begin().await?;
    let columns = tx
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA table_info(documents)".to_string(),
        ))
        .await?;
    let names: Vec<String> = columns
        .iter()
        .map(|row| row.try_get("", "name"))
        .collect::<Result<_, _>>()?;
    for (name, definition) in [
        ("slug", "TEXT"),
        ("description", "TEXT NOT NULL DEFAULT ''"),
        ("ai_summary", "TEXT NOT NULL DEFAULT ''"),
        ("date", "TEXT NOT NULL DEFAULT ''"),
        ("cover_url", "TEXT NOT NULL DEFAULT ''"),
        ("is_pinned", "BOOLEAN NOT NULL DEFAULT 0"),
    ] {
        if !names.iter().any(|column| column == name) {
            tx.execute_unprepared(&format!(
                "ALTER TABLE documents ADD COLUMN {name} {definition}"
            ))
            .await?;
        }
    }
    let legacy = tx
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='table' AND name='articles'".to_string(),
        ))
        .await?
        .is_some();
    if legacy {
        // 发现异常旧数据时回滚并停止迁移，避免静默丢失文章元数据。
        let orphan = tx
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT id FROM articles WHERE id NOT IN (SELECT id FROM documents) LIMIT 1"
                    .to_string(),
            ))
            .await?;
        if orphan.is_some() {
            return Err(ApiError::Internal(
                "文章迁移失败：旧 articles 存在未关联正文的记录，原表已保留".into(),
            ));
        }
        tx.execute_unprepared(
            "UPDATE documents SET
            slug=(SELECT slug FROM articles WHERE articles.id=documents.id),
            description=(SELECT description FROM articles WHERE articles.id=documents.id),
            date=(SELECT date FROM articles WHERE articles.id=documents.id),
            cover_url=(SELECT cover_url FROM articles WHERE articles.id=documents.id),
            is_pinned=(SELECT is_pinned FROM articles WHERE articles.id=documents.id)
            WHERE id IN (SELECT id FROM articles)",
        )
        .await?;
    }
    tx.execute_unprepared(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_documents_slug ON documents(slug)",
    )
    .await?;
    if legacy {
        tx.execute_unprepared("DROP TABLE articles").await?;
    }
    // 兼容旧同步脚本只写 documents 的文章，也修复已执行过单表迁移的数据库。
    use crate::entities::document::{Column, Entity};
    let imports = Entity::find()
        .filter(Column::Slug.is_null())
        .filter(Column::SourceKey.starts_with("astro-posts:"))
        .all(&tx)
        .await?;
    for doc in imports {
        super::store::promote_imported_article(&tx, doc).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
