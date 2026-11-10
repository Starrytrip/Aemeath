use crate::uploads::{is_safe_file_name, random_file_name};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Json, Router,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::error::{ApiError, ApiResult};
const MAX: u64 = 50 * 1024 * 1024;
fn extension(name: &str) -> Option<&'static str> {
    match name.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "mp3" => Some("mp3"),
        "wav" => Some("wav"),
        "flac" => Some("flac"),
        "ogg" => Some("ogg"),
        "m4a" => Some("m4a"),
        "lrc" => Some("lrc"),
        _ => None,
    }
}
fn signature_valid(ext: &str, bytes: &[u8]) -> bool {
    match ext {
        "mp3" => {
            bytes.starts_with(b"ID3")
                || (bytes.len() > 1 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0)
        }
        "wav" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE"),
        "flac" => bytes.starts_with(b"fLaC"),
        "ogg" => bytes.starts_with(b"OggS"),
        "m4a" => bytes.get(4..8) == Some(b"ftyp"),
        _ => false,
    }
}
fn mime(ext: &str) -> &'static str {
    match ext {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        _ => "text/plain; charset=utf-8",
    }
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/music/upload", axum::routing::post(upload))
        .route("/music/media/{file}", axum::routing::get(serve))
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
        let ext = extension(field.file_name().unwrap_or("")).ok_or_else(|| {
            ApiError::Unprocessable("支持 MP3、WAV、FLAC、OGG、M4A 和 LRC 文件".into())
        })?;
        let limit = if ext == "lrc" { 512 * 1024 } else { MAX };
        let name = format!("{}.{}", random_file_name(), ext);
        let dir = state.cfg.data_dir.join("music");
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
                if size > limit {
                    return Err(ApiError::Unprocessable(
                        "音频不能超过50MiB，歌词不能超过512KiB".into(),
                    ));
                }
                if signature.len() < 16 {
                    signature.extend(chunk.iter().take(16 - signature.len()));
                }
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            if ext == "lrc" {
                let bytes = tokio::fs::read(pending.path()).await?;
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| ApiError::Unprocessable("歌词需要 UTF-8 编码".into()))?;
                if !text
                    .trim_start_matches('\u{feff}')
                    .trim_start()
                    .starts_with('[')
                {
                    return Err(ApiError::Unprocessable("请选择有效的 LRC 歌词".into()));
                }
            } else if !signature_valid(ext, &signature) {
                return Err(ApiError::Unprocessable("音频格式无效".into()));
            }
            Ok::<(), ApiError>(())
        }
        .await?;
        file.sync_all().await?;
        drop(file);
        pending.persist(&path).map_err(|error| error.error)?;
        return Ok(Json(
            serde_json::json!({"url":format!("/api/v1/music/media/{name}"),"bytes":size}),
        ));
    }
    Err(ApiError::BadRequest("请选择音频或歌词".into()))
}
async fn serve(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    if !is_safe_file_name(&name) || extension(&name).is_none() {
        return Err(ApiError::NotFound("音乐文件不存在".into()));
    }
    let mut file = tokio::fs::File::open(state.cfg.data_dir.join("music").join(&name))
        .await
        .map_err(|_| ApiError::NotFound("音乐文件不存在".into()))?;
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
        .header(header::CONTENT_TYPE, mime(extension(&name).unwrap()))
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header("x-content-type-options", "nosniff")
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
