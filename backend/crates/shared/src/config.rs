//! 全局配置 — 优先级：环境变量 > `config.toml` > 内置默认值
//!
//! 敏感信息（`JWT_SECRET`、`SMTP_PASSWORD`、`ADMIN_PASSWORD`）建议只放环境变量或 `.env`；
//! `config.toml` 会被提交到仓库，默认值中不含任何真实凭据。

use std::path::PathBuf;

use serde::Deserialize;

/// 开发环境默认密钥；生产环境必须通过 `JWT_SECRET` 覆盖。
const DEV_SECRET: &str = "dev-only-insecure-secret-change-me";
/// 默认配置文件名（可用 `CONFIG_PATH` 指定其他路径）。
const DEFAULT_CONFIG_FILE: &str = "config.toml";

/// SMTP 发信配置；未配置 `SMTP_HOST` 时整体为 `None`，验证码只写服务端日志。
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    /// 登录用户名，留空表示不需要认证。
    pub username: Option<String>,
    pub password: Option<String>,
    /// 发件地址；未设置时回退到登录用户名。
    pub from: String,
    pub from_name: String,
    /// true 使用 STARTTLS，false 使用明文连接（仅限内网测试）。
    pub tls: bool,
    /// SMTP 超时（秒）。
    pub timeout_seconds: u64,
}

/// 限流额度。
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 通用 API 平均每秒补充的令牌数。
    pub api_per_second: u32,
    /// 通用 API 突发容量。
    pub api_burst: u32,
    /// 敏感接口（登录/注册/验证码/重置）平均补充一次所需秒数。
    pub sensitive_refill_seconds: u64,
    /// 敏感接口突发容量。
    pub sensitive_burst: u32,
}

/// 邮箱验证码参数。
#[derive(Debug, Clone)]
pub struct EmailCodeConfig {
    /// 验证码有效期（分钟）。
    pub ttl_minutes: i64,
    /// 同邮箱同用途的重发冷却（秒）。
    pub cooldown_seconds: i64,
}

/// 进程内缓存参数。
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 用户信息缓存 TTL（秒）。
    pub user_ttl_seconds: i64,
    /// 过期条目清理间隔（秒）。
    pub cleanup_interval_seconds: u64,
}

/// 全局配置结构。
#[derive(Debug, Clone)]
pub struct Settings {
    // ===== 应用 =====
    pub app_name: String,
    pub environment: String,
    pub secret_key: String,
    pub access_token_expire_minutes: i64,
    pub refresh_token_expire_days: i64,
    pub cookie_secure: bool,

    // ===== 数据库 =====
    /// SQLite 文件路径（已从 URL 形式解析）
    pub database_path: String,
    pub data_dir: PathBuf,

    // ===== CORS =====
    pub cors_origins: Vec<String>,

    // ===== 管理员初始账户 =====
    pub admin_username: String,
    pub admin_password: String,
    /// 管理员绑定邮箱（用于找回密码），留空则不绑定。
    pub admin_email: Option<String>,

    // ===== 邮件 =====
    pub smtp: Option<SmtpConfig>,

    // ===== 限流与缓存 =====
    pub rate_limit: RateLimitConfig,
    pub email_code: EmailCodeConfig,
    pub cache: CacheConfig,
    pub search: crate::search_config::SearchConfig,
    pub ai_summary: crate::summary_config::SummaryConfig,

    // ===== 监听 =====
    pub bind_addr: String,
    pub port: u16,
}

// ===== 配置文件结构：全部为 Option，未写或写空的项回退到内置默认值 =====

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    #[serde(default)]
    app: FileApp,
    #[serde(default)]
    server: FileServer,
    #[serde(default)]
    database: FileDatabase,
    #[serde(default)]
    auth: FileAuth,
    #[serde(default)]
    admin: FileAdmin,
    #[serde(default)]
    rate_limit: FileRateLimit,
    #[serde(default)]
    email_code: FileEmailCode,
    #[serde(default)]
    cache: FileCache,
    #[serde(default)]
    smtp: FileSmtp,
    #[serde(default)]
    search: crate::search_config::FileSearchConfig,
    #[serde(default)]
    ai_summary: crate::summary_config::FileSummaryConfig,
}

