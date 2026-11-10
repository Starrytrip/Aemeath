//! 用户信息内存缓存（短 TTL）
//!
//! 每个已登录请求都要读取用户并校验禁用状态。这里把查询结果缓存 30 秒，
//! 减少完整用户行的读取；启用状态、角色和令牌版本仍实时查库，管理员修改后也会清除此缓存。

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{Duration, NaiveDateTime};

use yukina_shared::serial::utc_now;
use yukina_store::entities::user::Model as UserModel;

struct Entry {
    user: UserModel,
    expires_at: NaiveDateTime,
}

/// 按用户 id 缓存 `users` 行；TTL 由配置决定。
pub struct UserCache {
    entries: Mutex<HashMap<i64, Entry>>,
    ttl_seconds: i64,
}

impl UserCache {
    pub fn new(ttl_seconds: i64) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl_seconds,
        }
    }

    /// 命中且未过期则返回用户，过期条目顺手删除。
    pub fn get(&self, user_id: i64) -> Option<UserModel> {
        let mut entries = self.entries.lock().ok()?;
        let expired = entries
            .get(&user_id)
            .is_some_and(|entry| entry.expires_at <= utc_now());
        if expired {
            entries.remove(&user_id);
            return None;
        }
        entries.get(&user_id).map(|entry| entry.user.clone())
    }

    pub fn store(&self, user: UserModel) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        let expires_at = utc_now() + Duration::seconds(self.ttl_seconds);
        entries.insert(user.id, Entry { user, expires_at });
    }

    pub fn retain_recent(&self) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        let now = utc_now();
        entries.retain(|_, entry| entry.expires_at > now);
    }
    pub fn invalidate(&self, user_id: i64) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.remove(&user_id);
        }
    }
}
