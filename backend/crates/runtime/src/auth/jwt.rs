//! JWT 签发 / 验证（HS256）

use std::collections::HashMap;

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use yukina_shared::config::Settings;

/// 签名算法。
pub const ALGORITHM: Algorithm = Algorithm::HS256;

/// JWT 载荷。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// 主体：用户 id
    #[serde(default)]
    pub sub: Option<String>,
    /// 令牌用途：access
    #[serde(default, rename = "type")]
    pub token_type: Option<String>,
    /// 过期时间（Unix 秒），jsonwebtoken 自动校验
    #[serde(default)]
    pub exp: Option<i64>,
    /// 其余自定义字段
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 签发访问令牌。`data` 中的字段合并进 payload。
pub fn create_access_token(
    settings: &Settings,
    mut data: serde_json::Value,
    expires_delta: Option<Duration>,
) -> Result<String, jsonwebtoken::errors::Error> {
    let expire = Utc::now()
        + expires_delta.unwrap_or_else(|| Duration::minutes(settings.access_token_expire_minutes));
    if let Some(obj) = data.as_object_mut() {
        obj.insert("type".into(), serde_json::Value::String("access".into()));
        obj.insert(
            "exp".into(),
            serde_json::Value::Number(expire.timestamp().into()),
        );
    }
    encode(
        &Header::new(ALGORITHM),
        &data,
        &EncodingKey::from_secret(settings.secret_key.as_bytes()),
    )
}

/// 解码并校验 JWT；任何错误返回 None。
pub fn decode_token(settings: &Settings, token: &str) -> Option<Claims> {
    let mut validation = Validation::new(ALGORITHM);
    validation.validate_exp = true;
    validation.required_spec_claims.clear();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(settings.secret_key.as_bytes()),
        &validation,
    )
    .ok()
    .map(|data| data.claims)
}
