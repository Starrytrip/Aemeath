use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    Json, Router,
};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait,
    FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::{
    error::{ApiError, ApiResult},
    serial::utc_now,
};
use yukina_store::entities::user::{self, Column as Col, Entity as Users};
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/users", axum::routing::get(list).post(create))
        .route(
            "/admin/users/{id}",
            axum::routing::patch(update).delete(remove),
        )
        .route(
            "/admin/users/{id}/reset-password",
            axum::routing::post(reset),
        )
        .route(
            "/admin/users/{id}/revoke-sessions",
            axum::routing::post(revoke),
        )
        .layer(DefaultBodyLimit::max(16 * 1024))
}
#[derive(Serialize, FromQueryResult)]
struct UserRow {
    id: i64,
    username: String,
    email: Option<String>,
    nickname: Option<String>,
    avatar_url: Option<String>,
    is_admin: bool,
    is_active: bool,
    last_login_at: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
}
#[derive(Default, Deserialize)]
struct ListQuery {
    page: Option<u64>,
    page_size: Option<u64>,
    q: Option<String>,
    role: Option<String>,
    active: Option<bool>,
}
async fn list(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut find = Users::find();
    if let Some(s) = q.q.filter(|s| !s.is_empty()) {
        if s.chars().count() > 200 {
            return Err(ApiError::BadRequest("搜索内容过长".into()));
        }
        find = find.filter(
            Condition::any()
                .add(Col::Username.contains(&s))
                .add(Col::Email.contains(&s))
                .add(Col::Nickname.contains(&s)),
        );
    }
    if let Some(role) = q.role.filter(|s| !s.is_empty()) {
        if !matches!(role.as_str(), "admin" | "member") {
            return Err(ApiError::BadRequest("角色筛选无效".into()));
        }
        find = find.filter(Col::IsAdmin.eq(role == "admin"));
    }
    if let Some(active) = q.active {
        find = find.filter(Col::IsActive.eq(active));
    }
    let total = find.clone().count(&state.db).await?;
    let size = q.page_size.unwrap_or(20).clamp(1, 100);
    let page = q.page.unwrap_or(1).max(1);
    let rows = find
        .select_only()
        .columns([
            Col::Id,
            Col::Username,
            Col::Email,
            Col::Nickname,
            Col::AvatarUrl,
            Col::IsAdmin,
            Col::IsActive,
            Col::LastLoginAt,
            Col::CreatedAt,
        ])
        .order_by_desc(Col::Id)
        .limit(size)
        .offset(page.saturating_sub(1).saturating_mul(size))
        .into_model::<UserRow>()
        .all(&state.db)
        .await?;
    Ok(Json(
        serde_json::json!({"items":rows,"total":total,"page":page,"page_size":size}),
    ))
}
#[derive(Deserialize)]
struct EditInput {
    nickname: String,
    email: String,
    is_admin: bool,
    is_active: bool,
}
#[derive(Deserialize)]
struct CreateInput {
    username: String,
    password: String,
    #[serde(flatten)]
    edit: EditInput,
}
fn fields(input: &EditInput) -> ApiResult<(Option<String>, Option<String>)> {
    let nick = input.nickname.trim();
    let email = input.email.trim().to_lowercase();
    if nick.chars().count() > 20 || nick.chars().any(char::is_control) {
        return Err(ApiError::Unprocessable(
            "昵称不能超过20字或包含控制字符".into(),
        ));
    }
    if !email.is_empty() {
        crate::handler::auth::validate_email(&email)?;
    }
    Ok((
        (!nick.is_empty()).then(|| nick.to_owned()),
        (!email.is_empty()).then_some(email),
    ))
}
async fn unique_email(
    db: &impl ConnectionTrait,
    email: &Option<String>,
    except: i64,
) -> ApiResult<()> {
    if let Some(email) = email {
        if Users::find()
            .filter(Col::Email.eq(email))
            .filter(Col::Id.ne(except))
            .count(db)
            .await?
            > 0
        {
            return Err(ApiError::BadRequest("邮箱已被使用".into()));
        }
    }
    Ok(())
}
async fn target(db: &impl ConnectionTrait, id: i64) -> ApiResult<user::Model> {
    Users::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("用户不存在".into()))
}
async fn protect(
    db: &impl ConnectionTrait,
    actor: i64,
    user: &user::Model,
    admin: bool,
    active: bool,
) -> ApiResult<()> {
    if actor == user.id && (!admin || !active) {
        return Err(ApiError::BadRequest(
            "不能禁用、删除或取消自己的管理员身份".into(),
        ));
    }
    if user.is_admin
        && user.is_active
        && (!admin || !active)
        && Users::find()
            .filter(Col::IsAdmin.eq(true))
            .filter(Col::IsActive.eq(true))
            .count(db)
            .await?
            <= 1
    {
        return Err(ApiError::BadRequest("至少保留一个可用管理员".into()));
    }
    Ok(())
}
async fn create(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(input): Json<CreateInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let name = crate::handler::auth::validate_new_username(&input.username)?;
    crate::handler::auth::validate_new_password(&input.password)?;
    let (nick, email) = fields(&input.edit)?;
    let hash = tokio::task::spawn_blocking(move || {
        yukina_runtime::auth::password::hash_password(&input.password)
    })
    .await
    .map_err(|_| ApiError::Internal("密码处理失败".into()))?
    .map_err(|_| ApiError::Internal("密码处理失败".into()))?;
    let tx = state.db.begin().await?;
    unique_email(&tx, &email, 0).await?;
    if Users::find()
        .filter(Col::Username.eq(&name))
        .count(&tx)
        .await?
        > 0
    {
        return Err(ApiError::BadRequest("用户名已存在".into()));
    }
    let row = user::ActiveModel {
        username: Set(name),
        password_hash: Set(hash),
        email: Set(email),
        nickname: Set(nick),
        is_admin: Set(input.edit.is_admin),
        is_active: Set(input.edit.is_active),
        auth_version: Set(0),
        created_at: Set(utc_now()),
        updated_at: Set(utc_now()),
        ..Default::default()
    }
    .insert(&tx)
    .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"id":row.id})))
}
async fn update(
    State(state): State<AppState>,
    AdminUser(actor): AdminUser,
    Path(id): Path<i64>,
    Json(input): Json<EditInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let (nickname, email) = fields(&input)?;
    let tx = state.db.begin().await?;
    let user = target(&tx, id).await?;
    protect(&tx, actor.id, &user, input.is_admin, input.is_active).await?;
    unique_email(&tx, &email, id).await?;
    let changed = user.is_admin != input.is_admin || user.is_active != input.is_active;
    user::ActiveModel {
        id: Set(id),
        email: Set(email),
        nickname: Set(nickname),
        is_admin: Set(input.is_admin),
        is_active: Set(input.is_active),
        auth_version: Set(user.auth_version + i64::from(changed)),
        updated_at: Set(utc_now()),
        ..Default::default()
    }
    .update(&tx)
    .await?;
    tx.commit().await?;
    state.users.invalidate(id);
    if changed {
        state.sessions.revoke_user(id);
    }
    Ok(Json(serde_json::json!({"updated":true})))
}
async fn remove(
    State(state): State<AppState>,
    AdminUser(actor): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let tx = state.db.begin().await?;
    let user = target(&tx, id).await?;
    protect(&tx, actor.id, &user, false, false).await?;
    yukina_store::entities::comment::Entity::update_many()
        .col_expr(
            yukina_store::entities::comment::Column::UserId,
            Expr::value(Option::<i64>::None),
        )
        .filter(yukina_store::entities::comment::Column::UserId.eq(id))
        .exec(&tx)
        .await?;
    Users::delete_by_id(id).exec(&tx).await?;
    tx.commit().await?;
    state.users.invalidate(id);
    state.sessions.revoke_user(id);
    Ok(Json(serde_json::json!({"deleted":true})))
}
#[derive(Deserialize)]
struct PasswordInput {
    password: String,
}
async fn reset(
    State(state): State<AppState>,
    AdminUser(actor): AdminUser,
    Path(id): Path<i64>,
    Json(input): Json<PasswordInput>,
) -> ApiResult<Json<serde_json::Value>> {
    if id == actor.id {
        return Err(ApiError::BadRequest(
            "请通过账号找回密码流程修改自己的密码".into(),
        ));
    }
    crate::handler::auth::validate_new_password(&input.password)?;
    let hash = tokio::task::spawn_blocking(move || {
        yukina_runtime::auth::password::hash_password(&input.password)
    })
    .await
    .map_err(|_| ApiError::Internal("密码处理失败".into()))?
    .map_err(|_| ApiError::Internal("密码处理失败".into()))?;
    let tx = state.db.begin().await?;
    target(&tx, id).await?;
    Users::update_many()
        .col_expr(Col::PasswordHash, Expr::value(hash))
        .col_expr(Col::AuthVersion, Expr::col(Col::AuthVersion).add(1))
        .col_expr(Col::UpdatedAt, Expr::value(utc_now()))
        .filter(Col::Id.eq(id))
        .exec(&tx)
        .await?;
    tx.commit().await?;
    state.users.invalidate(id);
    state.sessions.revoke_user(id);
    Ok(Json(serde_json::json!({"reset":true})))
}
async fn revoke(
    State(state): State<AppState>,
    AdminUser(actor): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    if id == actor.id {
        return Err(ApiError::BadRequest("请使用退出登录结束自己的会话".into()));
    }
    let result = Users::update_many()
        .col_expr(Col::AuthVersion, Expr::col(Col::AuthVersion).add(1))
        .filter(Col::Id.eq(id))
        .exec(&state.db)
        .await?;
    if result.rows_affected == 0 {
        return Err(ApiError::NotFound("用户不存在".into()));
    }
    state.users.invalidate(id);
    state.sessions.revoke_user(id);
    Ok(Json(serde_json::json!({"revoked":true})))
}