#[derive(Debug, Default, Deserialize)]
struct FileApp {
    name: Option<String>,
    environment: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FileServer {
    bind_addr: Option<String>,
    port: Option<i64>,
    cors_origins: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct FileDatabase {
    url: Option<String>,
    data_dir: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FileAuth {
    secret: Option<String>,
    access_token_expire_minutes: Option<i64>,
    refresh_token_expire_days: Option<i64>,
    cookie_secure: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct FileAdmin {
    username: Option<String>,
    password: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FileRateLimit {
    api_per_second: Option<i64>,
    api_burst: Option<i64>,
    sensitive_refill_seconds: Option<i64>,
    sensitive_burst: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
struct FileEmailCode {
    ttl_minutes: Option<i64>,
    cooldown_seconds: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
struct FileCache {
    user_ttl_seconds: Option<i64>,
    cleanup_interval_seconds: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
struct FileSmtp {
    host: Option<String>,
    port: Option<i64>,
    tls: Option<bool>,
    username: Option<String>,
    password: Option<String>,
    from: Option<String>,
    from_name: Option<String>,
    timeout_seconds: Option<i64>,
}

/// 读取配置文件；不存在时使用全默认值，格式错误直接终止启动。
fn load_config_file() -> FileConfig {
    let path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_FILE.to_string());
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str(&text) {
            Ok(cfg) => cfg,
            Err(err) => panic!("配置文件 {path} 解析失败: {err}"),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => FileConfig::default(),
        Err(err) => panic!("读取配置文件 {path} 失败: {err}"),
    }
}

/// 读取环境变量：未设置或空白时返回 `None`。
fn env_opt(key: &str) -> Option<String> {
    let value = std::env::var(key).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn env_i64(key: &str) -> Option<i64> {
    std::env::var(key).ok()?.trim().parse().ok()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// 普通字符串：环境变量 > 配置文件（非空）> 默认值。
fn pick_str(key: &str, file: Option<String>, default: &str) -> String {
    env_opt(key)
        .or_else(|| non_empty(file))
        .unwrap_or_else(|| default.to_string())
}

/// 可选字符串：未配置时返回 `None`。
fn pick_opt_str(key: &str, file: Option<String>) -> Option<String> {
    env_opt(key).or_else(|| non_empty(file))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// 整数：环境变量可解析时优先，否则用配置文件，最后默认值。
fn pick_i64(key: &str, file: Option<i64>, default: i64) -> i64 {
    env_i64(key).or(file).unwrap_or(default)
}

/// 布尔值：接受 true/false/1/0/yes/on 等写法。
fn pick_bool(key: &str, file: Option<bool>, default: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => parse_bool(&value).unwrap_or(default),
        Err(_) => file.unwrap_or(default),
    }
}

/// 列表：环境变量支持 JSON 数组或逗号分隔。
fn pick_list(key: &str, file: Option<Vec<String>>, default: &[&str]) -> Vec<String> {
    if let Ok(value) = std::env::var(key) {
        let value = value.trim();
        if !value.is_empty() {
            if value.starts_with('[') {
                if let Ok(list) = serde_json::from_str::<Vec<String>>(value) {
                    return list
                        .into_iter()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
            return value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        return default.iter().map(|s| s.to_string()).collect();
    }
    file.filter(|list| !list.is_empty())
        .unwrap_or_else(|| default.iter().map(|s| s.to_string()).collect())
}

/// 把 SQLite URL 解析为文件路径。
/// 支持 `sqlite:///./blog.db` 与 `sqlite://blog.db` 形式，也接受裸路径。
fn normalize_database_url(url: &str) -> String {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("sqlite://") {
        return rest.trim_start_matches('/').to_string();
    }
    url.to_string()
}

impl Settings {
    /// 合并 `config.toml` 与环境变量（`.env` 由 main.rs 在更早阶段加载）。
    pub fn load() -> Settings {
        let FileConfig {
            app,
            server,
            database,
            auth,
            admin,
            rate_limit,
            email_code,
            cache,
            smtp,
            search,
            ai_summary,
        } = load_config_file();

        let app_name = pick_str("APP_NAME", app.name, "Yukina Blog API");
        let database_url = pick_str("DATABASE_URL", database.url, "sqlite://data/yukina.db");

        Settings {
            app_name: app_name.clone(),
            environment: pick_str("ENVIRONMENT", app.environment, "development"),
            secret_key: pick_str("JWT_SECRET", auth.secret, DEV_SECRET),
            access_token_expire_minutes: pick_i64(
                "ACCESS_TOKEN_EXPIRE_MINUTES",
                auth.access_token_expire_minutes,
                30,
            ),
            refresh_token_expire_days: pick_i64(
                "REFRESH_TOKEN_EXPIRE_DAYS",
                auth.refresh_token_expire_days,
                7,
            ),
            cookie_secure: pick_bool("COOKIE_SECURE", auth.cookie_secure, false),
            database_path: compatible_database_path(normalize_database_url(&database_url)),
            data_dir: PathBuf::from(pick_str("DATA_DIR", database.data_dir, "data")),
            cors_origins: pick_list(
                "CORS_ORIGINS",
                server.cors_origins,
                &["http://localhost:4321", "http://127.0.0.1:4321"],
            ),
            admin_username: pick_str("ADMIN_USERNAME", admin.username, "admin"),
            admin_password: pick_str("ADMIN_PASSWORD", admin.password, "admin123"),
            admin_email: pick_opt_str("ADMIN_EMAIL", admin.email),
            smtp: load_smtp(smtp, &app_name),
            search: crate::search_config::SearchConfig::load(search),
            ai_summary: crate::summary_config::SummaryConfig::load(ai_summary),
            rate_limit: RateLimitConfig {
                api_per_second: pick_i64("RATE_API_PER_SECOND", rate_limit.api_per_second, 1).max(1)
                    as u32,
                api_burst: pick_i64("RATE_API_BURST", rate_limit.api_burst, 20).max(1) as u32,
                sensitive_refill_seconds: pick_i64(
                    "RATE_SENSITIVE_REFILL_SECONDS",
                    rate_limit.sensitive_refill_seconds,
                    90,
                )
                .max(1) as u64,
                sensitive_burst: pick_i64("RATE_SENSITIVE_BURST", rate_limit.sensitive_burst, 10)
                    .max(1) as u32,
            },
            email_code: EmailCodeConfig {
                ttl_minutes: pick_i64("EMAIL_CODE_TTL_MINUTES", email_code.ttl_minutes, 10).max(1),
                cooldown_seconds: pick_i64(
                    "EMAIL_CODE_COOLDOWN_SECONDS",
                    email_code.cooldown_seconds,
                    60,
                )
                .max(0),
            },
            cache: CacheConfig {
                user_ttl_seconds: pick_i64("USER_CACHE_TTL_SECONDS", cache.user_ttl_seconds, 30)
                    .max(0),
                cleanup_interval_seconds: pick_i64(
                    "CACHE_CLEANUP_INTERVAL_SECONDS",
                    cache.cleanup_interval_seconds,
                    300,
                )
                .max(1) as u64,
            },
            bind_addr: pick_str("BIND_ADDR", server.bind_addr, "127.0.0.1"),
            port: pick_i64("PORT", server.port, 8787).max(1) as u16,
        }
    }
}

/// 加载 SMTP 配置：`SMTP_HOST` 为空即视为未配置（验证码走日志模式）。
fn load_smtp(file: FileSmtp, app_name: &str) -> Option<SmtpConfig> {
    let host = pick_opt_str("SMTP_HOST", file.host)?;
    let username = pick_opt_str("SMTP_USERNAME", file.username);
    let password = pick_opt_str("SMTP_PASSWORD", file.password);
    let from = pick_opt_str("SMTP_FROM", file.from)
        .or_else(|| username.clone())
        .unwrap_or_default();
    if from.is_empty() {
        tracing::warn!("SMTP_HOST 已配置但缺少 SMTP_FROM / SMTP_USERNAME，邮箱验证码将只写入日志");
        return None;
    }
    let tls = pick_bool("SMTP_TLS", file.tls, true);
    Some(SmtpConfig {
        host,
        port: pick_i64("SMTP_PORT", file.port, if tls { 587 } else { 25 }).max(1) as u16,
        username,
        password,
        from,
        from_name: pick_str("SMTP_FROM_NAME", file.from_name, app_name),
        tls,
        timeout_seconds: pick_i64("SMTP_TIMEOUT_SECONDS", file.timeout_seconds, 20).max(1) as u64,
    })
}

/// 生产环境配置保护：密钥和管理员密码不允许沿用默认值。
pub fn validate_production_settings(settings: &Settings) -> Result<(), String> {
    if settings.environment != "production" {
        return Ok(());
    }
    if settings.secret_key == DEV_SECRET {
        return Err("生产环境必须设置 JWT_SECRET（或 config.toml 的 [auth].secret）".to_string());
    }
    if settings.admin_password == "admin123" {
        return Err(
            "生产环境必须修改 ADMIN_PASSWORD（或 config.toml 的 [admin].password）".to_string(),
        );
    }
    Ok(())
}

/// Continue using the existing database instead of silently creating an empty one.
fn compatible_database_path(path: String) -> String {
    let selected = std::path::Path::new(&path);
    if selected.file_name().and_then(|name| name.to_str()) == Some("yukina.db")
        && !selected.exists()
    {
        let previous = selected.with_file_name("aemeath.db");
        if previous.is_file() {
            return previous.to_string_lossy().into_owned();
        }
    }
    path
}

#[cfg(test)]
mod brand_tests {
    use super::compatible_database_path;
    #[test]
    fn rebrand_keeps_existing_database_and_respects_explicit_paths() {
        let dir = std::env::temp_dir().join(format!(
            "yukina-rebrand-{}",
            crate::media_names::random_file_name()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let new = dir.join("yukina.db");
        let old = dir.join("aemeath.db");
        std::fs::write(&old, b"existing-data").unwrap();
        assert_eq!(
            compatible_database_path(new.to_string_lossy().into_owned()),
            old.to_string_lossy()
        );
        std::fs::write(&new, b"new-data").unwrap();
        assert_eq!(
            compatible_database_path(new.to_string_lossy().into_owned()),
            new.to_string_lossy()
        );
        let explicit = dir.join("custom.db").to_string_lossy().into_owned();
        assert_eq!(compatible_database_path(explicit.clone()), explicit);
        std::fs::remove_file(old).unwrap();
        std::fs::remove_file(new).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
