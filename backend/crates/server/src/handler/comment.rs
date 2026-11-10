//! 评论 / 留言：列表、提交（支持嵌套回复）、管理员删除

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Multipart, Path, Query, State};
use axum::http::header::USER_AGENT;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use image::imageops::FilterType;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use yukina_media_service::uploads::{self, UploadResponse};
use yukina_runtime::auth::deps::{AdminUser, OptionalUser};
use yukina_runtime::ip_region::{client_ip, resolve_region};
use yukina_runtime::state::AppState;
use yukina_shared::error::{ApiError, ApiResult};
use yukina_shared::request::normalize_page;
use yukina_shared::serial::utc_now;
use yukina_store::entities::comment::{
    ActiveModel as CommentActive, Column as CommentCol, Entity as Comments,
};
use yukina_store::entities::comment_like::{
    ActiveModel as LikeActive, Column as LikeCol, Entity as CommentLikes,
};
use yukina_store::entities::user::{Column as UserCol, Entity as Users};

/// 昵称最大长度。
const MAX_NICK_LEN: usize = 30;
/// 评论内容最大长度。
const MAX_CONTENT_LEN: usize = 1000;
/// 单页最多返回条数。
const MAX_LIST_LIMIT: u64 = 200;

/// 列表查询参数。
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub page: String,
    pub mine: Option<bool>,
}

/// 提交评论请求（登录用户可省略昵称 / 邮箱 / 网站）。
#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    pub page: String,
    pub nick: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub content: String,
    /// 回复的父评论 id，顶层评论省略
    pub parent_id: Option<i64>,
    /// 提交端浏览器解析出的设备标签（如 `Chrome 141 · Windows 11`）
    pub device: Option<String>,
}

/// 对外返回的评论结构（邮箱不公开）。
#[derive(Debug, Serialize)]
pub struct CommentResponse {
    pub id: i64,
    pub page: String,
    pub nick: String,
    pub website: Option<String>,
    pub content: String,
    pub parent_id: Option<i64>,
    /// 登录用户头像，访客评论为 None
    pub avatar_url: Option<String>,
    /// 评论作者是否为管理员（前端展示「博主」标签）
    pub is_admin: bool,
    /// 评论来源地：境内省市 / 境外国家，解析失败时为脱敏 IP
    pub region: Option<String>,
    /// 发表时的设备标签（浏览器 · 系统）或原始 User-Agent
    pub user_agent: Option<String>,
    /// 点赞数
    pub likes: i64,
    /// 当前访客是否已点赞
    pub liked: bool,
    pub created_at: String,
}

impl CommentResponse {
    fn from_model(
        m: yukina_store::entities::comment::Model,
        avatar_url: Option<String>,
        is_admin: bool,
        region: Option<String>,
        likes: i64,
        liked: bool,
    ) -> Self {
        Self {
            id: m.id,
            page: m.page,
            nick: m.nick,
            website: m.website,
            content: m.content,
            parent_id: m.parent_id,
            avatar_url,
            is_admin,
            region,
            user_agent: m.user_agent,
            likes,
            liked,
            created_at: m.created_at.and_utc().to_rfc3339(),
        }
    }
}

/// 读取访客标识（用于点赞去重）；缺失或过长时视为无。
fn visitor_id(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("x-visitor-id")?.to_str().ok()?.trim();
    (!value.is_empty() && value.len() <= 128).then(|| value.to_string())
}

/// 去除首尾空白后校验长度。
fn required_text(value: &str, field: &str, max: usize) -> ApiResult<String> {
    let text = value.trim();
    if text.is_empty() || text.chars().count() > max {
        return Err(ApiError::Unprocessable(format!("{field}格式不正确")));
    }
    Ok(text.to_string())
}

/// 可选的站点地址：仅允许 http/https。
fn optional_website(value: Option<String>) -> ApiResult<Option<String>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let value = raw.trim().to_string();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 200
        || !(value.starts_with("http://") || value.starts_with("https://"))
    {
        return Err(ApiError::Unprocessable("个人网站格式不正确".into()));
    }
    Ok(Some(value))
}

/// 可选的邮箱：仅做基本格式校验。
fn optional_email(value: Option<String>) -> ApiResult<Option<String>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let value = raw.trim().to_string();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 120 || !value.contains('@') || value.contains(char::is_whitespace) {
        return Err(ApiError::Unprocessable("邮箱格式不正确".into()));
    }
    Ok(Some(value))
}

/// 判断某条评论是否挂在已删除的祖先之下（用于隐藏删除后遗留的子回复）。
fn has_deleted_ancestor(
    mut parent: Option<i64>,
    parent_of: &HashMap<i64, Option<i64>>,
    deleted: &HashSet<i64>,
) -> bool {
    let mut guard = 0;
    while let Some(id) = parent {
        if deleted.contains(&id) {
            return true;
        }
        if guard > 1000 {
            return false;
        }
        guard += 1;
        parent = parent_of.get(&id).copied().flatten();
    }
    false
}

