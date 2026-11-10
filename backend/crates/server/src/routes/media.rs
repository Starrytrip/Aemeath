//! 管理员图库：独立记录后台上传的图片，避免误删头像或评论配图。
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    routing::{get, post},
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use std::io::Cursor;
use yukina_media_service::uploads::{random_file_name, MAX_IMAGE_BYTES, UPLOAD_URL_PREFIX};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::{
    error::{ApiError, ApiResult},
    serial::utc_now,
};
use yukina_store::entities::media_image::{self, Column, Entity as Images};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/images", get(list).post(upload))
        .route("/admin/images/{id}", axum::routing::delete(remove))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .route(
            "/admin/images/import",
            post(import_original).layer(DefaultBodyLimit::max(34 * 1024 * 1024)),
        )
}

async fn import_original(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    mut multipart: Multipart,
) -> ApiResult<Json<media_image::Model>> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::BadRequest("上传格式不正确".into()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let name = field
            .file_name()
            .unwrap_or("image")
            .chars()
            .take(200)
            .collect::<String>();
        let bytes = field
            .bytes()
            .await
            .map_err(|_| ApiError::BadRequest("读取图片失败".into()))?
            .to_vec();
        return Ok(Json(
            store_original(&state.db, &state.cfg.data_dir, name, bytes).await?,
        ));
    }
    Err(ApiError::BadRequest("请选择图片".into()))
}

use yukina_media_service::images::store_original;

async fn list(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> ApiResult<Json<Vec<media_image::Model>>> {
    Ok(Json(
        Images::find()
            .order_by_desc(Column::Id)
            .limit(200)
            .all(&state.db)
            .await?,
    ))
}

async fn upload(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    mut multipart: Multipart,
) -> ApiResult<Json<media_image::Model>> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::BadRequest("上传格式不正确".into()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let name = field
            .file_name()
            .unwrap_or("image")
            .chars()
            .take(200)
            .collect::<String>();
        let bytes = field
            .bytes()
            .await
            .map_err(|_| ApiError::BadRequest("读取图片失败".into()))?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(ApiError::Unprocessable("图片不能超过 8 MiB".into()));
        }
        let (output, width, height) = tokio::task::spawn_blocking(move || -> ApiResult<_> {
            let mut reader = image::ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()
                .map_err(|_| ApiError::Unprocessable("无法识别图片".into()))?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(12000);
            limits.max_image_height = Some(12000);
            limits.max_alloc = Some(64 * 1024 * 1024);
            reader.limits(limits);
            let image = reader
                .decode()
                .map_err(|_| ApiError::Unprocessable("图片无效或尺寸过大".into()))?;
            let image = if image.width() > 4096 || image.height() > 4096 {
                image.resize(4096, 4096, image::imageops::FilterType::Lanczos3)
            } else {
                image
            }
            .to_rgb8();
            let mut output = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 85)
                .encode_image(&image)
                .map_err(|_| ApiError::Internal("图片处理失败".into()))?;
            Ok((output, image.width(), image.height()))
        })
        .await
        .map_err(|_| ApiError::Internal("图片处理任务失败".into()))??;
        let file_name = format!("media-{}.jpg", random_file_name());
        let dir = state.cfg.data_dir.join("uploads");
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(&file_name);
        tokio::fs::write(&path, &output).await?;
        let saved = media_image::ActiveModel {
            name: Set(name),
            url: Set(format!("{UPLOAD_URL_PREFIX}/{file_name}")),
            file_name: Set(file_name),
            width: Set(width.into()),
            height: Set(height.into()),
            bytes: Set(output.len() as i64),
            created_at: Set(utc_now()),
            ..Default::default()
        }
        .insert(&state.db)
        .await;
        match saved {
            Ok(row) => return Ok(Json(row)),
            Err(error) => {
                let _ = tokio::fs::remove_file(path).await;
                return Err(error.into());
            }
        }
    }
    Err(ApiError::BadRequest("请选择图片".into()))
}

async fn remove(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let row = Images::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("图片不存在".into()))?;
    if !yukina_media_service::uploads::is_safe_file_name(&row.file_name)
        || !row.file_name.starts_with("media-")
    {
        return Err(ApiError::BadRequest("图片路径无效".into()));
    }
    use yukina_store::entities::{background, comment, document, gallery_album, talk, user};
    let referenced = document::Entity::find()
        .filter(
            Condition::any()
                .add(document::Column::Markdown.contains(&row.url))
                .add(document::Column::CoverUrl.eq(&row.url)),
        )
        .count(&state.db)
        .await?
        > 0
        || background::Entity::find()
            .filter(
                Condition::any()
                    .add(background::Column::Url.eq(&row.url))
                    .add(background::Column::PosterUrl.eq(&row.url)),
            )
            .count(&state.db)
            .await?
            > 0
        || gallery_album::Entity::find()
            .filter(
                Condition::any()
                    .add(gallery_album::Column::Cover.eq(&row.url))
                    .add(gallery_album::Column::Photos.contains(&row.url)),
            )
            .count(&state.db)
            .await?
            > 0
        || talk::Entity::find()
            .filter(talk::Column::Images.contains(&row.url))
            .count(&state.db)
            .await?
            > 0
        || comment::Entity::find()
            .filter(comment::Column::Content.contains(&row.url))
            .count(&state.db)
            .await?
            > 0
        || user::Entity::find()
            .filter(
                Condition::any()
                    .add(user::Column::AvatarUrl.eq(&row.url))
                    .add(user::Column::CoverUrl.eq(&row.url)),
            )
            .count(&state.db)
            .await?
            > 0;
    let referenced = referenced
        || yukina_store::entities::site_entry::Entity::find()
            .filter(yukina_store::entities::site_entry::Column::Data.contains(&row.url))
            .count(&state.db)
            .await?
            > 0;
    if referenced {
        return Err(ApiError::Conflict(
            "图片仍被文章或站点内容引用，请先移除引用".into(),
        ));
    }
    match tokio::fs::remove_file(state.cfg.data_dir.join("uploads").join(&row.file_name)).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Images::delete_by_id(id).exec(&state.db).await?;
    Ok(Json(serde_json::json!({"deleted": true})))
}
