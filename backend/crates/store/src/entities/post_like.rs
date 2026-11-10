//! 实体：post_likes（按页面 + 访客去重的点赞记录）

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "post_likes")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// `page + 访客标识` 的 SHA-256 摘要，唯一约束保证同一访客只能点赞一次
    #[sea_orm(unique)]
    pub like_key: String,
    pub page: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