/// `GET /api/v1/comments?page=/xxx` — 列出某页面的评论（含回复）。
pub async fn list_comments(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    OptionalUser(user): OptionalUser,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<CommentResponse>>> {
    let page = normalize_page(&query.page)?;
    crate::routes::talks::ensure_visible(&state.db, &page).await?;
    // 一并取出已删除的评论，用于判断子回复是否应随父级一起隐藏
    let rows = Comments::find()
        .filter(CommentCol::Page.eq(page))
        .order_by_asc(CommentCol::CreatedAt)
        .limit(MAX_LIST_LIMIT)
        .all(&state.db)
        .await?;

    let parent_of: HashMap<i64, Option<i64>> =
        rows.iter().map(|row| (row.id, row.parent_id)).collect();
    let deleted: HashSet<i64> = rows
        .iter()
        .filter(|row| row.deleted_at.is_some())
        .map(|row| row.id)
        .collect();
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|row| !deleted.contains(&row.id))
        .filter(|row| !has_deleted_ancestor(row.parent_id, &parent_of, &deleted))
        .collect();

    // 邮箱只通过请求头传递，不出现在 URL 或公开响应中。
    let rows = if query.mine.unwrap_or(false) {
        let email = headers
            .get("x-comment-email")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        rows.into_iter()
            .filter(|row| {
                if let Some(ref user) = user {
                    row.user_id == Some(user.id)
                } else {
                    email.as_ref().is_some_and(|email| {
                        !email.is_empty()
                            && row
                                .email
                                .as_ref()
                                .is_some_and(|stored| stored.eq_ignore_ascii_case(email))
                    })
                }
            })
            .collect::<Vec<_>>()
    } else {
        rows
    };

    // 批量取登录用户的头像与管理员身份，避免逐条查询
    let user_ids: Vec<i64> = rows
        .iter()
        .filter_map(|row| row.user_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let users: HashMap<i64, (Option<String>, bool)> = if user_ids.is_empty() {
        HashMap::new()
    } else {
        Users::find()
            .filter(UserCol::Id.is_in(user_ids))
            .all(&state.db)
            .await?
            .into_iter()
            .map(|user| (user.id, (user.avatar_url, user.is_admin)))
            .collect()
    };

    // 批量读取点赞，统计总数并标记当前访客是否已点赞
    let visitor = visitor_id(&headers);
    let comment_ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
    let (like_counts, liked_ids): (HashMap<i64, i64>, HashSet<i64>) = if comment_ids.is_empty() {
        (HashMap::new(), HashSet::new())
    } else {
        let likes = CommentLikes::find()
            .filter(LikeCol::CommentId.is_in(comment_ids))
            .all(&state.db)
            .await?;
        let mut counts = HashMap::new();
        let mut liked = HashSet::new();
        for like in likes {
            *counts.entry(like.comment_id).or_insert(0) += 1;
            if visitor.as_deref() == Some(like.visitor_id.as_str()) {
                liked.insert(like.comment_id);
            }
        }
        (counts, liked)
    };

    let items = rows
        .into_iter()
        .map(|row| {
            let profile = row.user_id.and_then(|id| users.get(&id).cloned());
            let (avatar_url, is_admin) = match profile {
                Some((avatar, admin)) => (avatar, admin),
                None => (None, false),
            };
            let likes = like_counts.get(&row.id).copied().unwrap_or(0);
            let liked = liked_ids.contains(&row.id);
            let region = resolve_region(&state, row.ip.as_deref());
            CommentResponse::from_model(row, avatar_url, is_admin, region, likes, liked)
        })
        .collect();
    Ok(Json(items))
}

/// `POST /api/v1/comments` — 提交评论或回复；登录用户免填昵称等资料。
pub async fn create_comment(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<CreateRequest>,
) -> ApiResult<(StatusCode, Json<CommentResponse>)> {
    let page = normalize_page(&body.page)?;
    crate::routes::talks::ensure_visible(&state.db, &page).await?;
    let content = required_text(&body.content, "评论内容", MAX_CONTENT_LEN)?;

    // 登录态直接使用账号昵称与头像；访客必须填写昵称
    let (nick, email, website, user_id, avatar_url, is_admin) = match &user {
        Some(user) => {
            let nick = user
                .nickname
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&user.username)
                .to_string();
            (
                nick,
                None,
                None,
                Some(user.id),
                user.avatar_url.clone(),
                user.is_admin,
            )
        }
        None => {
            let nick = required_text(body.nick.as_deref().unwrap_or(""), "昵称", MAX_NICK_LEN)?;
            (
                nick,
                optional_email(body.email)?,
                optional_website(body.website)?,
                None,
                None,
                false,
            )
        }
    };

    // 回复目标必须存在、未被删除且属于同一页面
    if let Some(parent_id) = body.parent_id {
        let parent = Comments::find_by_id(parent_id)
            .one(&state.db)
            .await?
            .ok_or_else(|| ApiError::Unprocessable("回复的评论不存在".into()))?;
        if parent.deleted_at.is_some() || parent.page != page {
            return Err(ApiError::Unprocessable("回复的评论不存在".into()));
        }
    }

    // 记录来源 IP 与设备信息，用于评论下方的元信息展示。
    // 设备标签由提交端浏览器解析（可识别 Windows 11），缺失时回退原始 UA。
    let ip = client_ip(&headers, addr);
    let user_agent = body
        .device
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(120).collect::<String>())
        .or_else(|| {
            headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.chars().take(300).collect::<String>())
        });

    let inserted = CommentActive {
        page: Set(page),
        nick: Set(nick),
        email: Set(email),
        website: Set(website),
        content: Set(content),
        parent_id: Set(body.parent_id),
        user_id: Set(user_id),
        ip: Set(Some(ip)),
        user_agent: Set(user_agent),
        created_at: Set(utc_now()),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    let region = resolve_region(&state, inserted.ip.as_deref());
    Ok((
        StatusCode::CREATED,
        Json(CommentResponse::from_model(
            inserted, avatar_url, is_admin, region, 0, false,
        )),
    ))
}

