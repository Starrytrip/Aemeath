//! 实体：comment_likes（评论点赞，按访客去重）

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "comment_likes")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 被点赞的评论 id
    pub comment_id: i64,
    /// 访客标识（前端 localStorage 生成）
    pub visitor_id: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
