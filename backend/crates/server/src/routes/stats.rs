//! 统计路由声明 — 业务逻辑见 `crate::handler::stats`。

use axum::routing::{get, post};
use axum::Router;

use crate::handler::stats::{add_view, get_stats, toggle_like};
use yukina_runtime::state::AppState;

/// `/api/v1` 下的阅读量 / 点赞路由。
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stats", get(get_stats))
        .route("/stats/views", post(add_view))
        .route("/stats/like", post(toggle_like))
}
