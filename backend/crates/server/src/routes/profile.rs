//! 个人资料路由声明 — 业务逻辑见 `crate::handler::profile`。

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, patch, post};
use axum::Router;

use crate::handler::profile::{serve_upload, update_profile, upload_avatar, upload_cover};
use yukina_runtime::state::AppState;

/// 上传接口的请求体上限（含 multipart 边界开销）。
const UPLOAD_BODY_LIMIT: usize = 10 * 1024 * 1024;

/// `/api/v1` 下的个人资料与上传文件路由。
pub fn router() -> Router<AppState> {
    let uploads = Router::new()
        .route("/profile/avatar", post(upload_avatar))
        .route("/profile/cover", post(upload_cover))
        .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT));

    Router::new()
        .route("/profile", patch(update_profile))
        .route("/uploads/{file}", get(serve_upload))
        .merge(uploads)
}
