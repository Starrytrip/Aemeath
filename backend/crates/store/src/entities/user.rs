//! 实体：users

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub username: String,
    /// 绑定邮箱（唯一性由建表时的唯一索引保证，允许为空）
    pub email: Option<String>,
    pub password_hash: String,
    pub is_admin: bool,
    pub is_active: bool,
    #[sea_orm(default_value = 0)]
    pub auth_version: i64,
    /// 昵称（展示用，可留空）
    pub nickname: Option<String>,
    /// 性别：male / female / other
    pub gender: Option<String>,
    pub birthday: Option<chrono::NaiveDate>,
    /// 职位 / 身份
    pub job: Option<String>,
    /// 爱好（自由文本，逗号分隔）
    pub hobbies: Option<String>,
    /// 格言
    pub motto: Option<String>,
    /// 个人简介
    pub bio: Option<String>,
    /// 头像地址（本站上传路径或外链）
    pub avatar_url: Option<String>,
    /// 主页封面地址
    pub cover_url: Option<String>,
    pub last_login_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
