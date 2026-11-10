//! 密码哈希与令牌工具（bcrypt + SHA-256）

/// 哈希密码（bcrypt 默认 cost 12）。
pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
}

/// 校验密码。
pub fn verify_password(plain: &str, hashed: &str) -> bool {
    !hashed.is_empty() && bcrypt::verify(plain, hashed).unwrap_or(false)
}

/// 生成 64 字符高熵刷新令牌（约 384 bit）。
pub fn create_refresh_token() -> String {
    use rand::RngCore;

    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut bytes = [0u8; 64];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| CHARS[*b as usize % CHARS.len()] as char)
        .collect()
}

/// 对刷新令牌做 SHA-256 摘要后入库（明文令牌只出现一次）。
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
