//! 文档原文与持久化索引任务状态。时间保留现有 TEXT 格式，兼容已有数据库。
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "documents")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub source_key: String,
    pub title: String,
    pub url: String,
    pub markdown: String,
    pub category: String,
    pub tags: String,
    pub visibility: String,
    /// 普通导入文档为空；有 slug 的记录作为博客文章发布。
    pub slug: Option<String>,
    pub description: String,
    pub ai_summary: String,
    pub date: String,
    pub cover_url: String,
    pub is_pinned: bool,
    pub content_hash: String,
    pub revision: i64,
    pub index_status: String,
    pub index_profile: String,
    pub attempts: i64,
    pub retry_at: i64,
    pub index_error: Option<String>,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::document_chunk::Entity")]
    Chunks,
}

impl Related<super::document_chunk::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Chunks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
