//! 认证提取器
//!
//! - `CurrentUser`：必须登录（Bearer Token）
//! - `AdminUser`：必须管理员

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

use crate::state::AppState;
use yukina_shared::error::ApiError;
use yukina_store::entities::user::{Column as UserCol, Entity as Users, Model as UserModel};

/// 从请求头解析 Bearer Token（无 header 返回 None）。
fn bearer_token(parts: &Parts) -> Option<String> {
    let value = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let value = value.trim();
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::to_string)
}

/// 校验用户可用状态。
fn ensure_active(user: UserModel) -> Result<UserModel, ApiError> {
    if !user.is_active {
        return Err(ApiError::Forbidden("账户已被禁用".into()));
    }
    Ok(user)
}

/// 实时校验角色、启用状态与令牌版本；个人资料优先使用短期缓存。
async fn load_user(
    state: &AppState,
    claims: &crate::auth::jwt::Claims,
) -> Result<UserModel, ApiError> {
    if claims.token_type.as_deref() != Some("access") {
        return Err(ApiError::Unauthorized("无效的访问 Token".into()));
    }
    let subject = claims
        .sub
        .clone()
        .ok_or_else(|| ApiError::Unauthorized("Token 格式错误".into()))?;
    let Ok(user_id) = subject.parse::<i64>() else {
        return Err(ApiError::Unauthorized("Token 格式错误".into()));
    };

    // 权限与令牌版本直接查库，禁用/降权/撤销不会被缓存延迟或并发旧缓存覆盖。
    let Some((version, active, admin)) = Users::find_by_id(user_id)
        .select_only()
        .columns([UserCol::AuthVersion, UserCol::IsActive, UserCol::IsAdmin])
        .into_tuple::<(i64, bool, bool)>()
        .one(&state.db)
        .await?
    else {
        return Err(ApiError::Unauthorized("用户不存在".into()));
    };
    if !active {
        return Err(ApiError::Forbidden("账户已被禁用".into()));
    }
    if claims.extra.get("v").and_then(|v| v.as_i64()).unwrap_or(0) != version {
        return Err(ApiError::Unauthorized("登录已失效，请重新登录".into()));
    }
    if let Some(mut user) = state.users.get(user_id) {
        user.is_admin = admin;
        user.is_active = active;
        user.auth_version = version;
        return Ok(user);
    }

    let user = Users::find()
        .filter(UserCol::Id.eq(user_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("用户不存在".into()))?;
    state.users.store(user.clone());
    ensure_active(user)
}

/// 必须登录。
#[derive(Debug, Clone)]
pub struct CurrentUser(pub UserModel);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(token) = bearer_token(parts) else {
            return Err(ApiError::Unauthorized("未提供认证信息".into()));
        };
        let claims = crate::auth::jwt::decode_token(&state.cfg, &token)
            .ok_or_else(|| ApiError::Unauthorized("无效的访问 Token".into()))?;
        Ok(CurrentUser(load_user(state, &claims).await?))
    }
}

/// 可选登录：无 Token 或 Token 无效时按访客处理。
#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<UserModel>);

impl FromRequestParts<AppState> for OptionalUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(token) = bearer_token(parts) else {
            return Ok(OptionalUser(None));
        };
        let Some(claims) = crate::auth::jwt::decode_token(&state.cfg, &token) else {
            return Ok(OptionalUser(None));
        };
        match load_user(state, &claims).await {
            Ok(user) => Ok(OptionalUser(Some(user))),
            Err(_) => Ok(OptionalUser(None)),
        }
    }
}

/// 必须为启用的管理员。
#[derive(Debug, Clone)]
pub struct AdminUser(pub UserModel);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let CurrentUser(user) = CurrentUser::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(ApiError::Forbidden("需要管理员权限".into()));
        }
        Ok(AdminUser(user))
    }
}
