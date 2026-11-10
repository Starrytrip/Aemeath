//! 评论路由声明 — 业务逻辑见 `crate::handler::comment`。

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post};
use axum::Router;

use crate::handler::comment::{
    create_comment, delete_comment, list_comments, toggle_comment_like, upload_comment_image,
};
use yukina_runtime::state::AppState;

/// `/api/v1` 下的评论 / 留言路由。
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/comments", get(list_comments).post(create_comment))
        .route(
            "/comments/upload",
            post(upload_comment_image).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/comments/{id}", delete(delete_comment))
        .route("/comments/{id}/like", post(toggle_comment_like))
}
