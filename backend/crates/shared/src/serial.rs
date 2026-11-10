//! 时间序列化辅助 — 统一使用 naive UTC（存库格式）

use chrono::{NaiveDateTime, Utc};

/// 当前 UTC 时间（naive，存库格式）。
pub fn utc_now() -> NaiveDateTime {
    Utc::now().naive_utc()
}
