//! 上传文件的公共协议、命名与清理，供个人资料和评论处理器共用。

use axum::extract::Multipart;
use serde::Serialize;
use yukina_shared::error::{ApiError, ApiResult};

pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const JPEG_QUALITY: u8 = 85;
pub const UPLOAD_URL_PREFIX: &str = "/api/v1/uploads";

/// 图片上传响应。
#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub url: String,
}

/// 读取 multipart 中的 `file` 字段。
pub async fn read_upload_field(multipart: &mut Multipart) -> ApiResult<Vec<u8>> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::BadRequest(format!("上传内容解析失败: {err}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|err| ApiError::BadRequest(format!("读取上传文件失败: {err}")))?;
        return Ok(bytes.to_vec());
    }
    Err(ApiError::BadRequest("未找到上传文件".into()))
}

/// 生成 32 位十六进制随机文件名（不带扩展名）。
pub use yukina_shared::media_names::{is_safe_file_name, random_file_name};

/// 删除本站上传的旧文件；外链或非本站地址直接跳过。
pub async fn remove_uploaded(dir: &std::path::Path, url: &str) {
    let Some(file) = url.strip_prefix(&format!("{UPLOAD_URL_PREFIX}/")) else {
        return;
    };
    if !is_safe_file_name(file) {
        return;
    }
    let path = dir.join(file);
    if let Err(err) = tokio::fs::remove_file(&path).await {
        if err.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(%err, "删除旧上传文件失败");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_names_cannot_escape_the_upload_directory() {
        for name in [
            "",
            "..",
            "../avatar.jpg",
            "a/cover.jpg",
            "a\\cover.jpg",
            "C:avatar.jpg",
            "%2e%2e.jpg",
        ] {
            assert!(!is_safe_file_name(name), "accepted unsafe name: {name}");
        }
        assert!(is_safe_file_name("012abc-avatar_1.jpg"));
    }

    #[tokio::test]
    async fn cleanup_never_deletes_external_or_traversal_urls() {
        let dir = std::env::temp_dir().join(format!("yukina-upload-test-{}", random_file_name()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let file = dir.join("keep.jpg");
        tokio::fs::write(&file, b"test").await.unwrap();
        remove_uploaded(&dir, "https://example.com/api/v1/uploads/keep.jpg").await;
        remove_uploaded(&dir, "/api/v1/uploads/../keep.jpg").await;
        assert!(file.exists());
        remove_uploaded(&dir, "/api/v1/uploads/keep.jpg").await;
        assert!(!file.exists());
        // Deleting an already missing upload remains harmless.
        remove_uploaded(&dir, "/api/v1/uploads/keep.jpg").await;
        tokio::fs::remove_dir(&dir).await.unwrap();
    }
}
