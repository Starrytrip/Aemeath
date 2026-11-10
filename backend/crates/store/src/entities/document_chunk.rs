//! 文档切片；全文索引由数据库触发器维护。
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "document_chunks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub document_id: i64,
    pub position: i64,
    pub heading: String,
    pub content: String,
    pub search_text: String,
    pub embedding: Option<Vec<u8>>,
    pub embedding_profile: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::document::Entity",
        from = "Column::DocumentId",
        to = "super::document::Column::Id",
        on_delete = "Cascade"
    )]
    Document,
}

impl Related<super::document::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Document.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
