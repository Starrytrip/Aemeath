use crate::uploads::UPLOAD_URL_PREFIX;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use std::io::Cursor;
use yukina_shared::{
    error::{ApiError, ApiResult},
    serial::utc_now,
};
use yukina_store::entities::media_image::{self, Column, Entity as Images};
/// 原图按内容寻址，保留透明度、动图和原始格式，供文章迁移和同步复用。
pub async fn store_original(
    db: &impl ConnectionTrait,
    data_dir: &std::path::Path,
    name: String,
    bytes: Vec<u8>,
) -> ApiResult<media_image::Model> {
    use sha2::{Digest, Sha256};
    if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
        return Err(ApiError::Unprocessable(
            "原图不能为空，且不能超过 32 MiB".into(),
        ));
    }
    let hash = hex::encode(Sha256::digest(&bytes));
    let (bytes, width, height, extension) = tokio::task::spawn_blocking(move || -> ApiResult<_> {
        let mut reader = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(|_| ApiError::Unprocessable("无法识别图片".into()))?;
        let extension = match reader.format() {
            Some(image::ImageFormat::Png) => "png",
            Some(image::ImageFormat::Jpeg) => "jpg",
            Some(image::ImageFormat::WebP) => "webp",
            Some(image::ImageFormat::Gif) => "gif",
            _ => {
                return Err(ApiError::Unprocessable(
                    "原图支持 PNG、JPEG、WebP 和 GIF".into(),
                ))
            }
        };
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(12000);
        limits.max_image_height = Some(12000);
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader
            .decode()
            .map_err(|_| ApiError::Unprocessable("图片无效或尺寸过大".into()))?;
        Ok((bytes, decoded.width(), decoded.height(), extension))
    })
    .await
    .map_err(|_| ApiError::Internal("图片处理任务失败".into()))??;
    let file_name = format!("media-{hash}.{extension}");
    let dir = data_dir.join("uploads");
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join(&file_name), &bytes).await?;
    if let Some(existing) = Images::find()
        .filter(Column::FileName.eq(&file_name))
        .one(db)
        .await?
    {
        return Ok(existing);
    }
    // 并发重复上传也只创建一条素材记录。
    Images::insert(media_image::ActiveModel {
        name: Set(name),
        url: Set(format!("{UPLOAD_URL_PREFIX}/{file_name}")),
        file_name: Set(file_name.clone()),
        width: Set(width.into()),
        height: Set(height.into()),
        bytes: Set(bytes.len() as i64),
        created_at: Set(utc_now()),
        ..Default::default()
    })
    .on_conflict(
        sea_orm::sea_query::OnConflict::column(Column::FileName)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    Images::find()
        .filter(Column::FileName.eq(file_name))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::Internal("图片保存失败".into()))
}
