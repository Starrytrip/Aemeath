use super::*;
use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use std::sync::Arc;
use yukina_embedding::{valid_vector, Embedder};
use yukina_shared::search_config::SearchConfig;
use yukina_store::{
    db::register_vector_extension,
    documents::{
        schema,
        store::{self, DocumentInput},
    },
};

async fn database() -> DatabaseConnection {
    register_vector_extension();
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    schema::migrate(&db).await.unwrap();
    schema::migrate(&db).await.unwrap();
    db
}
fn input(key: &str, text: &str, visibility: &str) -> DocumentInput {
    DocumentInput {
        source_key: key.into(),
        title: key.into(),
        url: format!("/posts/{key}/"),
        markdown: text.into(),
        category: "技术".into(),
        tags: vec!["Rust".into()],
        visibility: visibility.into(),
        ai_summary: None,
        date: None,
        cover_url: None,
    }
}
fn disabled() -> Embedder {
    Embedder::new(SearchConfig {
        endpoint: None,
        model: String::new(),
        dimensions: 0,
        api_key: None,
    })
    .unwrap()
}
fn query(q: &str) -> SearchQuery {
    SearchQuery {
        q: q.into(),
        mode: Mode::Hybrid,
        limit: Some(10),
    }
}

#[tokio::test]
async fn search_round_trip_handles_chinese_updates_privacy_and_delete() {
    let db = database().await;
    let saved = store::save(
        &db,
        input(
            "public",
            "# 部署\n\nSQLite 全文检索支持中文关键词。",
            "public",
        ),
    )
    .await
    .unwrap();
    store::save(
        &db,
        input("private", "SQLite 全文检索 私密资料。", "private"),
    )
    .await
    .unwrap();
    for term in ["中文", "全文检索", "SQLite", "Rust", "技术"] {
        let response = run(&db, &disabled(), query(term)).await.unwrap();
        assert_eq!(response.mode, "keyword");
        assert_eq!(response.results.len(), 1, "{term}");
        assert_eq!(response.results[0].document_id, saved.id);
    }
    assert!(run(&db, &disabled(), query("私密资料"))
        .await
        .unwrap()
        .results
        .is_empty());
    assert!(run(&db, &disabled(), query("\" OR *")).await.is_ok());
    assert!(run(&db, &disabled(), query("%_"))
        .await
        .unwrap()
        .results
        .is_empty());
    let same = store::save(
        &db,
        input(
            "public",
            "# 部署\n\nSQLite 全文检索支持中文关键词。",
            "public",
        ),
    )
    .await
    .unwrap();
    assert_eq!(same.revision, saved.revision);
    assert_eq!(same.id, saved.id);
    let changed = store::save(&db, input("public", "全新的索引内容", "public"))
        .await
        .unwrap();
    assert_eq!(changed.revision, saved.revision + 1);
    assert_eq!(changed.id, saved.id);
    assert!(run(&db, &disabled(), query("SQLite"))
        .await
        .unwrap()
        .results
        .is_empty());
    assert_eq!(
        run(&db, &disabled(), query("全新的"))
            .await
            .unwrap()
            .results
            .len(),
        1
    );
    store::save(&db, input("public", "全新的索引内容", "private"))
        .await
        .unwrap();
    assert!(run(&db, &disabled(), query("全新的"))
        .await
        .unwrap()
        .results
        .is_empty());
    store::remove(&db, saved.id).await.unwrap();
    assert!(store::detail(&db, saved.id).await.is_err());
}

#[tokio::test]
async fn document_save_rolls_back_metadata_chunks_and_fts_on_failure() {
    let db = database().await;
    let saved = store::save(&db, input("atomic", "original searchable text", "public"))
        .await
        .unwrap();
    // 模拟切片写入失败，验证 upsert、旧切片删除和 FTS 触发器处于同一事务。
    db.execute_unprepared("CREATE TRIGGER reject_test_chunk BEFORE INSERT ON document_chunks BEGIN SELECT RAISE(ABORT, 'test failure'); END").await.unwrap();
    assert!(
        store::save(&db, input("atomic", "replacement content", "private"))
            .await
            .is_err()
    );
    let detail = store::detail(&db, saved.id).await.unwrap();
    assert_eq!(detail.document.revision, saved.revision);
    assert_eq!(detail.document.visibility, "public");
    assert_eq!(detail.markdown, "original searchable text");
    assert_eq!(
        run(&db, &disabled(), query("original"))
            .await
            .unwrap()
            .results
            .len(),
        1
    );
    assert!(run(&db, &disabled(), query("replacement"))
        .await
        .unwrap()
        .results
        .is_empty());
}

#[tokio::test]
async fn short_queries_and_long_documents_are_deduplicated() {
    let db = database().await;
    store::save(&db, input("one", &"内存优化 ".repeat(600), "public"))
        .await
        .unwrap();
    assert_eq!(
        run(&db, &disabled(), query("内存"))
            .await
            .unwrap()
            .results
            .len(),
        1
    );
    let row = db
        .query_one(sql("SELECT count(*) AS n FROM document_chunks", vec![]))
        .await
        .unwrap()
        .unwrap();
    assert!(row.try_get::<i64>("", "n").unwrap() > 1);
    let mut unsafe_input = input("bad", "text", "public");
    unsafe_input.url = "//untrusted.example".into();
    assert!(store::save(&db, unsafe_input).await.is_err());
}

