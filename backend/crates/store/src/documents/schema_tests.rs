use super::*;
use crate::documents::store::{self, DocumentInput};
use crate::entities::{document, document_chunk};
use sea_orm::{ColumnTrait, ConnectOptions, Database, EntityTrait, QueryFilter, QueryOrder};

async fn legacy_database() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    migrate(&db).await.unwrap();
    for (key, visibility) in [
        ("published", "public"),
        ("draft", "private"),
        ("reference", "public"),
    ] {
        store::save(
            &db,
            DocumentInput {
                source_key: key.into(),
                title: format!("旧文章 {key}"),
                url: format!("/reader/?slug={key}"),
                markdown: "# 原始正文\n\n保留全文检索与内容".into(),
                category: "生活".into(),
                tags: vec!["旧标签".into()],
                visibility: visibility.into(),
                ai_summary: None,
                date: None,
                cover_url: None,
            },
        )
        .await
        .unwrap();
    }
    // 还原升级前的真实 documents 列结构，覆盖 ALTER TABLE 的升级路径。
    db.execute_unprepared("DROP INDEX idx_documents_slug")
        .await
        .unwrap();
    for column in [
        "slug",
        "description",
        "ai_summary",
        "date",
        "cover_url",
        "is_pinned",
    ] {
        db.execute_unprepared(&format!("ALTER TABLE documents DROP COLUMN {column}"))
            .await
            .unwrap();
    }
    db.execute_unprepared("CREATE TABLE articles (id INTEGER PRIMARY KEY, slug TEXT NOT NULL, description TEXT NOT NULL, date TEXT NOT NULL, cover_url TEXT NOT NULL, is_pinned BOOLEAN NOT NULL)").await.unwrap();
    db.execute_unprepared("INSERT INTO articles VALUES (1,'published','原摘要','2026-11-09','/api/v1/uploads/original.jpg',1),(2,'draft','草稿摘要','2026-11-08','',0)").await.unwrap();
    db
}

async fn has_legacy_table(db: &DatabaseConnection) -> bool {
    db.query_one(Statement::from_string(
        DbBackend::Sqlite,
        "SELECT name FROM sqlite_master WHERE type='table' AND name='articles'".to_string(),
    ))
    .await
    .unwrap()
    .is_some()
}

#[tokio::test]
async fn legacy_articles_merge_without_changing_content_ids_indexes_or_visibility() {
    let db = legacy_database().await;
    let chunks = document_chunk::Entity::find()
        .order_by_asc(document_chunk::Column::Id)
        .all(&db)
        .await
        .unwrap();
    migrate(&db).await.unwrap();
    let row = document::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.source_key, "published");
    assert_eq!(row.title, "旧文章 published");
    assert_eq!(row.markdown, "# 原始正文\n\n保留全文检索与内容");
    assert_eq!(row.category, "生活");
    assert_eq!(row.tags, "[\"旧标签\"]");
    assert_eq!(row.visibility, "public");
    assert_eq!(row.slug.as_deref(), Some("published"));
    assert_eq!(row.description, "原摘要");
    assert_eq!(row.date, "2026-11-09");
    assert_eq!(row.cover_url, "/api/v1/uploads/original.jpg");
    assert!(row.is_pinned);
    let draft = document::Entity::find_by_id(2)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(draft.slug.as_deref(), Some("draft"));
    assert_eq!(draft.visibility, "private");
    let reference = document::Entity::find_by_id(3)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(reference.slug.is_none());
    assert!(!has_legacy_table(&db).await);
    assert_eq!(
        document_chunk::Entity::find()
            .order_by_asc(document_chunk::Column::Id)
            .all(&db)
            .await
            .unwrap(),
        chunks
    );
    let hit = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT count(*) AS n FROM document_fts WHERE document_fts MATCH '\"全文检索\"'"
                .to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(hit.try_get::<i64>("", "n").unwrap() > 0);

    // 重复启动不重建 articles，不改变迁移后的元数据、版本和索引状态。
    crate::migrations::run(&db).await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    assert!(!has_legacy_table(&db).await);
    assert_eq!(
        document::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        row
    );

    // 通用文档更新不会清掉同一记录的博客字段。
    store::save(
        &db,
        DocumentInput {
            source_key: "published".into(),
            title: "更新标题".into(),
            url: row.url.clone(),
            markdown: "新正文".into(),
            category: row.category.clone(),
            tags: vec!["旧标签".into()],
            visibility: "public".into(),
            ai_summary: None,
            date: None,
            cover_url: None,
        },
    )
    .await
    .unwrap();
    let edited = document::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(edited.slug, row.slug);
    assert_eq!(edited.cover_url, row.cover_url);
    assert_eq!(edited.description, row.description);
    assert_eq!(edited.date, row.date);
    assert!(edited.is_pinned);
    assert_eq!(edited.revision, row.revision + 1);
    store::remove(&db, 1).await.unwrap();
    assert!(document::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .is_none());
    assert!(document_chunk::Entity::find()
        .filter(document_chunk::Column::DocumentId.eq(1))
        .all(&db)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn invalid_legacy_articles_rollback_schema_and_keep_the_old_table() {
    for invalid in [
        "INSERT INTO articles VALUES (999,'orphan','','2026-11-09','',0)",
        "UPDATE articles SET slug='published' WHERE id=2",
    ] {
        let db = legacy_database().await;
        db.execute_unprepared(invalid).await.unwrap();
        assert!(migrate(&db).await.is_err());
        assert!(has_legacy_table(&db).await);
        let columns = db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA table_info(documents)".to_string(),
            ))
            .await
            .unwrap();
        assert!(!columns
            .iter()
            .any(|row| row.try_get::<String>("", "name").unwrap() == "slug"));
        let legacy = db
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT description FROM articles WHERE id=1".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            legacy.try_get::<String>("", "description").unwrap(),
            "原摘要"
        );
    }
}
