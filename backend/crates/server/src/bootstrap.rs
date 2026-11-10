//! 启动阶段的资源初始化；请求处理代码不应在这里添加业务逻辑。

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use std::{sync::Arc, time::Duration};
use tracing::info;
use yukina_runtime::{auth, code_cache, middleware, session_cache, state::AppState, user_cache};
use yukina_shared::{config::Settings, serial};
use yukina_store::{db, entities, migrations};

pub async fn initialize(settings: Arc<Settings>) -> AppState {
    // 确保运行目录存在
    std::fs::create_dir_all(&settings.data_dir).expect("创建数据目录失败");
    std::fs::create_dir_all(settings.data_dir.join("uploads")).expect("创建上传目录失败");

    // IP 归属地数据库：data/ip2region.xdb（缺失时评论来源回退为脱敏 IP）
    let ip_region = {
        let path = settings.data_dir.join("ip2region.xdb");
        match ip2region::Searcher::new(&path) {
            Ok(searcher) => {
                tracing::info!("已加载 IP 归属地数据库: {}", path.display());
                Some(Arc::new(searcher))
            }
            Err(err) => {
                tracing::warn!("未加载 IP 归属地数据库（{}），评论来源将显示脱敏 IP", err);
                None
            }
        }
    };

    // 数据库连接 + 建表
    let db = db::create_connection(&settings)
        .await
        .expect("数据库初始化失败");
    migrations::run(&db).await.expect("建表失败");

    ensure_admin(&db, &settings).await;

    let api_limiter = Arc::new(middleware::rate_limit::api_limiter(&settings.rate_limit));
    let login_limiter = Arc::new(middleware::rate_limit::login_limiter(&settings.rate_limit));
    let email_codes = Arc::new(code_cache::EmailCodeCache::new(
        settings.email_code.ttl_minutes,
        settings.email_code.cooldown_seconds,
    ));
    let sessions = Arc::new(session_cache::SessionCache::new());
    let users = Arc::new(user_cache::UserCache::new(settings.cache.user_ttl_seconds));
    let cleanup_interval = Duration::from_secs(settings.cache.cleanup_interval_seconds);
    let embeddings =
        Arc::new(yukina_embedding::Embedder::new(settings.search.clone()).expect("搜索配置无效"));
    let state = AppState {
        db,
        embeddings,
        cfg: settings.clone(),
        api_limiter: api_limiter.clone(),
        login_limiter: login_limiter.clone(),
        email_codes: email_codes.clone(),
        sessions: sessions.clone(),
        users: users.clone(),
        ip_region,
    };

    // 定期回收限流器中已恢复满额的空闲 IP、过期验证码、过期会话与过期用户缓存
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(cleanup_interval);
        loop {
            ticker.tick().await;
            api_limiter.retain_recent();
            login_limiter.retain_recent();
            email_codes.retain_recent();
            sessions.retain_recent();
            users.retain_recent();
        }
    });

    crate::documents::indexing::start(state.clone());
    state
}

async fn ensure_admin(db: &DatabaseConnection, settings: &Settings) {
    // 初始化管理员账户（如果不存在）
    let mut admin = entities::user::Entity::find()
        .filter(entities::user::Column::Username.eq(&settings.admin_username))
        .one(db)
        .await
        .expect("查询管理员失败");
    if admin.is_none() {
        let hash =
            auth::password::hash_password(&settings.admin_password).expect("管理员密码哈希失败");
        admin = Some(
            entities::user::ActiveModel {
                username: Set(settings.admin_username.clone()),
                password_hash: Set(hash),
                is_admin: Set(true),
                is_active: Set(true),
                created_at: Set(serial::utc_now()),
                updated_at: Set(serial::utc_now()),
                ..Default::default()
            }
            .insert(db)
            .await
            .expect("创建管理员失败"),
        );
        info!("已创建初始管理员账户: {}", settings.admin_username);
    }

    // 按 ADMIN_EMAIL 幂等绑定管理员邮箱（用于找回密码）
    if let (Some(admin), Some(email)) = (&admin, &settings.admin_email) {
        let email = email.trim().to_lowercase();
        if admin.email.is_none() {
            let result = entities::user::ActiveModel {
                id: ActiveValue::Set(admin.id),
                email: Set(Some(email.clone())),
                updated_at: Set(serial::utc_now()),
                ..Default::default()
            }
            .update(db)
            .await;
            match result {
                Ok(_) => info!("管理员已绑定邮箱: {email}"),
                Err(err) => {
                    tracing::warn!(%err, "绑定管理员邮箱失败（该邮箱可能已被其他账号占用）")
                }
            }
        }
    }
}
