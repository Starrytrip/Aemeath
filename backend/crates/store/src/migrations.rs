//! 建表与补列 — 使用 sea-orm `Schema` 从实体生成 DDL，不手写原生 SQL
//!
//! 等价于 SQLAlchemy 的 `Base.metadata.create_all`：表已存在时跳过；
//! 既有库缺少的新列通过 sea-query 的 `Table::alter` 补齐。

use sea_orm::sea_query::{Alias, ColumnDef, Index, Table};
use sea_orm::{ConnectionTrait, DatabaseConnection, Schema, Statement, TransactionTrait};

use crate::entities::{comment, comment_like, post_like, post_stat, user};
use yukina_shared::error::ApiError;

/// 按实体定义创建全部表（幂等），并为既有库补充新增列。
pub async fn run(db: &DatabaseConnection) -> Result<(), ApiError> {
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    for mut statement in [
        schema.create_table_from_entity(user::Entity),
        schema.create_table_from_entity(crate::entities::site_entry::Entity),
        schema.create_table_from_entity(comment::Entity),
        schema.create_table_from_entity(post_stat::Entity),
        schema.create_table_from_entity(post_like::Entity),
        schema.create_table_from_entity(comment_like::Entity),
        schema.create_table_from_entity(crate::entities::media_image::Entity),
        schema.create_table_from_entity(crate::entities::background::Entity),
        schema.create_table_from_entity(crate::entities::gallery_album::Entity),
        schema.create_table_from_entity(crate::entities::talk::Entity),
        schema.create_table_from_entity(crate::entities::visit_event::Entity),
        schema.create_table_from_entity(crate::entities::music_setting::Entity),
    ] {
        db.execute(backend.build(&statement.if_not_exists().to_owned()))
            .await?;
    }

    // 同一访客对同一评论只能点赞一次
    let like_index = Index::create()
        .if_not_exists()
        .name("idx_comment_likes_unique")
        .table(comment_like::Entity)
        .col(comment_like::Column::CommentId)
        .col(comment_like::Column::VisitorId)
        .unique()
        .to_owned();
    db.execute(backend.build(&like_index)).await?;
    for (name, column) in [
        (
            "idx_visit_time",
            crate::entities::visit_event::Column::OccurredAt,
        ),
        (
            "idx_visit_visitor",
            crate::entities::visit_event::Column::VisitorKey,
        ),
        (
            "idx_visit_ip",
            crate::entities::visit_event::Column::IpAddress,
        ),
    ] {
        db.execute(
            backend.build(
                &Index::create()
                    .if_not_exists()
                    .name(name)
                    .table(crate::entities::visit_event::Entity)
                    .col(column)
                    .to_owned(),
            ),
        )
        .await?;
    }

    // 在事务中检查实际表结构，仅补充缺失字段。
    add_columns(
        db,
        "users",
        vec![
            ColumnDef::new(user::Column::AuthVersion)
                .big_integer()
                .not_null()
                .default(0)
                .to_owned(),
            ColumnDef::new(user::Column::Email).string().to_owned(),
            ColumnDef::new(user::Column::Nickname).string().to_owned(),
            ColumnDef::new(user::Column::Gender).string().to_owned(),
            ColumnDef::new(user::Column::Birthday).date().to_owned(),
            ColumnDef::new(user::Column::Job).string().to_owned(),
            ColumnDef::new(user::Column::Hobbies).string().to_owned(),
            ColumnDef::new(user::Column::Motto).string().to_owned(),
            ColumnDef::new(user::Column::Bio).string().to_owned(),
            ColumnDef::new(user::Column::AvatarUrl).string().to_owned(),
            ColumnDef::new(user::Column::CoverUrl).string().to_owned(),
        ],
    )
    .await?;

    add_columns(
        db,
        "comments",
        vec![
            ColumnDef::new(comment::Column::ParentId)
                .big_integer()
                .to_owned(),
            ColumnDef::new(comment::Column::Ip).string().to_owned(),
            ColumnDef::new(comment::Column::UserAgent)
                .string()
                .to_owned(),
        ],
    )
    .await?;

    // email 唯一性：SQLite 不支持 ADD COLUMN 带 UNIQUE，改用唯一索引；
    // 未绑定邮箱的账号存 NULL，SQLite 允许多个 NULL，不影响这些账号
    let index = Index::create()
        .if_not_exists()
        .name("idx_users_email")
        .table(user::Entity)
        .col(user::Column::Email)
        .unique()
        .to_owned();
    db.execute(backend.build(&index)).await?;

    crate::documents::schema::migrate(db).await?;
    db.execute(
        backend.build(
            &Index::create()
                .if_not_exists()
                .name("idx_media_images_file_name")
                .table(crate::entities::media_image::Entity)
                .col(crate::entities::media_image::Column::FileName)
                .unique()
                .to_owned(),
        ),
    )
    .await?;
    Ok(())
}

/// SQLite 表结构查询和补列在同一事务内完成；真实 SQL 错误直接传播。
async fn add_columns(
    db: &DatabaseConnection,
    table: &str,
    columns: Vec<ColumnDef>,
) -> Result<(), ApiError> {
    let tx = db.begin().await?;
    let backend = tx.get_database_backend();
    let existing = tx
        .query_all(Statement::from_sql_and_values(
            backend,
            "SELECT name FROM pragma_table_info(?)",
            [table.into()],
        ))
        .await?
        .iter()
        .map(|row| row.try_get::<String>("", "name"))
        .collect::<Result<std::collections::HashSet<_>, _>>()?;
    for column in columns {
        if existing.contains(&column.get_column_name()) {
            continue;
        }
        let alter = Table::alter()
            .table(Alias::new(table))
            .add_column(column)
            .to_owned();
        tx.execute(backend.build(&alter)).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    #[tokio::test]
    async fn missing_columns_are_added_without_masking_sql_errors() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT)")
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO users VALUES (1, 'existing')")
            .await
            .unwrap();
        for _ in 0..2 {
            add_columns(
                &db,
                "users",
                vec![ColumnDef::new(Alias::new("email")).string().to_owned()],
            )
            .await
            .unwrap();
        }
        let row = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                "SELECT username,email FROM users WHERE id=1",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get::<String>("", "username").unwrap(), "existing");
        assert!(row
            .try_get::<Option<String>>("", "email")
            .unwrap()
            .is_none());
        assert!(add_columns(
            &db,
            "missing_table",
            vec![ColumnDef::new(Alias::new("email")).string().to_owned()]
        )
        .await
        .is_err());
    }
}
