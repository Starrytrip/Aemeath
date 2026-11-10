//! 实体：comments（评论 / 留言）

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "comments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 评论所属页面路径（如 `/posts/hello`、`/guestbook/`）
    pub page: String,
    pub nick: String,
    /// 邮箱仅后台可见，不对外返回
    pub email: Option<String>,
    pub website: Option<String>,
    pub content: String,
    /// 回复的父评论 id（顶层评论为 NULL）
    pub parent_id: Option<i64>,
    /// 登录用户发表时记录其 id
    pub user_id: Option<i64>,
    /// 发表时的客户端 IP（原始值，对外返回时脱敏）
    pub ip: Option<String>,
    /// 发表时的 User-Agent，用于展示浏览器 / 系统信息
    pub user_agent: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    /// 软删除时间，非空即视为已删除
    pub deleted_at: Option<chrono::NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
