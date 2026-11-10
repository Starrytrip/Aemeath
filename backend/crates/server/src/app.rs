//! HTTP 应用组装：CORS、限流、日志和路由的唯一入口。

use axum::{extract::State, routing::get, Json};
use axum::{http::HeaderValue, middleware::from_fn_with_state, Router};
use std::time::Duration;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use yukina_runtime::{middleware, state::AppState};
use yukina_shared::error::ApiResult;

pub fn router(state: AppState) -> Router {
    // CORS（指定来源 + 凭证）
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(
            state
                .cfg
                .cors_origins
                .iter()
                .filter_map(|o| o.parse::<HeaderValue>().ok()),
        ))
        .allow_credentials(true)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::PATCH,
            axum::http::Method::HEAD,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::header::ORIGIN,
            axum::http::header::CACHE_CONTROL,
            // 访客标识（阅读量 / 点赞去重）
            axum::http::header::HeaderName::from_static("x-visitor-id"),
            axum::http::header::HeaderName::from_static("x-comment-email"),
        ])
        .max_age(Duration::from_secs(3600));

    api_router()
        .layer(from_fn_with_state(
            state.clone(),
            middleware::rate_limit::api_rate_limit,
        ))
        // 在限流层之后合并同步路由；仍共用 CORS、日志和管理员鉴权。
        .merge(Router::new().nest("/api/v1", crate::documents::sync_router()))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// 汇总站点入口与业务 API，保持统一的公开路径。
pub(crate) fn api_router() -> Router<AppState> {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .nest("/api/v1", crate::routes::router())
}

/// `GET /` — 存活提示。
async fn root() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "message": "Yukina Blog API is running" }))
}

/// `GET /health` — 含数据库探活。
async fn health(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    use sea_orm::ConnectionTrait;
    state
        .db
        .query_one(sea_orm::Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            "SELECT 1",
        ))
        .await?;
    Ok(Json(
        serde_json::json!({ "status": "ok", "database": "ok" }),
    ))
}
