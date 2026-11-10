//! 个人资料：资料更新、头像 / 封面上传与图片读取
//!
//! 上传的图片统一解码后重编码为 JPEG，写入 `data/uploads`，数据库只保存访问 URL；
//! 这样既剥离了原文件中的潜在恶意内容，也避免依赖原图格式与尺寸。

use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::Json;
use chrono::NaiveDate;
use image::imageops::FilterType;
use sea_orm::ActiveModelTrait;
use serde::Deserialize;

use yukina_media_service::uploads::{
    is_safe_file_name, random_file_name, read_upload_field, remove_uploaded, UploadResponse,
    JPEG_QUALITY, MAX_IMAGE_BYTES, UPLOAD_URL_PREFIX,
};
use yukina_runtime::auth::deps::CurrentUser;
use yukina_runtime::dto::UserResponse;
use yukina_runtime::state::AppState;
use yukina_shared::error::{ApiError, ApiResult};
use yukina_shared::serial::utc_now;
use yukina_store::entities::user::{ActiveModel as UserActive, Model as UserModel};

/// 头像输出尺寸（正方形）。
const AVATAR_SIZE: u32 = 256;
/// 封面输出尺寸（3:1）。
const COVER_WIDTH: u32 = 1200;
const COVER_HEIGHT: u32 = 400;

/// 资料更新请求：字段缺省表示不修改，传空字符串表示清空。
#[derive(Debug, Deserialize)]
pub struct ProfileUpdateRequest {
    pub nickname: Option<String>,
    pub gender: Option<String>,
    pub birthday: Option<String>,
    pub job: Option<String>,
    pub hobbies: Option<String>,
    pub motto: Option<String>,
    pub bio: Option<String>,
}

/// 白名单字段：去空格、限长，空字符串视为清空。
fn clean_text(raw: String, max_chars: usize, label: &str) -> ApiResult<Option<String>> {
    let value = raw.trim().to_string();
    if value.chars().count() > max_chars {
        return Err(ApiError::Unprocessable(format!(
            "{label}最多 {max_chars} 个字符"
        )));
    }
    Ok((!value.is_empty()).then_some(value))
}

/// 校验并归一化性别。
fn clean_gender(raw: String) -> ApiResult<Option<String>> {
    let value = raw.trim().to_lowercase();
    match value.as_str() {
        "" => Ok(None),
        "male" | "female" | "other" => Ok(Some(value)),
        _ => Err(ApiError::Unprocessable("性别取值不正确".into())),
    }
}

/// 校验生日格式（YYYY-MM-DD），空字符串视为清空。
fn clean_birthday(raw: String) -> ApiResult<Option<NaiveDate>> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| ApiError::Unprocessable("生日格式需为 YYYY-MM-DD".into()))
}

/// 更新当前用户资料，返回完整资料。
pub async fn update_profile(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<ProfileUpdateRequest>,
) -> ApiResult<Json<UserResponse>> {
    let mut active = UserActive {
        id: sea_orm::ActiveValue::Set(user.id),
        updated_at: sea_orm::ActiveValue::Set(utc_now()),
        ..Default::default()
    };

    if let Some(value) = body.nickname {
        active.nickname = sea_orm::ActiveValue::Set(clean_text(value, 20, "昵称")?);
    }
    if let Some(value) = body.gender {
        active.gender = sea_orm::ActiveValue::Set(clean_gender(value)?);
    }
    if let Some(value) = body.birthday {
        active.birthday = sea_orm::ActiveValue::Set(clean_birthday(value)?);
    }
    if let Some(value) = body.job {
        active.job = sea_orm::ActiveValue::Set(clean_text(value, 40, "职位")?);
    }
    if let Some(value) = body.hobbies {
        active.hobbies = sea_orm::ActiveValue::Set(clean_text(value, 100, "爱好")?);
    }
    if let Some(value) = body.motto {
        active.motto = sea_orm::ActiveValue::Set(clean_text(value, 60, "格言")?);
    }
    if let Some(value) = body.bio {
        active.bio = sea_orm::ActiveValue::Set(clean_text(value, 200, "个人简介")?);
    }

    let updated = active.update(&state.db).await?;
    state.users.store(updated.clone());
    Ok(Json(UserResponse::from_model(&updated)))
}

