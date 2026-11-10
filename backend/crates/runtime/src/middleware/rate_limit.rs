//! 全局限流 — 基于 governor 的 GCRA 令牌桶，按客户端 IP 独立计数。
//!
//! 两层额度：
//! - 业务 API 共用额度，图片文件的 GET / HEAD 不占用此额度
//! - 登录接口额外收紧：10 次/15 分钟（突发 10）

use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::time::Duration;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::clock::{Clock, DefaultClock};
use governor::{DefaultKeyedRateLimiter, Quota};

use crate::state::AppState;
use yukina_shared::config::RateLimitConfig;
use yukina_shared::error::ApiError;

/// 敏感接口路径：登录、注册、发送验证码、重置密码额外叠加登录专项额度。
const SENSITIVE_PATHS: [&str; 4] = [
    "/api/v1/auth/login",
    "/api/v1/auth/register",
    "/api/v1/auth/email/code",
    "/api/v1/auth/password/reset",
];

/// 每 IP 一个独立令牌桶。
pub struct IpLimiter {
    inner: DefaultKeyedRateLimiter<String>,
}

impl IpLimiter {
    fn new(quota: Quota) -> Self {
        Self {
            inner: DefaultKeyedRateLimiter::keyed(quota),
        }
    }

    /// 消耗一个令牌；超限时返回建议的重试等待秒数。
    pub fn check(&self, ip: &str) -> Result<(), u64> {
        self.inner.check_key(&ip.to_owned()).map_err(|not_until| {
            let wait = not_until.wait_time_from(DefaultClock::default().now());
            wait.as_secs().max(1)
        })
    }

    /// 清理已恢复满额的空闲 key，避免长期运行后状态膨胀。
    pub fn retain_recent(&self) {
        self.inner.retain_recent();
        self.inner.shrink_to_fit();
    }
}

fn non_zero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("限流参数必须大于 0")
}

/// 全站 API 通用限流器。
pub fn api_limiter(cfg: &RateLimitConfig) -> IpLimiter {
    let quota =
        Quota::per_second(non_zero(cfg.api_per_second)).allow_burst(non_zero(cfg.api_burst));
    IpLimiter::new(quota)
}

/// 登录接口专用限流器。
pub fn login_limiter(cfg: &RateLimitConfig) -> IpLimiter {
    let quota = Quota::with_period(Duration::from_secs(cfg.sensitive_refill_seconds))
        .expect("限流周期必须大于 0")
        .allow_burst(non_zero(cfg.sensitive_burst));
    IpLimiter::new(quota)
}

/// 业务 API 走通用额度，登录接口再叠加登录额度。
pub async fn api_rate_limit(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    // 图库、相册和文章会同时读取多张图片，不能与业务 API 共用突发额度。
    // 仅豁免单个图片文件的读取；上传、删除和其他 API 仍保留限流。
    if matches!(*request.method(), Method::GET | Method::HEAD)
        && path
            .strip_prefix("/api/v1/uploads/")
            .is_some_and(yukina_shared::media_names::is_safe_file_name)
    {
        return next.run(request).await;
    }
    // 预检请求交给 CORS 层，不占用额度
    if path.starts_with("/") && request.method() != Method::OPTIONS {
        let ip = client_ip(&request, addr);
        if let Err(retry_after) = state.api_limiter.check(&ip) {
            tracing::warn!(%ip, path, "API 请求超过限流阈值");
            return ApiError::too_many(retry_after).into_response();
        }

        if request.method() == Method::POST && SENSITIVE_PATHS.contains(&path) {
            if let Err(retry_after) = state.login_limiter.check(&ip) {
                tracing::warn!(%ip, path, "认证请求超过限流阈值");
                return ApiError::too_many(retry_after).into_response();
            }
        }
    }

    next.run(request).await
}

/// 取客户端 IP：优先代理头，其次 TCP 连接地址。
///
/// 服务默认只监听 127.0.0.1，由 Nginx 反代注入 `X-Forwarded-For`，
/// 因此信任这两个头；若直接对公网暴露需改为只信可信代理来源。
fn client_ip(request: &Request, addr: SocketAddr) -> String {
    if let Some(ip) = forwarded_ip(request, "x-forwarded-for") {
        return ip;
    }
    if let Some(ip) = forwarded_ip(request, "x-real-ip") {
        return ip;
    }
    addr.ip().to_string()
}

fn forwarded_ip(request: &Request, header: &str) -> Option<String> {
    let value = request.headers().get(header)?.to_str().ok()?;
    let ip = value.split(',').next()?.trim();
    (!ip.is_empty()).then(|| ip.to_owned())
}
