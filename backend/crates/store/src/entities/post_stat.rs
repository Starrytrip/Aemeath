//! 实体：post_stats（按页面维度的阅读量与点赞数）

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "post_stats")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 页面路径，唯一
    #[sea_orm(unique)]
    pub page: String,
    pub views: i64,
    pub likes: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
