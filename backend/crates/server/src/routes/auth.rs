//! 认证路由声明 — 业务逻辑见 `crate::handler::auth`。

use axum::routing::{get, post};
use axum::Router;

use crate::handler::auth::{
    get_me, login, logout, refresh_session, register, reset_password, send_email_code,
};
use yukina_runtime::state::AppState;

/// `/api/v1/auth` 下的路由。
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh_session))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(get_me))
        .route("/auth/email/code", post(send_email_code))
        .route("/auth/password/reset", post(reset_password))
}
