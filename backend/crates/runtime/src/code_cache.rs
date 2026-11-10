//! 邮箱验证码内存缓存（单实例）
//!
//! 验证码不进数据库：重启后已发送的验证码失效，用户重新获取即可。
//! 多实例部署时各实例不共享验证码，需要横向扩展时应换成 Redis 等共享缓存。

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{Duration, NaiveDateTime};

use yukina_shared::serial::utc_now;

struct Entry {
    code_hash: String,
    expires_at: NaiveDateTime,
    sent_at: NaiveDateTime,
}

/// 按「用途 + 邮箱」存放验证码；有效期与冷却由配置决定。
pub struct EmailCodeCache {
    entries: Mutex<HashMap<String, Entry>>,
    ttl_minutes: i64,
    cooldown_seconds: i64,
}

impl EmailCodeCache {
    pub fn new(ttl_minutes: i64, cooldown_seconds: i64) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl_minutes,
            cooldown_seconds,
        }
    }

    /// 验证码有效期（分钟），用于接口响应与邮件正文。
    pub fn ttl_minutes(&self) -> i64 {
        self.ttl_minutes
    }

    fn key(purpose: &str, email: &str) -> String {
        format!("{purpose}:{email}")
    }

    /// 冷却剩余秒数；可立即发送时返回 `None`。
    pub fn cooldown_remaining(&self, purpose: &str, email: &str) -> Option<u64> {
        let entries = self.entries.lock().ok()?;
        let entry = entries.get(&Self::key(purpose, email))?;
        let now = utc_now();
        let ready_at = entry.sent_at + Duration::seconds(self.cooldown_seconds);
        (ready_at > now).then(|| (ready_at - now).num_seconds().max(1) as u64)
    }

    /// 写入验证码（覆盖同邮箱同用途的旧验证码）。
    pub fn store(&self, purpose: &str, email: &str, code_hash: String) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        let now = utc_now();
        entries.insert(
            Self::key(purpose, email),
            Entry {
                code_hash,
                expires_at: now + Duration::minutes(self.ttl_minutes),
                sent_at: now,
            },
        );
    }

    /// 校验并消耗验证码：匹配且未过期时删除并返回 true。
    pub fn consume(&self, purpose: &str, email: &str, code_hash: &str) -> bool {
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        let key = Self::key(purpose, email);
        let valid = match entries.get(&key) {
            Some(entry) => entry.expires_at > utc_now() && entry.code_hash == code_hash,
            None => false,
        };
        if !valid {
            return false;
        }
        entries.remove(&key);
        true
    }

    /// 清理过期条目，避免长期运行后状态膨胀。
    pub fn retain_recent(&self) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        let now = utc_now();
        entries.retain(|_, entry| entry.expires_at > now);
    }
}
