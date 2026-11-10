//! 由多个 HTTP 处理器共用的响应类型，避免处理器互相依赖。

use serde::Serialize;
use yukina_store::entities::user::Model as UserModel;

/// 当前登录用户信息（含个人资料字段）。
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub is_admin: bool,
    pub nickname: Option<String>,
    pub gender: Option<String>,
    pub birthday: Option<chrono::NaiveDate>,
    pub job: Option<String>,
    pub hobbies: Option<String>,
    pub motto: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub cover_url: Option<String>,
}

impl UserResponse {
    /// 从实体转换为对外响应。
    pub fn from_model(user: &UserModel) -> Self {
        Self {
            id: user.id,
            username: user.username.clone(),
            email: user.email.clone(),
            is_admin: user.is_admin,
            nickname: user.nickname.clone(),
            gender: user.gender.clone(),
            birthday: user.birthday,
            job: user.job.clone(),
            hobbies: user.hobbies.clone(),
            motto: user.motto.clone(),
            bio: user.bio.clone(),
            avatar_url: user.avatar_url.clone(),
            cover_url: user.cover_url.clone(),
        }
    }
}
