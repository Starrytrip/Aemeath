use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Json, Router,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use yukina_media_service::uploads::{is_safe_file_name, random_file_name};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::error::{ApiError, ApiResult};
const MAX: u64 = 50 * 1024 * 1024;
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/videos", axum::routing::post(upload))
        .route("/backgrounds/media/{file}", axum::routing::get(serve))
        .layer(DefaultBodyLimit::max((MAX + 1024 * 1024) as usize))
}
async fn upload(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::BadRequest("上传格式错误".into()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let mime = field.content_type().unwrap_or("").to_owned();
        let ext = match mime.as_str() {
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            _ => return Err(ApiError::Unprocessable("仅支持MP4和WebM".into())),
        };
        let name = format!("{}.{}", random_file_name(), ext);
        let dir = state.cfg.data_dir.join("backgrounds");
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(&name);
        let pending = tempfile::NamedTempFile::new_in(&dir)?;
        let mut file = tokio::fs::File::from_std(pending.reopen()?);
        let mut size = 0;
        let mut signature = Vec::new();
        async {
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|_| ApiError::BadRequest("上传中断".into()))?
            {
                size += chunk.len() as u64;
                if size > MAX {
                    return Err(ApiError::Unprocessable("视频不能超过50MiB".into()));
                }
                if signature.len() < 16 {
                    signature.extend(chunk.iter().take(16 - signature.len()));
                }
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            if (ext == "mp4" && !signature.get(4..8).is_some_and(|s| s == b"ftyp"))
                || (ext == "webm" && !signature.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]))
            {
                return Err(ApiError::Unprocessable("视频格式无效".into()));
            }
            Ok::<(), ApiError>(())
        }
        .await?;
        file.sync_all().await?;
        drop(file);
        pending.persist(&path).map_err(|error| error.error)?;
        return Ok(Json(
            serde_json::json!({"url":format!("/api/v1/backgrounds/media/{name}"),"bytes":size}),
        ));
    }
    Err(ApiError::BadRequest("请选择视频".into()))
}
async fn serve(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    if !is_safe_file_name(&name) || !(name.ends_with(".mp4") || name.ends_with(".webm")) {
        return Err(ApiError::NotFound("视频不存在".into()));
    }
    let mut file = tokio::fs::File::open(state.cfg.data_dir.join("backgrounds").join(&name))
        .await
        .map_err(|_| ApiError::NotFound("视频不存在".into()))?;
    let total = file.metadata().await?.len();
    let mut start = 0;
    let mut end = total.saturating_sub(1);
    let mut status = StatusCode::OK;
    if let Some(value) = headers.get(header::RANGE) {
        let parsed = value
            .to_str()
            .ok()
            .and_then(|s| s.strip_prefix("bytes="))
            .filter(|s| !s.contains(','))
            .and_then(|s| s.split_once('-'))
            .and_then(|(a, b)| {
                if a.is_empty() {
                    let n = b.parse::<u64>().ok()?;
                    if n == 0 {
                        return None;
                    }
                    Some((total.saturating_sub(n), total.saturating_sub(1)))
                } else {
                    Some((
                        a.parse::<u64>().ok()?,
                        if b.is_empty() {
                            total.saturating_sub(1)
                        } else {
                            b.parse::<u64>().ok()?.min(total.saturating_sub(1))
                        },
                    ))
                }
            });
        let Some((a, b)) = parsed.filter(|(a, b)| a <= b && *a < total) else {
            return Ok(Response::builder()
                .status(416)
                .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                .body(Body::empty())
                .unwrap());
        };
        start = a;
        end = b;
        status = StatusCode::PARTIAL_CONTENT;
    }
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let length = if total == 0 { 0 } else { end - start + 1 };
    let mut response = Response::builder()
        .status(status)
        .header(
            header::CONTENT_TYPE,
            if name.ends_with(".mp4") {
                "video/mp4"
            } else {
                "video/webm"
            },
        )
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, length);
    if status == StatusCode::PARTIAL_CONTENT {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }
    Ok(response
        .body(Body::from_stream(tokio_util::io::ReaderStream::new(
            file.take(length),
        )))
        .unwrap())
}