/// `DELETE /api/v1/comments/{id}` — 管理员软删除评论及其全部回复。
pub async fn delete_comment(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    let comment = Comments::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("评论不存在".into()))?;
    if comment.deleted_at.is_some() {
        return Ok(StatusCode::NO_CONTENT);
    }

    // 逐层软删除整棵回复树，避免删除父评论后子回复失去归属
    let mut pending = vec![id];
    while !pending.is_empty() {
        let level = std::mem::take(&mut pending);
        Comments::update_many()
            .filter(CommentCol::Id.is_in(level.clone()))
            .filter(CommentCol::DeletedAt.is_null())
            .col_expr(CommentCol::DeletedAt, Expr::value(utc_now()))
            .exec(&state.db)
            .await?;
        pending = Comments::find()
            .filter(CommentCol::ParentId.is_in(level))
            .filter(CommentCol::DeletedAt.is_null())
            .all(&state.db)
            .await?
            .into_iter()
            .map(|child| child.id)
            .collect();
    }

    Ok(StatusCode::NO_CONTENT)
}

/// 点赞响应。
#[derive(Debug, Serialize)]
pub struct LikeResponse {
    pub likes: i64,
    pub liked: bool,
}

/// `POST /api/v1/comments/{id}/like` — 按访客切换点赞状态。
pub async fn toggle_comment_like(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> ApiResult<Json<LikeResponse>> {
    let visitor =
        visitor_id(&headers).ok_or_else(|| ApiError::Unprocessable("缺少访客标识".into()))?;
    let comment = Comments::find_by_id(id)
        .one(&state.db)
        .await?
        .filter(|row| row.deleted_at.is_none())
        .ok_or_else(|| ApiError::NotFound("评论不存在".into()))?;

    crate::routes::talks::ensure_visible(&state.db, &comment.page).await?;
    let existing = CommentLikes::find()
        .filter(LikeCol::CommentId.eq(comment.id))
        .filter(LikeCol::VisitorId.eq(&visitor))
        .one(&state.db)
        .await?;

    let liked = match existing {
        Some(like) => {
            CommentLikes::delete_by_id(like.id).exec(&state.db).await?;
            false
        }
        None => {
            LikeActive {
                comment_id: Set(comment.id),
                visitor_id: Set(visitor),
                created_at: Set(utc_now()),
                ..Default::default()
            }
            .insert(&state.db)
            .await?;
            true
        }
    };

    let likes = CommentLikes::find()
        .filter(LikeCol::CommentId.eq(comment.id))
        .count(&state.db)
        .await? as i64;
    Ok(Json(LikeResponse { likes, liked }))
}

/// `POST /api/v1/comments/upload` — 上传评论配图（GIF 保留动图，其余转 JPEG）。
pub async fn upload_comment_image(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<Json<UploadResponse>> {
    let bytes = uploads::read_upload_field(&mut multipart).await?;
    if bytes.len() > uploads::MAX_IMAGE_BYTES {
        return Err(ApiError::Unprocessable(format!(
            "图片不能超过 {} MB",
            uploads::MAX_IMAGE_BYTES / 1024 / 1024
        )));
    }

    let (file_name, output) = if bytes.starts_with(b"GIF8") {
        (format!("{}.gif", uploads::random_file_name()), bytes)
    } else {
        let source = image::load_from_memory(&bytes)
            .map_err(|_| ApiError::Unprocessable("无法识别图片格式".into()))?;
        let resized = if source.width() > 1200 {
            let height = source.height() * 1200 / source.width();
            source.resize(1200, height.max(1), FilterType::Lanczos3)
        } else {
            source
        };
        let rgb = resized.to_rgb8();
        let mut output = Vec::new();
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, uploads::JPEG_QUALITY);
        encoder
            .encode_image(&rgb)
            .map_err(|err| ApiError::Internal(format!("图片编码失败: {err}")))?;
        (format!("{}.jpg", uploads::random_file_name()), output)
    };

    let dir = state.cfg.data_dir.join("uploads");
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join(&file_name), &output).await?;
    Ok(Json(UploadResponse {
        url: format!("{}/{file_name}", uploads::UPLOAD_URL_PREFIX),
    }))
}