async fn mock_embedder() -> (Embedder, tokio::task::JoinHandle<()>) {
    let app = Router::new().route("/embeddings",axum::routing::post(|Json(body):Json<serde_json::Value>| async move {
        let values = body["input"].as_array().unwrap();
        Json(serde_json::json!({"data":values.iter().enumerate().map(|(index,text)| {
            let text = text.as_str().unwrap();
            let embedding = if text.contains("内存") || text.contains("RAM") {vec![1.0,0.0,0.0]} else {vec![0.0,1.0,0.0]};
            serde_json::json!({"index":index,"embedding":embedding})
        }).collect::<Vec<_>>()}))
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let embedder = Embedder::new(SearchConfig {
        endpoint: Some(format!("http://{addr}/embeddings")),
        model: "test-v1".into(),
        dimensions: 3,
        api_key: None,
    })
    .unwrap();
    (embedder, task)
}

fn state(db: DatabaseConnection, embeddings: Embedder) -> AppState {
    use yukina_runtime::{
        code_cache::EmailCodeCache, middleware::rate_limit, session_cache::SessionCache,
        user_cache::UserCache,
    };
    let cfg = Arc::new(yukina_shared::config::Settings::load());
    AppState {
        db,
        embeddings: Arc::new(embeddings),
        api_limiter: Arc::new(rate_limit::api_limiter(&cfg.rate_limit)),
        login_limiter: Arc::new(rate_limit::login_limiter(&cfg.rate_limit)),
        cfg,
        email_codes: Arc::new(EmailCodeCache::new(10, 60)),
        sessions: Arc::new(SessionCache::new()),
        users: Arc::new(UserCache::new(30)),
        ip_region: None,
    }
}

#[tokio::test]
async fn indexing_and_semantic_search_work_with_sqlx_and_persisted_vectors() {
    let db = database().await;
    let memory = store::save(
        &db,
        input(
            "memory",
            "# 性能\n\n避免复制数据库可降低内存使用。",
            "public",
        ),
    )
    .await
    .unwrap();
    store::save(&db, input("flowers", "花园里的玫瑰开花了。", "public"))
        .await
        .unwrap();
    let (embedder, task) = mock_embedder().await;
    let state = state(db, embedder);
    crate::documents::indexing::tick(&state).await.unwrap();
    crate::documents::indexing::tick(&state).await.unwrap();
    assert_eq!(
        store::get(&state.db, memory.id).await.unwrap().index_status,
        "ready"
    );
    let response = run(&state.db, &state.embeddings, query("RAM 占用"))
        .await
        .unwrap();
    assert_eq!(response.mode, "hybrid");
    assert_eq!(response.results[0].document_id, memory.id);
    // 同维度的其他模型不能误用旧向量。
    let (mut changed, changed_task) = mock_embedder().await;
    changed.config.model = "test-v2".into();
    assert_eq!(
        run(&state.db, &changed, query("RAM 占用"))
            .await
            .unwrap()
            .mode,
        "keyword"
    );
    // 更新后旧向量被清除，必须重新建立索引。
    store::save(&state.db, input("memory", "新的内存优化方法。", "public"))
        .await
        .unwrap();
    assert_eq!(
        store::get(&state.db, memory.id).await.unwrap().index_status,
        "queued"
    );
    task.abort();
    changed_task.abort();
}

#[test]
fn bad_vectors_are_rejected_before_sqlite() {
    assert!(!valid_vector(&[0.0, 0.0], 2));
    assert!(!valid_vector(&[f32::NAN, 1.0], 2));
    assert!(!valid_vector(&[f32::INFINITY], 1));
    assert!(!valid_vector(&[1.0], 2));
    assert!(valid_vector(&[1.0, 0.0], 2));
}

#[tokio::test]
async fn editing_a_document_while_embedding_does_not_publish_stale_vectors() {
    let db = database().await;
    let doc = store::save(&db, input("race", "旧的公开内容", "public"))
        .await
        .unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let handler_entered = entered.clone();
    let handler_release = release.clone();
    let app = Router::new().route(
        "/embeddings",
        axum::routing::post(move || {
            let entered = handler_entered.clone();
            let release = handler_release.clone();
            async move {
                entered.notify_one();
                release.notified().await;
                Json(serde_json::json!({"data":[{"index":0,"embedding":[1.0,0.0,0.0]}]}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let embedder = Embedder::new(SearchConfig {
        endpoint: Some(format!("http://{addr}/embeddings")),
        model: "test".into(),
        dimensions: 3,
        api_key: None,
    })
    .unwrap();
    let state = state(db, embedder);
    let worker_state = state.clone();
    let worker = tokio::spawn(async move {
        crate::documents::indexing::tick(&worker_state)
            .await
            .unwrap();
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    let edited = store::save(&state.db, input("race", "新的私密内容", "private"))
        .await
        .unwrap();
    release.notify_one();
    worker.await.unwrap();
    assert_eq!(
        store::get(&state.db, doc.id).await.unwrap().revision,
        edited.revision
    );
    assert_eq!(
        store::get(&state.db, doc.id).await.unwrap().index_status,
        "private"
    );
    let row = state
        .db
        .query_one(sql(
            "SELECT count(*) AS n FROM document_chunks WHERE embedding IS NOT NULL",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 0);
    server.abort();
}

async fn api_state() -> (AppState, String, String) {
    use sea_orm::{ActiveModelTrait, Set};
    let db = database().await;
    yukina_store::migrations::run(&db).await.unwrap();
    let state = state(db, disabled());
    let mut tokens = Vec::new();
    for admin in [true, false] {
        let now = yukina_shared::serial::utc_now();
        let user = yukina_store::entities::user::ActiveModel {
            username: Set(if admin { "test-admin" } else { "test-user" }.into()),
            password_hash: Set("not-used-by-token-tests".into()),
            is_admin: Set(admin),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&state.db)
        .await
        .unwrap();
        tokens.push(
            yukina_runtime::auth::jwt::create_access_token(
                &state.cfg,
                serde_json::json!({"sub":user.id.to_string()}),
                None,
            )
            .unwrap(),
        );
    }
    (state, tokens[0].clone(), tokens[1].clone())
}

#[tokio::test]
async fn analytics_deduplicates_counts_and_protects_visit_details() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (state, admin, member) = api_state().await;
    let app = crate::router(state.clone()).layer(axum::Extension(axum::extract::ConnectInfo(
        "192.0.2.9:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let send = |method: &str, path: &str, token: Option<&str>, body: serde_json::Value| {
        let app = app.clone();
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("user-agent", "Test browser");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        async move {
            app.oneshot(request.body(Body::from(body.to_string())).unwrap())
                .await
                .unwrap()
        }
    };
    let event = serde_json::json!({"event_id":"event-123","visitor_id":"visitor-123","path":"/posts/test/?secret=hidden","title":"Test article","referrer":"https://example.com/source?token=secret#private"});
    for _ in 0..2 {
        assert_eq!(
            send("POST", "/api/v1/analytics/events", None, event.clone())
                .await
                .status(),
            StatusCode::OK
        );
    }
    let mut second = event.clone();
    second["event_id"] = "event-456".into();
    assert_eq!(
        send("POST", "/api/v1/analytics/events", None, second)
            .await
            .status(),
        StatusCode::OK
    );
    let mut third = event;
    third["event_id"] = "event-789".into();
    third["visitor_id"] = "visitor-456".into();
    assert_eq!(
        send("POST", "/api/v1/analytics/events", None, third)
            .await
            .status(),
        StatusCode::OK
    );
    for path in ["/api/v1/admin/visits", "/api/v1/admin/dashboard"] {
        assert_eq!(
            send("GET", path, None, serde_json::Value::Null)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            send("GET", path, Some(&member), serde_json::Value::Null)
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    use sea_orm::{ActiveModelTrait, Set};
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    let yesterday_end = today.and_hms_opt(0, 0, 0).unwrap()
        - chrono::Duration::hours(8)
        - chrono::Duration::seconds(1);
    yukina_store::entities::visit_event::ActiveModel {
        event_key: Set("boundary-event".into()),
        visitor_key: Set("another-visitor".into()),
        path: Set("/yesterday/".into()),
        title: Set("Yesterday".into()),
        referrer: Set(String::new()),
        user_agent: Set(String::new()),
        ip_address: Set("192.0.2.10".into()),
        occurred_at: Set(yesterday_end),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();
    let response = send(
        "GET",
        "/api/v1/admin/dashboard?days=7",
        Some(&admin),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_eq!(value["total"]["pv"], 4);
    assert_eq!(value["today"]["pv"], 3);
    assert_eq!(value["yesterday"]["pv"], 1);
    assert_eq!(value["recent"][0]["path"], "/posts/test/");
    assert_eq!(value["total"]["uv"], 3);
    assert_eq!(value["trend"].as_array().unwrap().len(), 7);
    assert_eq!(value["top_pages"][0]["pv"], 3);
    let response = send(
        "GET",
        "/api/v1/admin/visits?ip=192.0.2.9&page_size=2",
        Some(&admin),
        serde_json::Value::Null,
    )
    .await;
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_eq!(value["total"], 3);
    assert_eq!(value["items"].as_array().unwrap().len(), 2);
    assert_eq!(value["items"][0]["path"], "/posts/test/");
    assert_eq!(value["items"][0]["referrer"], "https://example.com/source");
    assert!(value["items"][0].get("visitor_key").is_none());
    assert!(value["items"][0].get("event_key").is_none());
    assert_eq!(
        send(
            "GET",
            "/api/v1/admin/visits?start=invalid",
            Some(&admin),
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn user_management_revokes_old_tokens_and_preserves_admin_access() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (state, admin, member) = api_state().await;
    let app = crate::router(state.clone()).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let send = |method: &str, path: &str, token: &str, body: serde_json::Value| {
        let app = app.clone();
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"));
        async move {
            app.oneshot(request.body(Body::from(body.to_string())).unwrap())
                .await
                .unwrap()
        }
    };
    assert_eq!(
        send(
            "GET",
            "/api/v1/admin/users",
            &member,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let response = send(
        "GET",
        "/api/v1/admin/users",
        &admin,
        serde_json::Value::Null,
    )
    .await;
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_eq!(value["total"], 2);
    assert!(value["items"][0].get("password_hash").is_none());
    assert!(value["items"][0].get("auth_version").is_none());
    assert_eq!(
        send("GET", "/api/v1/auth/me", &member, serde_json::Value::Null)
            .await
            .status(),
        StatusCode::OK
    );
    state.sessions.store(
        "test-session".into(),
        2,
        0,
        yukina_shared::serial::utc_now() + chrono::Duration::days(1),
    );
    assert_eq!(
        send(
            "POST",
            "/api/v1/admin/users/2/revoke-sessions",
            &admin,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert!(state
        .sessions
        .consume("test-session", yukina_shared::serial::utc_now())
        .is_none());
    assert_eq!(
        send("GET", "/api/v1/auth/me", &member, serde_json::Value::Null)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let input = serde_json::json!({"nickname":"Member","email":"member@example.com","is_admin":false,"is_active":true});
    assert_eq!(
        send("PATCH", "/api/v1/admin/users/1", &admin, input.clone())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let mut changed = input.clone();
    changed["is_active"] = false.into();
    assert_eq!(
        send("PATCH", "/api/v1/admin/users/2", &admin, changed)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        send("GET", "/api/v1/auth/me", &member, serde_json::Value::Null)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send("PATCH", "/api/v1/admin/users/2", &admin, input)
            .await
            .status(),
        StatusCode::OK
    );
    let row = yukina_store::entities::user::Entity::find_by_id(2)
        .one(&state.db)
        .await
        .unwrap()
        .unwrap();
    let fresh = yukina_runtime::auth::jwt::create_access_token(
        &state.cfg,
        serde_json::json!({"sub":"2","v":row.auth_version}),
        None,
    )
    .unwrap();
    assert_eq!(
        send(
            "POST",
            "/api/v1/admin/users/2/reset-password",
            &admin,
            serde_json::json!({"password":"a-new-test-password"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send("GET", "/api/v1/auth/me", &fresh, serde_json::Value::Null)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            "DELETE",
            "/api/v1/admin/users/1",
            &admin,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    // 重置后重新登录与刷新会话仍可正常签发当前版本的令牌。
    let login = send(
        "POST",
        "/api/v1/auth/login",
        &admin,
        serde_json::json!({"account":"test-user","password":"a-new-test-password"}),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    let cookie = login
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let refresh = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refresh.status(), StatusCode::OK);
    let input = serde_json::json!({"username":"new-user","password":"a-test-password","nickname":"New","email":"","is_admin":false,"is_active":true});
    let response = send("POST", "/api/v1/admin/users", &admin, input).await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    use sea_orm::{ActiveModelTrait, Set};
    let comment = yukina_store::entities::comment::ActiveModel {
        page: Set("/guestbook/".into()),
        nick: Set("Old author".into()),
        content: Set("Keep this comment".into()),
        user_id: Set(value["id"].as_i64()),
        created_at: Set(yukina_shared::serial::utc_now()),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();
    assert_eq!(
        send(
            "DELETE",
            &format!("/api/v1/admin/users/{}", value["id"]),
            &admin,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    let retained = yukina_store::entities::comment::Entity::find_by_id(comment.id)
        .one(&state.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.content, "Keep this comment");
    assert!(retained.user_id.is_none());
}

#[tokio::test]
async fn backgrounds_edit_and_filtered_reorder_preserve_hidden_positions() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (state, admin, member) = api_state().await;
    let app = crate::router(state.clone()).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let send = |method: &str, path: &str, token: Option<&str>, body: serde_json::Value| {
        let app = app.clone();
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = token {
            req = req.header("authorization", format!("Bearer {token}"));
        }
        async move {
            app.oneshot(req.body(Body::from(body.to_string())).unwrap())
                .await
                .unwrap()
        }
    };
    let mut ids = Vec::new();
    for (index, theme) in ["all", "dark", "all"].iter().enumerate() {
        let response=send("POST","/api/v1/backgrounds",Some(&admin),serde_json::json!({"url":format!("/image-{index}.jpg"),"poster_url":"","media_type":"image","theme":theme,"device":"desktop"})).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
        ids.push(body["id"].as_i64().unwrap());
    }
    let edit = serde_json::json!({"url":"/video.mp4","poster_url":"/poster.jpg","media_type":"video","theme":"all","device":"mobile"});
    let path = format!("/api/v1/backgrounds/{}", ids[0]);
    assert_eq!(
        send("PATCH", &path, None, edit.clone()).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send("PATCH", &path, Some(&member), edit.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            "PATCH",
            "/api/v1/backgrounds/99999",
            Some(&admin),
            edit.clone()
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let before = yukina_store::entities::background::Entity::find_by_id(ids[0])
        .one(&state.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        send("PATCH", &path, Some(&admin), edit.clone())
            .await
            .status(),
        StatusCode::OK
    );
    let after = yukina_store::entities::background::Entity::find_by_id(ids[0])
        .one(&state.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.sort_order, before.sort_order);
    assert_eq!(after.theme, "all");
    assert_eq!(after.device, "mobile");
    assert_eq!(after.poster_url, "/poster.jpg");
    let mut invalid = edit;
    invalid["theme"] = "invalid".into();
    assert_eq!(
        send("PATCH", &path, Some(&admin), invalid).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(
            "POST",
            "/api/v1/backgrounds/reorder",
            Some(&admin),
            serde_json::json!({"ids":[ids[2],ids[0]]})
        )
        .await
        .status(),
        StatusCode::OK
    );
    async fn order(db: &DatabaseConnection) -> Vec<i64> {
        use sea_orm::{EntityTrait, QueryOrder};
        yukina_store::entities::background::Entity::find()
            .order_by_asc(yukina_store::entities::background::Column::SortOrder)
            .order_by_asc(yukina_store::entities::background::Column::Id)
            .all(db)
            .await
            .unwrap()
            .into_iter()
            .map(|b| b.id)
            .collect()
    }
    assert_eq!(order(&state.db).await, vec![ids[2], ids[1], ids[0]]);
    assert_eq!(
        send(
            "POST",
            "/api/v1/backgrounds/reorder",
            Some(&admin),
            serde_json::json!({"ids":[ids[0],ids[0]]})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            "POST",
            "/api/v1/backgrounds/reorder",
            Some(&admin),
            serde_json::json!({"ids":[ids[0],99999]})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(order(&state.db).await, vec![ids[2], ids[1], ids[0]]);
}

#[tokio::test]
async fn articles_publish_privacy_and_background_admin_permissions() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (state, admin, member) = api_state().await;
    let app = crate::router(state).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let send = |method: &str, path: &str, token: Option<&str>, body: serde_json::Value| {
        let app = app.clone();
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = token {
            req = req.header("authorization", format!("Bearer {token}"));
        }
        async move {
            app.oneshot(req.body(Body::from(body.to_string())).unwrap())
                .await
                .unwrap()
        }
    };
    let mut data = serde_json::json!({"document":input("manual:article-test","# hello\n\n<script>alert(1)</script>\n\n[bad](javascript:alert%281%29)","private"),"slug":"test-article","description":"摘要","date":"2026-11-03","cover_url":"","is_pinned":true});
    assert_eq!(
        send(
            "POST",
            "/api/v1/admin/articles",
            Some(&member),
            data.clone()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send("POST", "/api/v1/admin/articles", Some(&admin), data.clone())
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            "GET",
            "/api/v1/articles/test-article",
            None,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    data["document"]["visibility"] = "public".into();
    assert_eq!(
        send("POST", "/api/v1/admin/articles", Some(&admin), data)
            .await
            .status(),
        StatusCode::OK
    );
    let response = send(
        "GET",
        "/api/v1/articles/test-article",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
    let html = body["html"].as_str().unwrap();
    assert!(!html.contains("<script>"));
    assert!(!html.contains("javascript:"));
    assert_eq!(body["meta"]["is_pinned"], true);
    let bg = serde_json::json!({"url":"/assets/test.jpg","poster_url":"","media_type":"image","theme":"dark","device":"mobile"});
    assert_eq!(
        send("POST", "/api/v1/backgrounds", None, bg.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send("POST", "/api/v1/backgrounds", Some(&admin), bg)
            .await
            .status(),
        StatusCode::OK
    );
    let response = send("GET", "/api/v1/backgrounds", None, serde_json::Value::Null).await;
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
    assert_eq!(body[0]["theme"], "dark");
    assert_eq!(body[0]["device"], "mobile");
    let ids = serde_json::json!({"ids":[body[0]["id"]]});
    assert_eq!(
        send(
            "POST",
            "/api/v1/backgrounds/reorder",
            Some(&member),
            ids.clone()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            "POST",
            "/api/v1/backgrounds/reorder",
            Some(&admin),
            ids.clone()
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            "POST",
            "/api/v1/backgrounds/batch-delete",
            Some(&admin),
            ids
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn media_admin_upload_list_serve_and_delete() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (mut state, admin, member) = api_state().await;
    let mut cfg = (*state.cfg).clone();
    cfg.data_dir = std::env::temp_dir().join(format!(
        "yukina-media-{}",
        yukina_media_service::uploads::random_file_name()
    ));
    let dir = cfg.data_dir.clone();
    state.cfg = Arc::new(cfg);
    let app = crate::router(state).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    for (method, path) in [
        ("GET", "/api/v1/admin/images"),
        ("POST", "/api/v1/admin/images"),
        ("POST", "/api/v1/admin/images/import"),
        ("DELETE", "/api/v1/admin/images/1"),
    ] {
        for (token, expected) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some(&member), StatusCode::FORBIDDEN),
        ] {
            let mut req = Request::builder().method(method).uri(path);
            if let Some(token) = token {
                req = req.header("authorization", format!("Bearer {token}"));
            }
            assert_eq!(
                app.clone()
                    .oneshot(req.body(Body::empty()).unwrap())
                    .await
                    .unwrap()
                    .status(),
                expected
            );
        }
    }
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(4, 3)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let mut multipart = b"--test-boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample.png\"\r\nContent-Type: image/png\r\n\r\n".to_vec();
    multipart.extend(png.into_inner());
    multipart.extend(b"\r\n--test-boundary--\r\n");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/images")
                .header("authorization", format!("Bearer {admin}"))
                .header(
                    "content-type",
                    "multipart/form-data; boundary=test-boundary",
                )
                .body(Body::from(multipart))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let image: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
    assert_eq!(image["width"], 4);
    let url = image["url"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/jpeg");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/images")
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/admin/images/{}", image["id"]))
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        app.oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    tokio::fs::create_dir_all(dir.join("backgrounds"))
        .await
        .unwrap();
    tokio::fs::write(dir.join("backgrounds/test.webm"), b"0123456789")
        .await
        .unwrap();
    // 独立路由复用同一临时目录，确认播放器 Range 请求只读取指定区间。
    let (mut video_state, _, _) = api_state().await;
    let mut config = (*video_state.cfg).clone();
    config.data_dir = dir.clone();
    video_state.cfg = Arc::new(config);
    let video_app = crate::routes::videos::router().with_state(video_state);
    let response = video_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/backgrounds/media/test.webm")
                .header("range", "bytes=2-4")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["content-range"], "bytes 2-4/10");
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"234"
    );
    let response = video_app
        .oneshot(
            Request::builder()
                .uri("/backgrounds/media/test.webm")
                .header("range", "bytes=30-40")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    tokio::fs::remove_dir_all(dir).await.unwrap();
}

#[tokio::test]
async fn document_sync_bypasses_rate_limit_but_keeps_admin_auth() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    let (state, admin, member) = api_state().await;
    let app = crate::router(state.clone()).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    for path in ["/api/v1/documents", "/api/v1/documents/reconcile"] {
        let body = if path.ends_with("/reconcile") {
            serde_json::json!({"source_keys": []})
        } else {
            serde_json::to_value(input("sync-test", "同步测试", "public")).unwrap()
        };
        for (token, expected) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some(&member), StatusCode::FORBIDDEN),
            (Some(&admin), StatusCode::OK),
        ] {
            while state.api_limiter.check("127.0.0.1").is_ok() {}
            let mut request = Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json");
            if let Some(token) = token {
                request = request.header("authorization", format!("Bearer {token}"));
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::from(body.to_string())).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{path}");
        }
    }
    for (method, path) in [
        ("GET", "/api/v1/documents"),
        ("GET", "/api/v1/search?q=test"),
        ("GET", "/api/v1/search/articles?q=test"),
        ("GET", "/api/v1/article-source?source_key=test"),
        ("POST", "/api/v1/admin/articles/generate-summary"),
        ("DELETE", "/api/v1/documents/1"),
        ("POST", "/api/v1/documents/1/reindex"),
    ] {
        while state.api_limiter.check("127.0.0.1").is_ok() {}
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("authorization", format!("Bearer {admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "{method} {path}"
        );
    }
}

#[tokio::test]
async fn http_routes_enforce_admin_and_public_search_hides_private_documents() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (state, admin, member) = api_state().await;
    let app = crate::router(state).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    for (token, expected) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some(member.as_str()), StatusCode::FORBIDDEN),
    ] {
        let mut request = Request::builder().uri("/api/v1/documents");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    let body = serde_json::to_vec(&input("http-test", "搜索路由的公开内容", "public")).unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/documents")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/search?q=http-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(body["mode"], "keyword");
    assert_eq!(body["results"].as_array().unwrap().len(), 1);
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/documents")
                .header("origin", "http://localhost:4321")
                .header("access-control-request-method", "POST")
                .header(
                    "access-control-request-headers",
                    "authorization,content-type",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .contains_key("access-control-allow-origin"));
}

#[tokio::test]
async fn failed_jobs_stop_after_three_attempts_and_reconcile_is_reversible() {
    let db = database().await;
    let doc = store::save(
        &db,
        input("astro-posts:gone.md", "需要保护的历史正文", "public"),
    )
    .await
    .unwrap();
    assert_eq!(store::reconcile(&db, vec![]).await.unwrap(), 1);
    assert!(run(&db, &disabled(), query("历史正文"))
        .await
        .unwrap()
        .results
        .is_empty());
    let restored = store::save(
        &db,
        input("astro-posts:gone.md", "需要保护的历史正文", "public"),
    )
    .await
    .unwrap();
    assert_eq!(restored.id, doc.id);
    assert_eq!(restored.visibility, "public");
    let broken = Embedder::new(SearchConfig {
        endpoint: Some("http://127.0.0.1:1/embeddings".into()),
        model: "test".into(),
        dimensions: 3,
        api_key: None,
    })
    .unwrap();
    let state = state(db, broken);
    for _ in 0..3 {
        state
            .db
            .execute(sql("UPDATE documents SET retry_at=0", vec![]))
            .await
            .unwrap();
        crate::documents::indexing::tick(&state).await.unwrap();
    }
    assert_eq!(
        store::get(&state.db, doc.id).await.unwrap().index_status,
        "failed"
    );
    store::reindex(&state.db, doc.id).await.unwrap();
    assert_eq!(
        store::get(&state.db, doc.id).await.unwrap().index_status,
        "queued"
    );
}

#[tokio::test]
async fn music_settings_permissions_modes_secrets_and_revision() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (state, admin, member) = api_state().await;
    let app = crate::router(state.clone()).layer(axum::Extension(axum::extract::ConnectInfo(
        "192.0.2.9:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let send = |method: &str, path: &str, token: Option<&str>, body: serde_json::Value| {
        let app = app.clone();
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        async move {
            app.oneshot(request.body(Body::from(body.to_string())).unwrap())
                .await
                .unwrap()
        }
    };
    let null = serde_json::Value::Null;
    assert_eq!(
        send("GET", "/api/v1/admin/music", None, null.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send("GET", "/api/v1/admin/music", Some(&member), null.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send("GET", "/api/v1/music/playlist", None, null.clone())
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let mut config =
        serde_json::to_value(yukina_media_service::music::Configuration::default()).unwrap();
    config["songs"] = serde_json::json!([{"id":"one","name":"First","artist":"Artist","url":"/test.mp3","cover":"","lrc":"[00:00.00]Hello"}]);
    let save = serde_json::json!({"configuration":config,"revision":0,"auth":"server-only-secret"});
    assert_eq!(
        send("PUT", "/api/v1/admin/music", Some(&member), save.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send("PUT", "/api/v1/admin/music", Some(&admin), save.clone())
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        send("PUT", "/api/v1/admin/music", Some(&admin), save)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let result = send("GET", "/api/v1/music/playlist", None, null.clone()).await;
    let public = String::from_utf8(
        to_bytes(result.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!public.contains("secret"));
    let parsed: serde_json::Value = serde_json::from_str(&public).unwrap();
    assert_eq!(parsed["playlist"][0]["lrc"], "[00:00.00]Hello");
    let admin_response = send("GET", "/api/v1/admin/music", Some(&admin), null.clone()).await;
    let body = to_bytes(admin_response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&body).contains("server-only-secret"));
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["has_auth"], true);
    config["mode"] = "meting".into();
    assert_eq!(
        send(
            "PUT",
            "/api/v1/admin/music",
            Some(&admin),
            serde_json::json!({"configuration":config,"revision":1})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    config["meting"]["id"] = "123".into();
    assert_eq!(
        send(
            "PUT",
            "/api/v1/admin/music",
            Some(&admin),
            serde_json::json!({"configuration":config,"revision":1})
        )
        .await
        .status(),
        StatusCode::OK
    );
    use sea_orm::EntityTrait;
    let saved = yukina_store::entities::music_setting::Entity::find_by_id(1)
        .one(&state.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.meting_auth, "server-only-secret");
    config["mode"] = "local".into();
    config["volume"] = 0.into();
    assert_eq!(
        send(
            "PUT",
            "/api/v1/admin/music",
            Some(&admin),
            serde_json::json!({"configuration":config,"revision":2,"clear_auth":true})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let result = send("POST", "/api/v1/admin/music/preview", Some(&admin), null).await;
    assert_eq!(result.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(result.into_body(), 2 * 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(body["playlist"][0]["name"], "First");
    assert_eq!(body["volume"].as_f64(), Some(0.0));
    assert!(yukina_store::entities::music_setting::Entity::find_by_id(1)
        .one(&state.db)
        .await
        .unwrap()
        .unwrap()
        .meting_auth
        .is_empty());
}

#[tokio::test]
async fn music_upload_streams_ranges_and_rejects_invalid_files() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (mut state, admin, member) = api_state().await;
    let mut cfg = (*state.cfg).clone();
    cfg.data_dir = std::env::temp_dir().join(format!(
        "yukina-music-test-{}",
        yukina_media_service::uploads::random_file_name()
    ));
    let dir = cfg.data_dir.clone();
    state.cfg = Arc::new(cfg);
    let app = crate::router(state).layer(axum::Extension(axum::extract::ConnectInfo(
        "192.0.2.9:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let send = |token: &str, name: &str, data: &[u8]| {
        let app = app.clone();
        let mut bytes=format!("--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
        bytes.extend_from_slice(data);
        bytes.extend_from_slice(b"\r\n--boundary--\r\n");
        let req = Request::post("/api/v1/admin/music/upload")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "multipart/form-data; boundary=boundary")
            .body(Body::from(bytes))
            .unwrap();
        async move { app.oneshot(req).await.unwrap() }
    };
    let audio = b"RIFF0000WAVEfmt 0123456789";
    assert_eq!(
        send(&member, "test.wav", audio).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&admin, "test.mp3", b"<html>invalid").await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(&admin, "test.lrc", b"[00:00.00]Hello").await.status(),
        StatusCode::OK
    );
    let res = send(&admin, "test.wav", audio).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(res.into_body(), 2048).await.unwrap()).unwrap();
    let url = body["url"].as_str().unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::get(url)
                .header("range", "bytes=0-3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(res.headers()["content-type"], "audio/wav");
    assert_eq!(&to_bytes(res.into_body(), 100).await.unwrap()[..], b"RIFF");
    let res = app
        .clone()
        .oneshot(
            Request::get(url)
                .header("range", "bytes=999-")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(std::fs::read_dir(dir.join("music")).unwrap().count(), 2);
    tokio::fs::remove_dir_all(dir).await.unwrap();
}

/// 手动浏览器验证夹具：内存库、临时上传目录和固定本机端口，四分钟后自动退出。
#[tokio::test]
#[ignore]
async fn admin_browser_preview() {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let (mut state, _, _) = api_state().await;
    let mut cfg = (*state.cfg).clone();
    cfg.data_dir = std::env::temp_dir().join(format!(
        "yukina-admin-preview-{}",
        yukina_media_service::uploads::random_file_name()
    ));
    let dir = cfg.data_dir.clone();
    state.cfg = Arc::new(cfg);
    use yukina_store::entities::user::{Column, Entity};
    Entity::update_many()
        .col_expr(
            Column::PasswordHash,
            sea_orm::sea_query::Expr::value(bcrypt::hash("preview-only-password", 4).unwrap()),
        )
        .filter(Column::Username.eq("test-admin"))
        .exec(&state.db)
        .await
        .unwrap();
    store::save(
        &state.db,
        input(
            "manual:preview",
            "# 内容管理\n\n这里可以维护后端文档，支持关键词与语义搜索。",
            "public",
        ),
    )
    .await
    .unwrap();
    // 隔离的演示素材，便于验证拖动、编辑和不限主题。
    use sea_orm::{ActiveModelTrait, Set};
    for (index, theme) in ["all", "light", "dark"].iter().enumerate() {
        yukina_store::entities::background::ActiveModel {
            url: Set(format!(
                "/assets/images/wallpaper/wallpaper-{:02}.webp",
                index + 1
            )),
            poster_url: Set(String::new()),
            media_type: Set("image".into()),
            theme: Set(theme.to_string()),
            device: Set("desktop".into()),
            sort_order: Set(index as i64),
            ..Default::default()
        }
        .insert(&state.db)
        .await
        .unwrap();
    }
    // 仪表盘预览使用隔离的演示访问，不进入正式数据库。
    for index in 0..18_i64 {
        yukina_store::entities::visit_event::ActiveModel {
            event_key: Set(format!("preview-{index}")),
            visitor_key: Set(format!("visitor-{}", index % 5)),
            path: Set(if index % 2 == 0 { "/" } else { "/guestbook/" }.into()),
            title: Set(if index % 2 == 0 {
                "首页"
            } else {
                "留言板"
            }
            .into()),
            referrer: Set("https://example.com/".into()),
            user_agent: Set("Preview browser".into()),
            ip_address: Set(format!("192.0.2.{}", index % 5 + 1)),
            occurred_at: Set(yukina_shared::serial::utc_now() - chrono::Duration::hours(index * 3)),
            ..Default::default()
        }
        .insert(&state.db)
        .await
        .unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8801")
        .await
        .unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            crate::router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_secs(240)).await;
    server.abort();
    let _ = tokio::fs::remove_dir_all(dir).await;
}

/// 显式运行：需要仓库的 Node 依赖和最新生产构建。使用内存库、临时端口，不触碰运行中的后端。
#[tokio::test]
#[ignore]
async fn sync_site_documents_through_real_http() {
    let (state, admin, _) = api_state().await;
    let app = crate::router(state.clone());
    let port = std::env::var("DOCUMENTS_TEST_PORT").unwrap_or_else(|_| "0".into());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let output = tokio::process::Command::new("node")
        .args([
            "node_modules/tsx/dist/cli.mjs",
            "scripts/sync-documents.ts",
            "--apply",
        ])
        .current_dir(root)
        .env("DOCUMENTS_API_URL", format!("http://{addr}"))
        .env("DOCUMENTS_ADMIN_TOKEN", admin)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let found = run(&state.db, &state.embeddings, query("博客"))
        .await
        .unwrap();
    assert!(!found.results.is_empty());
    assert_eq!(found.mode, "keyword");
    // 可选的短时浏览器验证窗口；仍只使用内存库，不写入真实运行数据。
    if let Ok(seconds) = std::env::var("DOCUMENTS_TEST_PREVIEW_SECONDS") {
        let seconds = seconds.parse::<u64>().unwrap_or(0).min(300);
        println!("Isolated document API ready at http://{addr}");
        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
    }
    server.abort();
}

#[tokio::test]
async fn talks_permissions_pins_edits_likes_comments_and_unpublish() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (state, admin, member) = api_state().await;
    // Keep integration assertions focused on features rather than the shared rate limiter.
    let app = crate::app::api_router()
        .with_state(state.clone())
        .layer(axum::Extension(axum::extract::ConnectInfo(
            "192.0.2.11:12345".parse::<std::net::SocketAddr>().unwrap(),
        )));
    let send = |method: &str, path: &str, token: Option<&str>, body: serde_json::Value| {
        let app = app.clone();
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("x-visitor-id", "talk-test-visitor");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        async move {
            app.oneshot(request.body(Body::from(body.to_string())).unwrap())
                .await
                .unwrap()
        }
    };
    async fn value(response: axum::response::Response) -> serde_json::Value {
        assert!(response.status().is_success(), "{}", response.status());
        serde_json::from_slice(&to_bytes(response.into_body(), 200000).await.unwrap()).unwrap()
    }
    let input =
        serde_json::json!({"content":"一条日常","images":[],"published":false,"is_pinned":false});
    assert_eq!(
        send("POST", "/api/v1/admin/talks", None, input.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send("POST", "/api/v1/admin/talks", Some(&member), input.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let mut first =
        value(send("POST", "/api/v1/admin/talks", Some(&admin), input.clone()).await).await;
    let id = first["id"].as_i64().unwrap();
    let path = format!("/talks/{id}/");
    let api = format!("/api/v1/admin/talks/{id}");
    assert_eq!(
        value(send("GET", "/api/v1/talks", None, serde_json::Value::Null).await).await["total"],
        0
    );
    assert_eq!(
        send(
            "GET",
            &format!("/api/v1/comments?page={path}"),
            None,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    first["published"] = true.into();
    first["is_pinned"] = true.into();
    first = value(send("PUT", &api, Some(&admin), first).await).await;
    let second_input =
        serde_json::json!({"content":"第二条","images":[],"published":true,"is_pinned":false});
    let second = value(send("POST", "/api/v1/admin/talks", Some(&admin), second_input).await).await;
    let feed = value(
        send(
            "GET",
            "/api/v1/talks?page_size=1",
            None,
            serde_json::Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(feed["total"], 2);
    assert_eq!(feed["items"][0]["id"], id);
    let page_two = value(
        send(
            "GET",
            "/api/v1/talks?page_size=1&page=2",
            None,
            serde_json::Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(page_two["items"][0]["id"], second["id"]);
    let like = value(
        send(
            "POST",
            "/api/v1/stats/like",
            None,
            serde_json::json!({"page":path}),
        )
        .await,
    )
    .await;
    assert_eq!(like["likes"], 1);
    assert_eq!(like["liked"], true);
    let feed = value(send("GET", "/api/v1/talks", None, serde_json::Value::Null).await).await;
    assert_eq!(feed["items"][0]["likes"], 1);
    assert_eq!(feed["items"][0]["liked"], true);
    let unlike = value(
        send(
            "POST",
            "/api/v1/stats/like",
            None,
            serde_json::json!({"page":path}),
        )
        .await,
    )
    .await;
    assert_eq!(unlike["likes"], 0);
    assert_eq!(unlike["liked"], false);
    let comment = value(
        send(
            "POST",
            "/api/v1/comments",
            None,
            serde_json::json!({"page":path,"nick":"路过的朋友","content":"日常也很美好"}),
        )
        .await,
    )
    .await;
    let comment_id = comment["id"].as_i64().unwrap();
    value(send("POST","/api/v1/comments",None,serde_json::json!({"page":path,"nick":"另一位朋友","content":"同感","parent_id":comment_id})).await).await;
    let comments = value(
        send(
            "GET",
            &format!("/api/v1/comments?page={path}"),
            None,
            serde_json::Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(comments.as_array().unwrap().len(), 2);
    let other = value(
        send(
            "GET",
            &format!(
                "/api/v1/comments?page=/talks/{}/",
                second["id"].as_i64().unwrap()
            ),
            None,
            serde_json::Value::Null,
        )
        .await,
    )
    .await;
    assert!(other.as_array().unwrap().is_empty());
    let stale = first.clone();
    first["content"] = "编辑后的内容".into();
    first = value(send("PUT", &api, Some(&admin), first).await).await;
    assert_eq!(first["content"], "编辑后的内容");
    assert_eq!(
        send("PUT", &api, Some(&admin), stale).await.status(),
        StatusCode::CONFLICT
    );
    first["published"] = false.into();
    first = value(send("PUT", &api, Some(&admin), first).await).await;
    assert_eq!(
        value(send("GET", "/api/v1/talks", None, serde_json::Value::Null).await).await["total"],
        1
    );
    assert_eq!(
        send(
            "GET",
            &format!("/api/v1/comments?page={path}"),
            None,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            "POST",
            "/api/v1/comments",
            None,
            serde_json::json!({"page":path,"nick":"访客","content":"隐藏后不能评论"})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            "POST",
            "/api/v1/stats/like",
            None,
            serde_json::json!({"page":path})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            "POST",
            &format!("/api/v1/comments/{comment_id}/like"),
            None,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    first["published"] = true.into();
    value(send("PUT", &api, Some(&admin), first).await).await;
    value(send("DELETE", &api, Some(&admin), serde_json::Value::Null).await).await;
    assert_eq!(
        value(
            send(
                "GET",
                "/api/v1/admin/talks",
                Some(&admin),
                serde_json::Value::Null
            )
            .await
        )
        .await["total"],
        1
    );
    assert_eq!(
        send(
            "GET",
            &format!("/api/v1/comments?page={path}"),
            None,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let bad = serde_json::json!({"content":"","images":["javascript:alert(1)"],"published":true,"is_pinned":false});
    assert_eq!(
        send("POST", "/api/v1/admin/talks", Some(&admin), bad)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(
            "POST",
            "/api/v1/admin/talks",
            Some(&admin),
            serde_json::json!({"content":"","images":[],"published":true,"is_pinned":false})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn simultaneous_talk_likes_keep_each_visitor_and_the_total() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use sea_orm::{ActiveModelTrait, Set};
    use tower::ServiceExt;
    let (state, _, _) = api_state().await;
    let now = yukina_shared::serial::utc_now();
    let talk = yukina_store::entities::talk::ActiveModel {
        content: Set("并发点赞".into()),
        images: Set("[]".into()),
        published: Set(true),
        is_pinned: Set(false),
        revision: Set(1),
        created_at: Set(now),
        updated_at: Set(now),
        published_at: Set(Some(now)),
        deleted_at: Set(None),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();
    let path = format!("/talks/{}/", talk.id);
    let app = crate::app::api_router().with_state(state.clone());
    for expected in [12, 0] {
        let mut tasks = Vec::new();
        for visitor in 0..12 {
            let app = app.clone();
            let path = path.clone();
            tasks.push(tokio::spawn(async move {
                let request = Request::builder()
                    .method("POST")
                    .uri("/api/v1/stats/like")
                    .header("content-type", "application/json")
                    .header("x-visitor-id", format!("parallel-visitor-{visitor}"))
                    .body(Body::from(serde_json::json!({"page":path}).to_string()))
                    .unwrap();
                assert!(app.oneshot(request).await.unwrap().status().is_success());
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/stats?page={path}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let response: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
        assert_eq!(response["likes"], expected);
    }
}

#[tokio::test]
async fn imported_articles_are_searchable_after_sync_and_legacy_backfill() {
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    let db = database().await;
    let mut imported = input("astro-posts:legacy/index.md", "同步文章检索", "public");
    imported.url = "/posts/legacy/".into();
    let public = store::save(&db, imported).await.unwrap();
    let mut public_row = yukina_store::entities::document::Entity::find_by_id(public.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(public_row.slug.as_deref(), Some("legacy"));
    let revision = public_row.revision;
    let mut summary_update = input("astro-posts:legacy/index.md", "同步文章检索", "public");
    summary_update.url = public_row.url.clone();
    summary_update.ai_summary = Some("同步的 AI 摘要".into());
    summary_update.date = Some("2026-11-09".into());
    store::save(&db, summary_update).await.unwrap();
    public_row = yukina_store::entities::document::Entity::find_by_id(public.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(public_row.revision, revision);
    assert_eq!(public_row.ai_summary, "同步的 AI 摘要");
    assert_eq!(public_row.date, "2026-11-09");
    let mut invalid_date = input("astro-posts:legacy/index.md", "无效更新不应落库", "public");
    invalid_date.date = Some("2026-02-30".into());
    assert!(store::save(&db, invalid_date).await.is_err());
    assert_eq!(
        yukina_store::entities::document::Entity::find_by_id(public.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        public_row
    );
    store::save(
        &db,
        input("astro-posts:draft.md", "同步文章检索", "private"),
    )
    .await
    .unwrap();
    store::save(&db, input("generic-reference", "同步文章检索", "public"))
        .await
        .unwrap();
    let find = || articles::ArticleSearchQuery {
        q: "同步文章检索".into(),
        limit: None,
        offset: None,
    };
    let result = articles::run(&db, find()).await.unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].id, public.id);
    // 模拟旧同步记录缺少 slug，升级修复且不改正文、版本、发布状态。
    yukina_store::entities::document::ActiveModel {
        id: Set(public.id),
        slug: Set(None),
        ..Default::default()
    }
    .update(&db)
    .await
    .unwrap();
    assert_eq!(articles::run(&db, find()).await.unwrap().total, 0);
    schema::migrate(&db).await.unwrap();
    schema::migrate(&db).await.unwrap();
    assert_eq!(articles::run(&db, find()).await.unwrap().total, 1);
    assert_eq!(
        yukina_store::entities::document::Entity::find_by_id(public.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        public_row
    );
    // 重复同步、更新内容后仍可搜索；取消公开后立即消失。
    store::save(
        &db,
        input("astro-posts:legacy/index.md", "同步文章检索 更新", "public"),
    )
    .await
    .unwrap();
    assert_eq!(articles::run(&db, find()).await.unwrap().total, 1);
    store::save(
        &db,
        input(
            "astro-posts:legacy/index.md",
            "同步文章检索 更新",
            "private",
        ),
    )
    .await
    .unwrap();
    assert_eq!(articles::run(&db, find()).await.unwrap().total, 0);
}

#[tokio::test]
async fn article_search_uses_live_published_articles_and_never_static_or_document_only_rows() {
    use axum::{
        body::{to_bytes, Body},
        http::{HeaderMap, Request, StatusCode},
        Router,
    };
    use tower::ServiceExt;
    async fn call(
        app: &Router,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: serde_json::Value,
    ) -> (StatusCode, HeaderMap, serde_json::Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            panic!(
                "{method} {path} returned {status}: {}",
                String::from_utf8_lossy(&bytes)
            )
        });
        (status, headers, value)
    }
    async fn find(app: &Router, q: &str, token: Option<&str>) -> serde_json::Value {
        let mut url = reqwest::Url::parse("http://localhost/api/v1/search/articles").unwrap();
        url.query_pairs_mut().append_pair("q", q);
        let (status, headers, value) = call(
            app,
            "GET",
            &format!("{}?{}", url.path(), url.query().unwrap()),
            token,
            serde_json::Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert_eq!(headers["cache-control"], "no-store");
        value
    }
    let (mut state, admin, _) = api_state().await;
    let mut rates = state.cfg.rate_limit.clone();
    rates.api_burst = 500;
    state.api_limiter = Arc::new(yukina_runtime::middleware::rate_limit::api_limiter(&rates));
    let db = state.db.clone();
    let app = crate::router(state).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let mut payload = serde_json::json!({"document": {"source_key":"manual:backend-search", "url":"/reader/?slug=backend-only", "title":"Needle 自动发布 100%_保障", "markdown":"# 正文\n\n独有测试正文 bodyneedle 新文章", "category":"部署", "tags":["Rust", "机器人"], "visibility":"private"}, "slug":"backend-only", "description":"数据库摘要", "ai_summary":"原始 AI 摘要", "date":"2026-11-08", "cover_url":"/uploads/live.png", "is_pinned":false});
    let (status, _, saved) = call(
        &app,
        "POST",
        "/api/v1/admin/articles",
        Some(&admin),
        payload.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(find(&app, "Needle", None).await["total"], 0);
    assert_eq!(find(&app, "Needle", Some(&admin)).await["total"], 0);
    payload.as_object_mut().unwrap().remove("ai_summary");
    payload["document"]["visibility"] = "public".into();
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/admin/articles",
            Some(&admin),
            payload.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    store::save(
        &db,
        input("document-only", "Needle bodyneedle 非文章文档", "public"),
    )
    .await
    .unwrap();
    let secondary = serde_json::json!({"document": {"source_key":"manual:body-result", "url":"/reader/?slug=body-only", "title":"普通文章", "markdown":"# 正文\n\nNeedle bodyneedle", "category":"其他", "tags":[], "visibility":"public"}, "slug":"body-only", "description":"其他摘要", "date":"2026-11-07", "cover_url":"", "is_pinned":true});
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/admin/articles",
            Some(&admin),
            secondary
        )
        .await
        .0,
        StatusCode::OK
    );
    let value = find(&app, "Needle", None).await;
    assert_eq!(value["total"], 2);
    assert_eq!(value["items"][0]["slug"], "backend-only");
    assert_eq!(value["items"][0]["url"], "/reader/?slug=backend-only");
    assert_eq!(value["items"][0]["cover_url"], "/uploads/live.png");
    assert_eq!(value["items"][0]["description"], "数据库摘要");
    assert_eq!(find(&app, "bodyneedle", None).await["total"], 2);
    assert_eq!(find(&app, "自动", None).await["total"], 1);
    assert_eq!(find(&app, "Rust", None).await["total"], 1);
    assert_eq!(find(&app, "部署", None).await["total"], 1);
    assert_eq!(find(&app, "数据库摘要", None).await["total"], 1);
    assert_eq!(find(&app, "Needle Rust", None).await["total"], 1);
    assert_eq!(find(&app, "Needle missing-phrase", None).await["total"], 0);
    assert_eq!(find(&app, "%", None).await["total"], 1);
    assert_eq!(find(&app, "_", None).await["total"], 1);
    assert_eq!(find(&app, "100%_", None).await["total"], 1);
    assert_eq!(find(&app, "' OR 1=1 --", None).await["total"], 0);
    let (status, _, first) = call(
        &app,
        "GET",
        "/api/v1/search/articles?q=Needle&limit=1",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["total"], 2);
    assert_eq!(first["items"].as_array().unwrap().len(), 1);
    let (_, _, second) = call(
        &app,
        "GET",
        "/api/v1/search/articles?q=Needle&limit=1&offset=1",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(second["items"][0]["slug"], "body-only");
    for path in [
        "/api/v1/search/articles?q=%20",
        "/api/v1/search/articles?q=needle&offset=100001",
    ] {
        assert_eq!(
            call(&app, "GET", path, None, serde_json::Value::Null)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    payload["document"]["title"] = "更新标题".into();
    payload["document"]["markdown"] = "更新后的 liveupdated 正文".into();
    payload["description"] = "更新的数据库摘要".into();
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/admin/articles",
            Some(&admin),
            payload.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(find(&app, "Needle", None).await["total"], 1);
    let edited = find(&app, "liveupdated", None).await;
    assert_eq!(edited["total"], 1);
    assert_eq!(edited["items"][0]["title"], "更新标题");
    assert_eq!(edited["items"][0]["description"], "更新的数据库摘要");
    let id = saved["id"].as_i64().unwrap();
    use sea_orm::EntityTrait;
    let before = yukina_store::entities::document::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    payload["is_pinned"] = true.into();
    payload["cover_url"] = "/uploads/new-cover.jpg".into();
    payload["date"] = "2026-11-09".into();
    let (status, _, updated) = call(
        &app,
        "POST",
        "/api/v1/admin/articles",
        Some(&admin),
        payload.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["id"], saved["id"]);
    let after = yukina_store::entities::document::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.revision, before.revision); // 只改展示字段不重建正文索引。
    assert!(after.is_pinned);
    assert_eq!(after.cover_url, "/uploads/new-cover.jpg");
    let (status, _, read) = call(
        &app,
        "GET",
        "/api/v1/articles/backend-only",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["meta"]["id"], saved["id"]);
    assert_eq!(read["meta"]["cover_url"], "/uploads/new-cover.jpg");
    assert!(read["html"].as_str().unwrap().contains("liveupdated"));
    let (status, _, list) = call(
        &app,
        "GET",
        "/api/v1/admin/articles",
        Some(&admin),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 2); // 不包含普通文档。
    let (status, _, list) = call(
        &app,
        "GET",
        "/api/v1/articles",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 2);
    let mut conflict = payload.clone();
    conflict["slug"] = "body-only".into();
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/admin/articles",
            Some(&admin),
            conflict
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    payload["slug"] = "renamed-article".into();
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/admin/articles",
            Some(&admin),
            payload.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/v1/articles/backend-only",
            None,
            serde_json::Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/v1/articles/renamed-article",
            None,
            serde_json::Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        find(&app, "liveupdated", None).await["items"][0]["url"],
        "/reader/?slug=renamed-article"
    );
    let source_url = "/api/v1/article-source?source_key=manual%3Abackend-search";
    let (status, headers, resolved) =
        call(&app, "GET", source_url, None, serde_json::Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(resolved["meta"]["slug"], "renamed-article");
    assert_eq!(resolved["title"], "更新标题");
    assert_eq!(resolved["meta"]["ai_summary"], "原始 AI 摘要");
    assert_eq!(resolved["source_key"], "manual:backend-search");
    assert!(resolved["html"].as_str().unwrap().contains("liveupdated"));
    assert!(db
        .query_one(sql(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='articles'",
            vec![]
        ))
        .await
        .unwrap()
        .is_none());
    payload["document"]["visibility"] = "private".into();
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/admin/articles",
            Some(&admin),
            payload.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(find(&app, "liveupdated", None).await["total"], 0);
    for token in [None, Some(admin.as_str())] {
        assert_eq!(
            call(&app, "GET", source_url, token, serde_json::Value::Null)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    payload["document"]["visibility"] = "public".into();
    payload["ai_summary"] = "".into();
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/admin/articles",
            Some(&admin),
            payload
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(find(&app, "liveupdated", None).await["total"], 1);
    let (_, _, cleared) = call(&app, "GET", source_url, None, serde_json::Value::Null).await;
    assert_eq!(cleared["meta"]["ai_summary"], "");
    store::remove(&db, saved["id"].as_i64().unwrap())
        .await
        .unwrap();
    assert_eq!(find(&app, "liveupdated", None).await["total"], 0);
    assert_eq!(
        call(&app, "GET", source_url, None, serde_json::Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn summary_generation_requires_admin_uses_editor_draft_and_never_saves_it() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        response::IntoResponse,
        routing::post,
    };
    use std::sync::Mutex;
    use tower::ServiceExt;
    let captured = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let observed = captured.clone();
    let upstream = Router::new().route("/completions", post(move |axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String,String>>, headers: axum::http::HeaderMap, Json(body): Json<serde_json::Value>| {
        let observed = observed.clone();
        async move {
            assert_eq!(headers["authorization"], "Bearer fixture-key");
            observed.lock().unwrap().push(body);
            match query.get("case").map(String::as_str).unwrap_or("ok") {
                "error" => (StatusCode::UNAUTHORIZED, "fixture-key: private upstream details").into_response(),
                "redirect" => (StatusCode::FOUND, [("location", "https://invalid.example/completions")]).into_response(),
                "invalid" => "not-json".into_response(),
                "huge" => "x".repeat(70000).into_response(),
                "empty" => Json(serde_json::json!({"choices":[{"message":{"content":""}}]})).into_response(),
                "partial" => Json(serde_json::json!({"choices":[{"finish_reason":"length","message":{"content":"未完成摘要"}}]})).into_response(),
                _ => Json(serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":"  根据当前正文生成的测试摘要。  "}}]})).into_response(),
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, upstream).await.unwrap();
    });
    let (mut state, admin, member) = api_state().await;
    let saved = store::save(
        &state.db,
        input("summary-private", "数据库里的旧正文", "private"),
    )
    .await
    .unwrap();
    let before = store::detail(&state.db, saved.id).await.unwrap();
    Arc::make_mut(&mut state.cfg).ai_summary = yukina_shared::summary_config::SummaryConfig {
        endpoint: Some(format!("http://{address}/completions")),
        model: "fixture-model".into(),
        api_key: Some("fixture-key".into()),
    };
    async fn send(
        state: &AppState,
        token: Option<&str>,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let app = crate::router(state.clone()).layer(axum::Extension(axum::extract::ConnectInfo(
            "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
        )));
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/admin/articles/generate-summary")
            .header("content-type", "application/json");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    let draft = serde_json::json!({"title":"尚未保存的标题", "markdown":"尚未保存的最新正文"});
    assert_eq!(
        send(&state, None, draft.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&state, Some(&member), draft.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    assert!(captured.lock().unwrap().is_empty());
    let (status, value) = send(&state, Some(&admin), draft.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["summary"], "根据当前正文生成的测试摘要。");
    assert_eq!(value["truncated"], false);
    {
        let requests = captured.lock().unwrap();
        assert_eq!(requests[0]["model"], "fixture-model");
        assert!(requests[0]["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("尚未保存的最新正文"));
        assert!(!requests[0]["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("数据库里的旧正文"));
    }
    let long =
        serde_json::json!({"title":"长文", "markdown":format!("开头{}结尾", "中".repeat(25000))});
    assert_eq!(send(&state, Some(&admin), long).await.1["truncated"], true);
    {
        let requests = captured.lock().unwrap();
        let text = requests.last().unwrap()["messages"][1]["content"]
            .as_str()
            .unwrap();
        assert!(text.contains("开头") && text.contains("结尾") && text.chars().count() < 25000);
    }
    for case in ["error", "redirect", "invalid", "huge", "empty", "partial"] {
        Arc::make_mut(&mut state.cfg).ai_summary.endpoint =
            Some(format!("http://{address}/completions?case={case}"));
        let (status, value) = send(&state, Some(&admin), draft.clone()).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{case}: {value}");
        assert!(!value.to_string().contains("fixture-key"));
    }
    for invalid in [
        serde_json::json!({"title":"", "markdown":"正文"}),
        serde_json::json!({"title":"文章", "markdown":" "}),
        serde_json::json!({"title":"文章", "markdown":"x".repeat(1024*1024+1)}),
    ] {
        assert_eq!(
            send(&state, Some(&admin), invalid).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    Arc::make_mut(&mut state.cfg).ai_summary.endpoint = None;
    assert_eq!(
        send(&state, Some(&admin), draft.clone()).await.0,
        StatusCode::BAD_REQUEST
    );
    Arc::make_mut(&mut state.cfg).ai_summary.endpoint =
        Some("http://invalid.example/completions".into());
    assert_eq!(
        send(&state, Some(&admin), draft).await.0,
        StatusCode::BAD_REQUEST
    );
    let after = store::detail(&state.db, saved.id).await.unwrap();
    assert_eq!(after.markdown, before.markdown);
    assert_eq!(after.document.revision, before.document.revision);
    assert_eq!(after.document.visibility, "private");
    use sea_orm::EntityTrait;
    assert!(
        yukina_store::entities::document::Entity::find_by_id(saved.id)
            .one(&state.db)
            .await
            .unwrap()
            .unwrap()
            .ai_summary
            .is_empty()
    );
    assert!(!format!("{:?}", state.cfg.ai_summary).contains("fixture-key"));
    server.abort();
}

#[tokio::test]
async fn gallery_image_reads_do_not_share_the_business_api_rate_limit() {
    use axum::{
        body::{to_bytes, Body},
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt;

    let (mut state, admin, _) = api_state().await;
    let directory = std::env::temp_dir().join(format!(
        "yukina-image-rate-test-{}",
        yukina_media_service::uploads::random_file_name()
    ));
    let uploads = directory.join("uploads");
    std::fs::create_dir_all(&uploads).unwrap();
    Arc::make_mut(&mut state.cfg).data_dir = directory.clone();
    let mut rates = state.cfg.rate_limit.clone();
    rates.api_per_second = 1;
    rates.api_burst = 1;
    state.api_limiter = Arc::new(yukina_runtime::middleware::rate_limit::api_limiter(&rates));
    let app = crate::router(state.clone()).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:23456".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let mut image = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(4, 3)
        .write_to(&mut image, image::ImageFormat::WebP)
        .unwrap();
    let bytes = image.into_inner();
    std::fs::write(uploads.join("test.webp"), &bytes).unwrap();

    // Loading many thumbnails must not consume even the single API token.
    for _ in 0..32 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/uploads/test.webp")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/webp");
        assert_eq!(
            to_bytes(response.into_body(), 10000)
                .await
                .unwrap()
                .as_ref(),
            bytes
        );
    }
    assert!(state.api_limiter.check("127.0.0.1").is_ok());

    // Image reads still work after business requests have exhausted the quota.
    for method in [Method::GET, Method::HEAD] {
        while state.api_limiter.check("127.0.0.1").is_ok() {}
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/api/v1/uploads/test.webp")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    for (method, path) in [
        ("GET", "/api/v1/admin/images"),
        ("POST", "/api/v1/admin/images"),
        ("POST", "/api/v1/admin/images/import"),
        ("DELETE", "/api/v1/admin/images/1"),
        ("GET", "/api/v1/search/articles?q=test"),
        ("POST", "/api/v1/auth/login"),
    ] {
        while state.api_limiter.check("127.0.0.1").is_ok() {}
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("authorization", format!("Bearer {admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "{method} {path}"
        );
    }
    std::fs::remove_file(uploads.join("test.webp")).unwrap();
    std::fs::remove_dir(uploads).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn original_article_images_keep_bytes_deduplicate_and_cannot_be_deleted_while_referenced() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (mut state, admin, member) = api_state().await;
    let directory = std::env::temp_dir().join(format!(
        "yukina-original-images-{}",
        yukina_media_service::uploads::random_file_name()
    ));
    Arc::make_mut(&mut state.cfg).data_dir = directory.clone();
    let db = state.db.clone();
    let app = crate::router(state).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:23456".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(4, 3)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let bytes = png.into_inner();
    let mut multipart = b"--test-image\r\nContent-Disposition: form-data; name=\"file\"; filename=\"transparent.png\"\r\nContent-Type: image/png\r\n\r\n".to_vec();
    multipart.extend(&bytes);
    multipart.extend(b"\r\n--test-image--\r\n");
    async fn upload(app: &Router, token: Option<&str>, body: Vec<u8>) -> axum::response::Response {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/admin/images/import")
            .header("content-type", "multipart/form-data; boundary=test-image");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        app.clone()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap()
    }
    assert_eq!(
        upload(&app, None, multipart.clone()).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        upload(&app, Some(&member), multipart.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = upload(&app, Some(&admin), multipart.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let saved: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
    let again = upload(&app, Some(&admin), multipart).await;
    let again: serde_json::Value =
        serde_json::from_slice(&to_bytes(again.into_body(), 10000).await.unwrap()).unwrap();
    assert_eq!(saved["id"], again["id"]);
    let url = saved["url"].as_str().unwrap();
    assert!(url.ends_with(".png"));
    let response = app
        .clone()
        .oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(
        to_bytes(response.into_body(), 10000)
            .await
            .unwrap()
            .as_ref(),
        bytes.as_slice()
    );
    let document = store::save(
        &db,
        input(
            "image-reference",
            &format!("![保留透明度]({url})"),
            "private",
        ),
    )
    .await
    .unwrap();
    let delete = || {
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/admin/images/{}", saved["id"]))
            .header("authorization", format!("Bearer {admin}"))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        app.clone().oneshot(delete()).await.unwrap().status(),
        StatusCode::CONFLICT
    );
    store::remove(&db, document.id).await.unwrap();
    assert_eq!(
        app.clone().oneshot(delete()).await.unwrap().status(),
        StatusCode::OK
    );
    assert_eq!(
        app.oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    std::fs::remove_dir(directory.join("uploads")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn managed_site_permissions_revision_visibility_and_live_feed() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (state, admin, member) = api_state().await;
    let app = crate::router(state.clone()).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:23456".parse::<std::net::SocketAddr>().unwrap(),
    )));
    async fn send(
        app: &Router,
        method: &str,
        path: &str,
        token: Option<&str>,
        data: serde_json::Value,
    ) -> (StatusCode, Vec<u8>) {
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = token {
            r = r.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .clone()
            .oneshot(r.body(Body::from(data.to_string())).unwrap())
            .await
            .unwrap();
        (
            response.status(),
            to_bytes(response.into_body(), 2_000_000)
                .await
                .unwrap()
                .to_vec(),
        )
    }
    for path in [
        "/api/v1/admin/site/settings",
        "/api/v1/admin/site/friends",
        "/api/v1/admin/site/page-about",
        "/api/v1/admin/comments",
    ] {
        assert_eq!(
            send(&app, "GET", path, None, serde_json::Value::Null)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            send(&app, "GET", path, Some(&member), serde_json::Value::Null)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    // Keep the rest of this integration test focused on authorization and content behavior.
    let mut rates = state.cfg.rate_limit.clone();
    rates.api_burst = 1000;
    let mut state = state;
    state.api_limiter = Arc::new(yukina_runtime::middleware::rate_limit::api_limiter(&rates));
    let app = crate::router(state.clone()).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:23456".parse::<std::net::SocketAddr>().unwrap(),
    )));
    let page = serde_json::json!({"title":"About","markdown":"# Safe\n<script>alert(1)</script>\n![img](/api/v1/uploads/a.png)","published":false});
    for (token, code) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some(member.as_str()), StatusCode::FORBIDDEN),
        (Some(admin.as_str()), StatusCode::OK),
    ] {
        assert_eq!(
            send(
                &app,
                "PUT",
                "/api/v1/admin/site/page-about",
                token,
                serde_json::json!({"data":page,"revision":0})
            )
            .await
            .0,
            code
        );
    }
    assert_eq!(
        send(
            &app,
            "GET",
            "/api/v1/pages/about",
            None,
            serde_json::Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let mut public_page = page.clone();
    public_page["published"] = true.into();
    assert_eq!(
        send(
            &app,
            "PUT",
            "/api/v1/admin/site/page-about",
            Some(&admin),
            serde_json::json!({"data":public_page,"revision":1})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &app,
            "PUT",
            "/api/v1/admin/site/page-about",
            Some(&admin),
            serde_json::json!({"data":page,"revision":1})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, body) = send(
        &app,
        "GET",
        "/api/v1/pages/about",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(!value["html"].as_str().unwrap().contains("<script>"));
    assert!(value["html"]
        .as_str()
        .unwrap()
        .contains("/api/v1/uploads/a.png"));
    let friends = serde_json::json!([{"id":"a","title":"Published","description":"ok","siteurl":"https://example.org","imgurl":"","rss":"","enabled":true,"weight":1},{"id":"b","title":"Hidden","description":"secret","siteurl":"https://example.net","imgurl":"","rss":"","enabled":false,"weight":10}]);
    assert_eq!(
        send(
            &app,
            "PUT",
            "/api/v1/admin/site/friends",
            Some(&admin),
            serde_json::json!({"data":friends,"revision":0})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, body) = send(
        &app,
        "GET",
        "/api/v1/friends",
        None,
        serde_json::Value::Null,
    )
    .await;
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["items"].as_array().unwrap().len(), 1);
    assert_eq!(value["items"][0]["title"], "Published");
    let mut invalid = friends.clone();
    invalid[0]["siteurl"] = "javascript:alert(1)".into();
    assert_eq!(
        send(
            &app,
            "PUT",
            "/api/v1/admin/site/friends",
            Some(&admin),
            serde_json::json!({"data":invalid,"revision":1})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let doc = store::save(
        &state.db,
        input("manual:live-index", "Unique live index title", "public"),
    )
    .await
    .unwrap();
    use sea_orm::{ActiveModelTrait, Set};
    yukina_store::entities::document::ActiveModel {
        id: Set(doc.id),
        slug: Set(Some("live-index".into())),
        title: Set("Unique live index title".into()),
        date: Set("2026-11-10".into()),
        tags: Set("[\"BackendOnly\"]".into()),
        ..Default::default()
    }
    .update(&state.db)
    .await
    .unwrap();
    let (_, body) = send(
        &app,
        "GET",
        "/api/v1/articles",
        None,
        serde_json::Value::Null,
    )
    .await;
    let rows: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(rows
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["tags"] == serde_json::json!(["BackendOnly"])));
    let (_, body) = send(
        &app,
        "GET",
        "/api/v1/feed.xml",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert!(String::from_utf8(body)
        .unwrap()
        .contains("Unique live index title"));
    store::save(
        &state.db,
        input("manual:live-index", "Unique live index title", "private"),
    )
    .await
    .unwrap();
    for path in [
        "/api/v1/articles",
        "/api/v1/feed.xml",
        "/api/v1/sitemap.xml",
    ] {
        let (_, body) = send(&app, "GET", path, None, serde_json::Value::Null).await;
        let body = String::from_utf8(body).unwrap();
        assert!(!body.contains("Unique live index title") && !body.contains("slug=live-index"));
    }
}
