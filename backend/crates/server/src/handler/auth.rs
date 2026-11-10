//! 账户认证：邮箱验证码注册、用户名或邮箱登录、邮箱找回密码
//!
//! 安全约定：
//! - 登录失败与账户不存在统一返回 401，避免账号枚举
//! - 找回密码时邮箱未绑定也返回同样的提示
//! - 邮箱验证码只放进程内缓存（仅存 SHA-256 摘要），明文通过邮件下发

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Duration;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::json;

use yukina_runtime::auth::deps::CurrentUser;
use yukina_runtime::auth::{jwt, password};
use yukina_runtime::dto::UserResponse;
use yukina_runtime::state::AppState;
use yukina_shared::error::{ApiError, ApiResult};
use yukina_shared::serial::utc_now;
use yukina_store::entities::user::{ActiveModel as UserActive, Column as UserCol, Entity as Users};

/// 刷新令牌 Cookie 名。
const REFRESH_COOKIE: &str = "yukina_refresh_token";
const LEGACY_REFRESH_COOKIE: &str = "aemeath_refresh_token";
/// 刷新 Cookie 的生效路径（只允许认证接口携带）。
const REFRESH_COOKIE_PATH: &str = "/api/v1/auth";
/// token_type 固定值。
const BEARER: &str = "bearer";
/// 验证码用途：注册。
const PURPOSE_REGISTER: &str = "register";
/// 验证码用途：找回密码。
const PURPOSE_RESET: &str = "reset";

// ===== DTO =====

/// 登录请求：`account` 可填用户名或邮箱（兼容旧的 `username` 字段）。
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    #[serde(alias = "username")]
    pub account: String,
    pub password: String,
}

/// 发送邮箱验证码请求。
#[derive(Debug, Deserialize)]
pub struct SendEmailCodeRequest {
    pub email: String,
    /// `register` 注册 / `reset` 找回密码
    pub purpose: String,
}

/// 注册请求。
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub code: String,
    pub password: String,
}

/// 使用邮箱验证码重置密码请求。
#[derive(Debug, Deserialize)]
pub struct PasswordResetConfirm {
    pub email: String,
    pub code: String,
    pub password: String,
}

/// 登录 / 刷新 / 注册成功响应。
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
}

/// 提示类响应。
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub detail: String,
    /// 验证码有效期（分钟），仅发送验证码接口返回。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_ttl_minutes: Option<i64>,
}

// ===== 通用辅助 =====

/// 写入 Set-Cookie 头（HttpOnly + SameSite=Lax + 可选 Secure）。
///
/// Max-Age 在 Cookie 渲染后手动拼接：cookie crate 的 `max_age` 构造器需要
/// time::Duration，而 time 不是本项目直接依赖。`max_age_secs = 0` 即删除语义。
fn append_set_cookie(
    headers: &mut HeaderMap,
    name: &str,
    value: &str,
    path: &str,
    max_age_secs: i64,
    secure: bool,
) {
    let cookie = Cookie::build((name.to_owned(), value.to_owned()))
        .path(path.to_owned())
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .build();
    let mut text = cookie.to_string();
    text.push_str("; Max-Age=");
    text.push_str(&max_age_secs.to_string());
    if let Ok(v) = header::HeaderValue::from_str(&text) {
        headers.append(header::SET_COOKIE, v);
    } else {
        tracing::error!("Set-Cookie 头构造失败");
    }
}

/// 创建刷新会话并更新最近登录时间，返回 (访问令牌, 刷新令牌)。
async fn create_session(
    state: &AppState,
    user_id: i64,
    auth_version: i64,
) -> ApiResult<(String, String)> {
    let refresh_token = password::create_refresh_token();
    let now = utc_now();
    let expires_at = now + Duration::days(state.cfg.refresh_token_expire_days);

    // 刷新会话只放内存缓存，令牌只以 SHA-256 摘要作为键
    state.sessions.store(
        password::hash_token(&refresh_token),
        user_id,
        auth_version,
        expires_at,
    );

    UserActive {
        id: ActiveValue::Set(user_id),
        last_login_at: Set(Some(now)),
        updated_at: Set(now),
        ..Default::default()
    }
    .update(&state.db)
    .await?;

    let access_token = jwt::create_access_token(
        &state.cfg,
        json!({ "sub": user_id.to_string(), "v":auth_version }),
        None,
    )
    .map_err(|e| ApiError::Internal(format!("签发访问令牌失败: {e}")))?;
    Ok((access_token, refresh_token))
}

