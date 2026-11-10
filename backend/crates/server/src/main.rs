//! Yukina Blog API — Rust (axum) 版入口
//!
//! 加载配置 → 初始化共享资源 → 组装路由 → 启动 HTTP 服务。

use std::net::SocketAddr;
use std::sync::Arc;

use tracing::info;

use yukina_server::{bootstrap, media_migration, router, routes};
use yukina_shared::config;

use config::{validate_production_settings, Settings};

#[tokio::main]
async fn main() {
    // 加载 .env（不存在时忽略，全部走环境变量）
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "yukina=info,tower_http=info,sqlx=warn".into()),
        )
        .init();

    let settings = Arc::new(Settings::load());
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next();
    if command.as_deref() == Some("import-site") {
        let path = arguments.next().expect("缺少站点配置文件路径");
        if let Err(error) =
            routes::site::import_defaults(&settings, std::path::Path::new(&path)).await
        {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if command.as_deref() == Some("migrate-article-images") {
        let Some(plan) = arguments.next() else {
            eprintln!("缺少图片迁移计划路径");
            std::process::exit(1);
        };
        if let Err(error) = media_migration::run(&settings, std::path::Path::new(&plan)).await {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if let Err(msg) = validate_production_settings(&settings) {
        panic!("{msg}");
    }

    let state = bootstrap::initialize(settings.clone()).await;
    let app = router(state);

    let addr: SocketAddr = format!("{}:{}", settings.bind_addr, settings.port)
        .parse()
        .expect("监听地址无效");
    info!("{} 启动于 http://{addr}", settings.app_name);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("端口监听失败");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("服务运行失败");
}
