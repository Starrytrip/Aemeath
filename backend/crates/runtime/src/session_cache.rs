//! 刷新会话内存缓存（单实例）
//!
//! 会话不落库：后端重启后刷新令牌全部失效，用户重新登录即可；
//! 多实例部署时各实例不共享会话，需要横向扩展时应换成 Redis 等共享存储。
//! 访问令牌仍是短命 JWT，每个请求只验签不查会话。

use chrono::NaiveDateTime;
use dashmap::DashMap;

use yukina_shared::serial::utc_now;

struct Entry {
    user_id: i64,
    auth_version: i64,
    expires_at: NaiveDateTime,
}

/// 按刷新令牌的 SHA-256 摘要存放会话。
#[derive(Default)]
pub struct SessionCache {
    entries: DashMap<String, Entry>,
}

impl SessionCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 写入刷新会话。
    pub fn store(
        &self,
        token_hash: String,
        user_id: i64,
        auth_version: i64,
        expires_at: NaiveDateTime,
    ) {
        self.entries.insert(
            token_hash,
            Entry {
                user_id,
                auth_version,
                expires_at,
            },
        );
    }

    /// 取出并作废会话（刷新即轮换），返回 user_id；过期或不存在返回 None。
    pub fn consume(&self, token_hash: &str, now: NaiveDateTime) -> Option<(i64, i64)> {
        let (_, entry) = self.entries.remove(token_hash)?;
        (entry.expires_at > now).then_some((entry.user_id, entry.auth_version))
    }

    /// 删除单个会话（退出登录）。
    pub fn remove(&self, token_hash: &str) {
        self.entries.remove(token_hash);
    }

    /// 撤销某个用户的全部会话（重置密码后强制下线）。
    pub fn revoke_user(&self, user_id: i64) {
        self.entries.retain(|_, entry| entry.user_id != user_id);
    }

    /// 清理过期会话，避免长期运行后状态膨胀。
    pub fn retain_recent(&self) {
        let now = utc_now();
        self.entries.retain(|_, entry| entry.expires_at > now);
    }
}
