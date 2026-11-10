//! 后端业务路由。公开前缀、探活与 HTTP 中间件由 server 内的 app 模块统一组装。

pub mod admin_comments;
pub mod admin_users;
pub mod analytics;
pub mod article_summary;
pub mod auth;
pub mod comment;
pub mod content;
pub mod feed;
pub mod friends_feed;
pub mod gallery;
pub mod media;
pub mod profile;
pub mod site;
pub mod stats;
pub mod talks;
pub mod videos;

use axum::Router;
use yukina_runtime::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(site::router())
        .merge(feed::router())
        .merge(admin_comments::router())
        .merge(analytics::router())
        .merge(admin_users::router())
        .merge(article_summary::router())
        .merge(comment::router())
        .merge(profile::router())
        .merge(stats::router())
        .merge(media::router())
        .merge(content::router())
        .merge(gallery::router())
        .merge(talks::router())
        .merge(videos::router())
        .merge(yukina_media_service::music::router())
        .merge(crate::documents::router())
        .merge(crate::search::router())
}
