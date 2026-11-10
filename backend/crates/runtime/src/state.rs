//! 应用共享状态

use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::code_cache::EmailCodeCache;
use crate::middleware::rate_limit::IpLimiter;
use crate::session_cache::SessionCache;
use crate::user_cache::UserCache;
use yukina_shared::config::Settings;

/// 所有 handler 通过 `State<AppState>` 拿到共享资源。
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub embeddings: Arc<yukina_embedding::Embedder>,
    pub cfg: Arc<Settings>,
    pub api_limiter: Arc<IpLimiter>,
    pub login_limiter: Arc<IpLimiter>,
    /// 邮箱验证码内存缓存
    pub email_codes: Arc<EmailCodeCache>,
    /// 刷新会话内存缓存
    pub sessions: Arc<SessionCache>,
    /// 用户信息短 TTL 缓存
    pub users: Arc<UserCache>,
    /// 所有路由与请求共享一份 xdb 数据，避免克隆状态时复制整个数据库。
    pub ip_region: Option<Arc<ip2region::Searcher>>,
}
