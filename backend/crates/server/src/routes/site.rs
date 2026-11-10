//! 站点公开配置与后台内容：按条目保存，修订号防止并发覆盖。
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use sea_orm::{sea_query::OnConflict, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use serde_json::{json, Value};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::{
    error::{ApiError, ApiResult},
    serial::utc_now,
};
use yukina_store::entities::site_entry::{self, Entity as Entries};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/site", get(public_settings))
        .route("/site-bootstrap.js", get(bootstrap_script))
        .route("/admin/page-preview", axum::routing::post(preview))
        .route("/friends", get(public_friends))
        .route("/friends/feed", get(super::friends_feed::feed))
        .route("/pages/{slug}", get(public_page))
        .route("/admin/site/{key}", get(read).put(save))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}
fn public(data: Value) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Json(data)).into_response()
}
pub async fn data(db: &impl ConnectionTrait, key: &str) -> ApiResult<Option<Value>> {
    Entries::find_by_id(key)
        .one(db)
        .await?
        .map(|row| serde_json::from_str(&row.data).map_err(|e| ApiError::Internal(e.to_string())))
        .transpose()
}
async fn public_settings(State(state): State<AppState>) -> ApiResult<Response> {
    Ok(public(
        json!({"configuration":data(&state.db, "settings").await?}),
    ))
}
async fn public_friends(State(state): State<AppState>) -> ApiResult<Response> {
    let configured = data(&state.db, "friends").await?;
    let mut rows: Vec<Value> = configured
        .as_ref()
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r["enabled"].as_bool().unwrap_or(true))
        .collect();
    rows.sort_by_key(|r| -r["weight"].as_i64().unwrap_or(0));
    Ok(public(
        json!({"configured":configured.is_some(),"items":rows}),
    ))
}
async fn public_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResult<Response> {
    let key = format!("page-{slug}");
    valid_key(&key)?;
    let page = data(&state.db, &key)
        .await?
        .ok_or_else(|| ApiError::NotFound("页面尚未配置".into()))?;
    if page["published"] != true {
        return Err(ApiError::NotFound("页面未发布".into()));
    }
    Ok(public(
        json!({"title":page["title"],"markdown":page["markdown"],"html":crate::routes::content::markdown_html(page["markdown"].as_str().unwrap_or(""))}),
    ))
}
fn valid_key(key: &str) -> ApiResult<()> {
    if matches!(key, "settings" | "friends")
        || (key.starts_with("page-")
            && (6..=100).contains(&key.len())
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        Ok(())
    } else {
        Err(ApiError::BadRequest("配置名称不正确".into()))
    }
}
async fn read(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(key): Path<String>,
) -> ApiResult<Json<Value>> {
    valid_key(&key)?;
    let row = Entries::find_by_id(&key).one(&state.db).await?;
    Ok(Json(match row {
        Some(r) => {
            json!({"data":serde_json::from_str::<Value>(&r.data).unwrap_or(Value::Null),"revision":r.revision})
        }
        None => json!({"data":null,"revision":0}),
    }))
}
#[derive(Deserialize)]
struct Input {
    data: Value,
    revision: i64,
}
async fn save(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(key): Path<String>,
    Json(input): Json<Input>,
) -> ApiResult<Json<Value>> {
    valid_key(&key)?;
    if input.revision < 0 || input.revision == i64::MAX {
        return Err(ApiError::Unprocessable("配置版本不正确".into()));
    }
    validate(&key, &input.data)?;
    let serialized =
        serde_json::to_string(&input.data).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    if input.revision == 0 {
        Entries::insert(site_entry::ActiveModel {
            key: Set(key.clone()),
            data: Set(serialized),
            revision: Set(1),
            updated_at: Set(utc_now()),
        })
        .on_conflict(
            OnConflict::column(site_entry::Column::Key)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(&state.db)
        .await?;
        let row = Entries::find_by_id(&key)
            .one(&state.db)
            .await?
            .ok_or_else(|| ApiError::Internal("配置保存失败".into()))?;
        // An existing entry must never be overwritten by a stale first save.
        if row.revision != 1
            || serde_json::from_str::<Value>(&row.data).ok().as_ref() != Some(&input.data)
        {
            return Err(ApiError::Conflict("配置已被修改，请重新加载".into()));
        }
    } else {
        let result = Entries::update_many()
            .set(site_entry::ActiveModel {
                data: Set(serialized),
                revision: Set(input.revision + 1),
                updated_at: Set(utc_now()),
                ..Default::default()
            })
            .filter(site_entry::Column::Key.eq(&key))
            .filter(site_entry::Column::Revision.eq(input.revision))
            .exec(&state.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(ApiError::Conflict("配置已被修改，请重新加载".into()));
        }
    }
    Ok(Json(json!({"revision":input.revision+1})))
}
fn text<'a>(v: &'a Value, key: &str, max: usize) -> ApiResult<&'a str> {
    v[key]
        .as_str()
        .filter(|s| s.chars().count() <= max && !s.contains('\0'))
        .ok_or_else(|| ApiError::Unprocessable(format!("{key} 内容不正确或过长")))
}
fn url(s: &str) -> bool {
    yukina_shared::validation::valid_url(s)
        || (s.starts_with("http://")
            && s.len() <= 2000
            && !s.chars().any(char::is_control)
            && !s.contains('\\'))
}
fn check_url(v: &Value, key: &str) -> ApiResult<()> {
    if url(text(v, key, 2000)?) {
        Ok(())
    } else {
        Err(ApiError::Unprocessable(format!("{key} 地址不正确")))
    }
}
fn array(v: &Value, max: usize) -> ApiResult<&Vec<Value>> {
    v.as_array()
        .filter(|a| a.len() <= max)
        .ok_or_else(|| ApiError::Unprocessable("列表格式不正确或数量过多".into()))
}
fn links(rows: &Value, depth: usize) -> ApiResult<()> {
    for row in array(rows, 50)? {
        text(row, "name", 80)?;
        let s = text(row, "url", 2000)?;
        if !url(s) && s != "#" && !s.starts_with("mailto:") && !s.starts_with("copy:") {
            return Err(ApiError::Unprocessable("链接地址不正确".into()));
        }
        if let Some(c) = row.get("children") {
            if depth == 0 {
                return Err(ApiError::Unprocessable("导航最多两级".into()));
            }
            links(c, depth - 1)?;
        }
    }
    Ok(())
}
pub fn validate(key: &str, v: &Value) -> ApiResult<()> {
    valid_key(key)?;
    if key == "friends" {
        for f in array(v, 500)? {
            text(f, "title", 100)?;
            text(f, "description", 2000)?;
            for field in ["siteurl", "imgurl", "rss"] {
                check_url(f, field)?;
            }
            let weight = f["weight"].as_i64().unwrap_or(0);
            if !(-100000..=100000).contains(&weight) {
                return Err(ApiError::Unprocessable("排序值超出范围".into()));
            }
        }
    } else if key.starts_with("page-") {
        if text(v, "title", 200)?.trim().is_empty() || !v["published"].is_boolean() {
            return Err(ApiError::Unprocessable("请填写页面标题与发布状态".into()));
        }
        text(v, "markdown", 500000)?;
    } else {
        let b = &v["basic"];
        if text(b, "title", 100)?.trim().is_empty() {
            return Err(ApiError::Unprocessable("站点名称不能为空".into()));
        }
        for field in ["subtitle", "description", "keywords"] {
            text(b, field, 4000)?;
        }
        for field in ["site_url", "logo", "favicon"] {
            check_url(b, field)?;
        }
        let site = text(b, "site_url", 2000)?;
        if reqwest::Url::parse(site).ok().is_none_or(|u| {
            !matches!(u.scheme(), "http" | "https")
                || u.host_str().is_none()
                || !u.username().is_empty()
                || u.password().is_some()
                || u.query().is_some()
                || u.fragment().is_some()
        }) {
            return Err(ApiError::Unprocessable(
                "站点地址需填写完整的 http(s) 地址".into(),
            ));
        }
        if chrono::NaiveDate::parse_from_str(text(b, "start_date", 10)?, "%Y-%m-%d").is_err() {
            return Err(ApiError::Unprocessable("建站日期不正确".into()));
        }
        let p = &v["profile"];
        text(p, "name", 100)?;
        text(p, "bio", 4000)?;
        check_url(p, "avatar")?;
        links(&p["links"], 0)?;
        let a = &v["announcement"];
        text(a, "title", 200)?;
        text(a, "content", 10000)?;
        links(&a["links"], 0)?;
        links(&v["navigation"], 1)?;
        text(&v["footer"], "copyright", 2000)?;
        links(&v["footer"]["records"], 0)?;
        let a = &v["appearance"];
        for (field, min, max) in [
            ("hue", 0., 360.),
            ("sakura_count", 0., 100.),
            ("sakura_speed", 0.2, 3.),
            ("carousel_interval", 3., 120.),
            ("posts_per_page", 1., 50.),
        ] {
            if !a[field]
                .as_f64()
                .is_some_and(|n| n.is_finite() && (min..=max).contains(&n))
            {
                return Err(ApiError::Unprocessable(format!("{field} 超出范围")));
            }
        }
        for (field, choices) in [
            ("theme", &["yukina", "pure"][..]),
            ("mode", &["light", "dark", "system"]),
            ("list_mode", &["grid", "list"]),
            (
                "wallpaper_mode",
                &["banner", "fullscreen", "overlay", "none"],
            ),
            ("navbar_behavior", &["auto", "static"]),
            ("card_decoration", &["soft", "flat"]),
        ] {
            if !a[field].as_str().is_some_and(|s| choices.contains(&s)) {
                return Err(ApiError::Unprocessable(format!("{field} 不正确")));
            }
        }
        for field in ["intro_enabled", "sakura_enabled"] {
            if !a[field].is_boolean() {
                return Err(ApiError::Unprocessable(format!("{field} 不正确")));
            }
        }
        for w in array(&v["sidebar"]["widgets"], 40)? {
            if ![
                "profile",
                "announcement",
                "categories",
                "tags",
                "sidebarToc",
                "stats",
                "calendar",
                "music",
                "siteInfo",
                "timeGreeting",
                "quoteOfTheDay",
                "scheduleProgress",
            ]
            .contains(&text(w, "type", 40)?)
                || !["left", "right"].contains(&text(w, "side", 10)?)
            {
                return Err(ApiError::Unprocessable("侧栏组件不正确".into()));
            }
        }
    }
    Ok(())
}
pub async fn import_defaults(
    settings: &yukina_shared::config::Settings,
    path: &std::path::Path,
) -> ApiResult<()> {
    let values: std::collections::BTreeMap<String, Value> =
        serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    for (key, value) in &values {
        validate(key, value)?;
    }
    let db = yukina_store::db::create_connection(settings).await?;
    yukina_store::migrations::run(&db).await?;
    use sea_orm::TransactionTrait;
    let tx = db.begin().await?;
    for (key, value) in values {
        Entries::insert(site_entry::ActiveModel {
            key: Set(key),
            data: Set(value.to_string()),
            revision: Set(1),
            updated_at: Set(utc_now()),
        })
        .on_conflict(
            OnConflict::column(site_entry::Column::Key)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(&tx)
        .await?;
    }
    tx.commit().await?;
    println!("站点配置导入完成，已有后台配置保持不变");
    Ok(())
}

async fn preview(AdminUser(_): AdminUser, Json(input): Json<Value>) -> ApiResult<Json<Value>> {
    let markdown = text(&input, "markdown", 500000)?;
    Ok(Json(
        json!({"html":super::content::markdown_html(markdown)}),
    ))
}

async fn bootstrap_script(State(state): State<AppState>) -> ApiResult<Response> {
    let value = data(&state.db, "settings").await?.unwrap_or(Value::Null);
    let json = value
        .to_string()
        .replace('<', "\\u003c")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        format!("window.__yukinaSiteDefaults={json};"),
    )
        .into_response())
}