/// 上传头像。
pub async fn upload_avatar(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    multipart: Multipart,
) -> ApiResult<Json<UploadResponse>> {
    upload_image(state, user, multipart, ImageKind::Avatar).await
}

/// 上传封面。
pub async fn upload_cover(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    multipart: Multipart,
) -> ApiResult<Json<UploadResponse>> {
    upload_image(state, user, multipart, ImageKind::Cover).await
}

/// 读取上传后的图片（头像 / 封面共用）。
pub async fn serve_upload(
    State(state): State<AppState>,
    Path(file): Path<String>,
) -> ApiResult<Response> {
    if !is_safe_file_name(&file) {
        return Err(ApiError::NotFound("文件不存在".into()));
    }
    let path = state.cfg.data_dir.join("uploads").join(&file);
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return Err(ApiError::NotFound("文件不存在".into()));
    };

    let content_type = match std::path::Path::new(&file)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "gif" => "image/gif",
        "png" => "image/png",
        "webp" => "image/webp",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return Err(ApiError::NotFound("文件不存在".into())),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(bytes))
        .map_err(|err| ApiError::Internal(format!("构造图片响应失败: {err}")))
}

/// 图片用途。
#[derive(Clone, Copy)]
enum ImageKind {
    Avatar,
    Cover,
}

impl ImageKind {
    /// 输出边长 / 尺寸说明。
    fn label(self) -> &'static str {
        match self {
            ImageKind::Avatar => "头像",
            ImageKind::Cover => "封面",
        }
    }
}

/// 从 multipart 中取出 file 字段并按用途处理。
async fn upload_image(
    state: AppState,
    user: UserModel,
    mut multipart: Multipart,
    kind: ImageKind,
) -> ApiResult<Json<UploadResponse>> {
    let bytes = read_upload_field(&mut multipart).await?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ApiError::Unprocessable(format!(
            "{}图片不能超过 {} MB",
            kind.label(),
            MAX_IMAGE_BYTES / 1024 / 1024
        )));
    }

    let source = image::load_from_memory(&bytes)
        .map_err(|_| ApiError::Unprocessable("无法识别图片格式".into()))?;
    let (width, height) = match kind {
        ImageKind::Avatar => (AVATAR_SIZE, AVATAR_SIZE),
        ImageKind::Cover => (COVER_WIDTH, COVER_HEIGHT),
    };
    let resized = source.resize_to_fill(width, height, FilterType::Lanczos3);
    let rgb = resized.to_rgb8();

    let mut output = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, JPEG_QUALITY);
    encoder
        .encode_image(&rgb)
        .map_err(|err| ApiError::Internal(format!("图片编码失败: {err}")))?;

    let dir = state.cfg.data_dir.join("uploads");
    tokio::fs::create_dir_all(&dir).await?;
    let file_name = format!("{}.jpg", random_file_name());
    tokio::fs::write(dir.join(&file_name), &output).await?;
    let url = format!("{UPLOAD_URL_PREFIX}/{file_name}");

    let now = utc_now();
    let mut active = UserActive {
        id: sea_orm::ActiveValue::Set(user.id),
        updated_at: sea_orm::ActiveValue::Set(now),
        ..Default::default()
    };
    let old_url = match kind {
        ImageKind::Avatar => {
            active.avatar_url = sea_orm::ActiveValue::Set(Some(url.clone()));
            user.avatar_url.clone()
        }
        ImageKind::Cover => {
            active.cover_url = sea_orm::ActiveValue::Set(Some(url.clone()));
            user.cover_url.clone()
        }
    };
    let updated = active.update(&state.db).await?;
    state.users.store(updated);

    // 新图已落库后再清理旧文件，失败也不影响本次上传
    if let Some(old_url) = old_url {
        remove_uploaded(&dir, &old_url).await;
    }

    Ok(Json(UploadResponse { url }))
}