// ===== 校验 / 验证码辅助 =====

/// 校验新用户名：2-30 个字符，且不含空白或控制字符。
pub(crate) fn validate_new_username(raw: &str) -> ApiResult<String> {
    let username = raw.trim().to_string();
    let length = username.chars().count();
    if !(2..=30).contains(&length)
        || username
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(ApiError::Unprocessable(
            "用户名需为 2-30 个字符且不含空格".into(),
        ));
    }
    Ok(username)
}

/// 校验新密码长度（与登录规则保持一致）。
pub(crate) fn validate_new_password(password: &str) -> ApiResult<()> {
    if !(8..=128).contains(&password.chars().count()) {
        return Err(ApiError::Unprocessable("密码长度需为 8-128 位".into()));
    }
    Ok(())
}

/// 校验并归一化邮箱：去空格、转小写、基础格式检查。
pub(crate) fn validate_email(raw: &str) -> ApiResult<String> {
    let email = raw.trim().to_lowercase();
    let invalid = || ApiError::Unprocessable("邮箱格式不正确".into());
    if email.chars().count() > 254 || email.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid());
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err(invalid());
    };
    if local.is_empty() || domain.is_empty() || !domain.contains('.') || domain.contains('@') {
        return Err(invalid());
    }
    Ok(email)
}

/// 校验 6 位数字验证码。
fn validate_code(raw: &str) -> ApiResult<String> {
    let code = raw.trim().to_string();
    if code.chars().count() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::Unprocessable("验证码格式不正确".into()));
    }
    Ok(code)
}

/// 生成 6 位数字邮箱验证码。
fn create_email_code() -> String {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let mut code = String::with_capacity(6);
    for _ in 0..6 {
        code.push(char::from(b'0' + rng.gen_range(0..10)));
    }
    code
}

// ===== handlers =====

/// 发送邮箱验证码（注册 / 找回密码共用）。
pub async fn send_email_code(
    State(state): State<AppState>,
    Json(body): Json<SendEmailCodeRequest>,
) -> ApiResult<Json<MessageResponse>> {
    let email = validate_email(&body.email)?;
    let ttl_minutes = state.email_codes.ttl_minutes();
    let purpose = match body.purpose.trim() {
        PURPOSE_REGISTER => PURPOSE_REGISTER,
        PURPOSE_RESET => PURPOSE_RESET,
        _ => return Err(ApiError::Unprocessable("验证码用途不正确".into())),
    };

    // 同一邮箱同用途 60 秒冷却
    if let Some(retry_after) = state.email_codes.cooldown_remaining(purpose, &email) {
        return Err(ApiError::TooManyRequests {
            message: "验证码发送过于频繁，请稍后再试".into(),
            retry_after: Some(retry_after),
        });
    }

    if purpose == PURPOSE_REGISTER {
        let taken = Users::find()
            .filter(UserCol::Email.eq(&email))
            .one(&state.db)
            .await?;
        if taken.is_some() {
            return Err(ApiError::Conflict("该邮箱已被使用".into()));
        }
    } else {
        // 找回密码：邮箱未绑定或账户被禁用时不发信，但返回同一提示避免枚举
        let active = Users::find()
            .filter(UserCol::Email.eq(&email))
            .one(&state.db)
            .await?
            .filter(|u| u.is_active);
        if active.is_none() {
            return Ok(Json(MessageResponse {
                detail: "若该邮箱已绑定账号，验证码已发送，请查收。".into(),
                code_ttl_minutes: Some(ttl_minutes),
            }));
        }
    }

    let code = create_email_code();
    state
        .email_codes
        .store(purpose, &email, password::hash_token(&code));

    let (subject, text) = if purpose == PURPOSE_REGISTER {
        (
            format!("【{}】注册验证码", state.cfg.app_name),
            format!(
                "你的注册验证码是 {code}，{ttl_minutes} 分钟内有效。\n如非本人操作，请忽略此邮件。"
            ),
        )
    } else {
        (
            format!("【{}】找回密码验证码", state.cfg.app_name),
            format!(
                "你的找回密码验证码是 {code}，{ttl_minutes} 分钟内有效。\n如非本人操作，请及时修改密码。"
            ),
        )
    };
    yukina_email_notifier::send_text(&state.cfg, &email, &subject, &text).await?;

    Ok(Json(MessageResponse {
        detail: if purpose == PURPOSE_REGISTER {
            "验证码已发送，请查收邮箱。".into()
        } else {
            "若该邮箱已绑定账号，验证码已发送，请查收。".into()
        },
        code_ttl_minutes: Some(ttl_minutes),
    }))
}

/// 自助注册普通账号：邮箱验证码校验通过后创建账号并直接登录。
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> ApiResult<Response> {
    let username = validate_new_username(&body.username)?;
    let email = validate_email(&body.email)?;
    let code = validate_code(&body.code)?;
    validate_new_password(&body.password)?;

    let exists = Users::find()
        .filter(UserCol::Username.eq(&username))
        .one(&state.db)
        .await?;
    if exists.is_some() {
        return Err(ApiError::Conflict("该用户名已被占用".into()));
    }
    let email_taken = Users::find()
        .filter(UserCol::Email.eq(&email))
        .one(&state.db)
        .await?;
    if email_taken.is_some() {
        return Err(ApiError::Conflict("该邮箱已被使用".into()));
    }

    if !state
        .email_codes
        .consume(PURPOSE_REGISTER, &email, &password::hash_token(&code))
    {
        return Err(ApiError::BadRequest("邮箱验证码无效或已过期".into()));
    }

    let now = utc_now();
    let user = UserActive {
        username: Set(username),
        email: Set(Some(email)),
        password_hash: Set(password::hash_password(&body.password)?),
        is_admin: Set(false),
        is_active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    let (access_token, refresh_token) = create_session(&state, user.id, user.auth_version).await?;
    let mut response = (
        StatusCode::OK,
        Json(TokenResponse {
            access_token,
            token_type: BEARER.into(),
        }),
    )
        .into_response();
    append_set_cookie(
        response.headers_mut(),
        REFRESH_COOKIE,
        &refresh_token,
        REFRESH_COOKIE_PATH,
        state.cfg.refresh_token_expire_days * 86400,
        state.cfg.cookie_secure,
    );
    append_set_cookie(
        response.headers_mut(),
        LEGACY_REFRESH_COOKIE,
        "",
        REFRESH_COOKIE_PATH,
        0,
        state.cfg.cookie_secure,
    );
    Ok(response)
}

/// 用户名或邮箱 + 密码登录，成功后签发访问令牌并种下刷新 Cookie。
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> ApiResult<Response> {
    let account = body.account.trim().to_string();
    if account.is_empty() || account.chars().count() > 254 {
        return Err(ApiError::Unprocessable("请输入用户名或邮箱".into()));
    }
    if !(8..=128).contains(&body.password.chars().count()) {
        return Err(ApiError::Unprocessable("密码长度需为 8-128 位".into()));
    }

    // 含 `@` 视为邮箱，否则按用户名查询
    let user = if account.contains('@') {
        Users::find()
            .filter(UserCol::Email.eq(account.to_lowercase()))
            .one(&state.db)
            .await?
    } else {
        Users::find()
            .filter(UserCol::Username.eq(&account))
            .one(&state.db)
            .await?
    };

    // 密码错误与账户不存在统一 401，避免枚举
    let user = match user {
        Some(u) if password::verify_password(&body.password, &u.password_hash) => u,
        _ => return Err(ApiError::Unauthorized("用户名或密码错误".into())),
    };
    if !user.is_active {
        return Err(ApiError::Forbidden("账户已被禁用".into()));
    }

    let (access_token, refresh_token) = create_session(&state, user.id, user.auth_version).await?;
    let mut response = (
        StatusCode::OK,
        Json(TokenResponse {
            access_token,
            token_type: BEARER.into(),
        }),
    )
        .into_response();
    append_set_cookie(
        response.headers_mut(),
        REFRESH_COOKIE,
        &refresh_token,
        REFRESH_COOKIE_PATH,
        state.cfg.refresh_token_expire_days * 86400,
        state.cfg.cookie_secure,
    );
    append_set_cookie(
        response.headers_mut(),
        LEGACY_REFRESH_COOKIE,
        "",
        REFRESH_COOKIE_PATH,
        0,
        state.cfg.cookie_secure,
    );
    Ok(response)
}

/// 轮换刷新令牌并签发新的访问令牌。
pub async fn refresh_session(State(state): State<AppState>, jar: CookieJar) -> ApiResult<Response> {
    let Some(refresh_token) = jar
        .get(REFRESH_COOKIE)
        .or_else(|| jar.get(LEGACY_REFRESH_COOKIE))
        .map(|c| c.value().to_owned())
        .filter(|v| !v.is_empty())
    else {
        return Err(ApiError::Unauthorized("登录会话不存在".into()));
    };

    // 取出即作废（轮换语义）：旧刷新令牌只能用一次
    let Some((user_id, auth_version)) = state
        .sessions
        .consume(&password::hash_token(&refresh_token), utc_now())
    else {
        return Err(ApiError::Unauthorized("登录会话已失效".into()));
    };

    let user = Users::find_by_id(user_id).one(&state.db).await?;
    let user = match user {
        Some(u) if u.is_active && u.auth_version == auth_version => u,
        _ => return Err(ApiError::Unauthorized("账户不可用".into())),
    };

    let (access_token, refresh_token) = create_session(&state, user.id, user.auth_version).await?;

    let mut response = (
        StatusCode::OK,
        Json(TokenResponse {
            access_token,
            token_type: BEARER.into(),
        }),
    )
        .into_response();
    append_set_cookie(
        response.headers_mut(),
        REFRESH_COOKIE,
        &refresh_token,
        REFRESH_COOKIE_PATH,
        state.cfg.refresh_token_expire_days * 86400,
        state.cfg.cookie_secure,
    );
    append_set_cookie(
        response.headers_mut(),
        LEGACY_REFRESH_COOKIE,
        "",
        REFRESH_COOKIE_PATH,
        0,
        state.cfg.cookie_secure,
    );
    Ok(response)
}

/// 撤销当前刷新会话并删除 Cookie；无会话时也返回 204。
pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> ApiResult<Response> {
    if let Some(token) = jar
        .get(REFRESH_COOKIE)
        .or_else(|| jar.get(LEGACY_REFRESH_COOKIE))
        .map(|c| c.value().to_owned())
        .filter(|v| !v.is_empty())
    {
        state.sessions.remove(&password::hash_token(&token));
    }

    let mut response = StatusCode::NO_CONTENT.into_response();
    // 删除 Cookie：空值 + Max-Age=0
    append_set_cookie(
        response.headers_mut(),
        REFRESH_COOKIE,
        "",
        REFRESH_COOKIE_PATH,
        0,
        state.cfg.cookie_secure,
    );
    append_set_cookie(
        response.headers_mut(),
        LEGACY_REFRESH_COOKIE,
        "",
        REFRESH_COOKIE_PATH,
        0,
        state.cfg.cookie_secure,
    );
    Ok(response)
}

/// 使用邮箱验证码重置密码，成功后撤销该用户全部登录会话。
pub async fn reset_password(
    State(state): State<AppState>,
    Json(body): Json<PasswordResetConfirm>,
) -> ApiResult<Json<MessageResponse>> {
    let email = validate_email(&body.email)?;
    let code = validate_code(&body.code)?;
    validate_new_password(&body.password)?;

    // 邮箱未绑定与验证码错误统一提示，避免枚举
    let Some(user) = Users::find()
        .filter(UserCol::Email.eq(&email))
        .one(&state.db)
        .await?
        .filter(|u| u.is_active)
    else {
        return Err(ApiError::BadRequest("邮箱或验证码不正确".into()));
    };

    if !state
        .email_codes
        .consume(PURPOSE_RESET, &email, &password::hash_token(&code))
    {
        return Err(ApiError::BadRequest("验证码无效或已过期".into()));
    }

    let now = utc_now();
    Users::update_many()
        .col_expr(
            UserCol::PasswordHash,
            sea_orm::sea_query::Expr::value(password::hash_password(&body.password)?),
        )
        .col_expr(
            UserCol::AuthVersion,
            sea_orm::sea_query::Expr::col(UserCol::AuthVersion).add(1),
        )
        .col_expr(UserCol::UpdatedAt, sea_orm::sea_query::Expr::value(now))
        .filter(UserCol::Id.eq(user.id))
        .exec(&state.db)
        .await?;

    // 撤销全部会话，强制重新登录
    state.sessions.revoke_user(user.id);
    state.users.invalidate(user.id);

    Ok(Json(MessageResponse {
        detail: "密码已重置，请使用新密码登录。".into(),
        code_ttl_minutes: None,
    }))
}

/// 获取当前登录用户信息。
pub async fn get_me(CurrentUser(user): CurrentUser) -> Json<UserResponse> {
    Json(UserResponse::from_model(&user))
}
