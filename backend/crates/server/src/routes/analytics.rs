use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, Query, State},
    http::HeaderMap,
    Json, Router,
};
use chrono::{Duration, NaiveDate, NaiveDateTime};
use sea_orm::{
    sea_query::{Alias, Expr, Func, OnConflict},
    ColumnTrait, Condition, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::{
    error::{ApiError, ApiResult},
    serial::utc_now,
};
use yukina_store::entities::{
    background, comment, document, media_image, user,
    visit_event::{self, Column as Col, Entity as Events},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/analytics/events", axum::routing::post(record))
        .route("/admin/dashboard", axum::routing::get(dashboard))
        .route("/admin/visits", axum::routing::get(visits))
        .layer(DefaultBodyLimit::max(16 * 1024))
}
#[derive(Deserialize)]
struct VisitInput {
    event_id: String,
    visitor_id: String,
    path: String,
    title: String,
    #[serde(default)]
    referrer: String,
}
async fn record(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<VisitInput>,
) -> ApiResult<Json<serde_json::Value>> {
    if !(6..=100).contains(&input.visitor_id.len())
        || !(6..=100).contains(&input.event_id.len())
        || input.path.len() > 2000
        || input.title.chars().count() > 300
    {
        return Err(ApiError::BadRequest("访问事件格式不正确".into()));
    }
    let path = yukina_shared::request::normalize_page(&input.path)?;
    if path == "/admin/"
        || path == "/admin"
        || path.starts_with("/admin/")
        || path.starts_with("/api/")
    {
        return Err(ApiError::BadRequest("不记录后台或接口访问".into()));
    }
    let visitor_key = hex::encode(Sha256::digest(format!(
        "{}:{}",
        state.cfg.secret_key, input.visitor_id
    )));
    let event_key = hex::encode(Sha256::digest(format!("{visitor_key}:{}", input.event_id)));
    let ip = yukina_runtime::ip_region::client_ip(&headers, addr)
        .parse::<std::net::IpAddr>()
        .unwrap_or(addr.ip())
        .to_string();
    let referrer = if input.referrer.is_empty() {
        String::new()
    } else {
        reqwest::Url::parse(&input.referrer)
            .ok()
            .filter(|u| matches!(u.scheme(), "http" | "https"))
            .map(|mut u| {
                u.set_query(None);
                u.set_fragment(None);
                let _ = u.set_username("");
                let _ = u.set_password(None);
                u.to_string().chars().take(1000).collect()
            })
            .unwrap_or_default()
    };
    Events::insert(visit_event::ActiveModel {
        event_key: Set(event_key),
        visitor_key: Set(visitor_key),
        path: Set(path),
        title: Set(input.title),
        referrer: Set(referrer),
        user_agent: Set(headers
            .get("user-agent")
            .and_then(|s| s.to_str().ok())
            .unwrap_or("")
            .chars()
            .take(500)
            .collect()),
        ip_address: Set(ip),
        occurred_at: Set(utc_now()),
        ..Default::default()
    })
    .on_conflict(OnConflict::column(Col::EventKey).do_nothing().to_owned())
    .exec_without_returning(&state.db)
    .await?;
    Ok(Json(serde_json::json!({"recorded":true})))
}
#[derive(Default, Deserialize)]
struct VisitsQuery {
    page: Option<u64>,
    page_size: Option<u64>,
    ip: Option<String>,
    path: Option<String>,
    start: Option<String>,
    end: Option<String>,
}
fn boundary(value: Option<&str>, end: bool) -> ApiResult<Option<NaiveDateTime>> {
    value
        .filter(|s| !s.is_empty())
        .map(|s| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|d| {
                    d.and_hms_opt(0, 0, 0).unwrap() - Duration::hours(8)
                        + if end {
                            Duration::days(1)
                        } else {
                            Duration::zero()
                        }
                })
                .map_err(|_| ApiError::BadRequest("日期格式应为YYYY-MM-DD".into()))
        })
        .transpose()
}
#[derive(Serialize, FromQueryResult)]
struct VisitRow {
    id: i64,
    path: String,
    title: String,
    referrer: String,
    user_agent: String,
    ip_address: String,
    occurred_at: NaiveDateTime,
}
async fn visits(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Query(q): Query<VisitsQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut find = Events::find();
    if let Some(ip) = q.ip.filter(|s| !s.is_empty()) {
        let ip = ip
            .parse::<std::net::IpAddr>()
            .map_err(|_| ApiError::BadRequest("IP地址格式不正确".into()))?;
        find = find.filter(Col::IpAddress.eq(ip.to_string()));
    }
    if let Some(path) = q.path.filter(|s| !s.is_empty()) {
        if path.chars().count() > 300 {
            return Err(ApiError::BadRequest("页面筛选过长".into()));
        }
        find = find.filter(
            Condition::any()
                .add(Col::Path.contains(&path))
                .add(Col::Title.contains(&path)),
        );
    }
    let start = boundary(q.start.as_deref(), false)?;
    let end = boundary(q.end.as_deref(), true)?;
    if start.zip(end).is_some_and(|(a, b)| a >= b) {
        return Err(ApiError::BadRequest("开始日期不能晚于结束日期".into()));
    }
    if let Some(start) = start {
        find = find.filter(Col::OccurredAt.gte(start));
    }
    if let Some(end) = end {
        find = find.filter(Col::OccurredAt.lt(end));
    }
    let total = find.clone().count(&state.db).await?;
    let size = q.page_size.unwrap_or(30).clamp(1, 100);
    let page = q.page.unwrap_or(1).max(1);
    let rows = find
        .select_only()
        .columns([
            Col::Id,
            Col::Path,
            Col::Title,
            Col::Referrer,
            Col::UserAgent,
            Col::IpAddress,
            Col::OccurredAt,
        ])
        .order_by_desc(Col::OccurredAt)
        .order_by_desc(Col::Id)
        .offset(page.saturating_sub(1).saturating_mul(size))
        .limit(size)
        .into_model::<VisitRow>()
        .all(&state.db)
        .await?;
    let rows = rows
        .into_iter()
        .map(|row| {
            let region = yukina_runtime::ip_region::resolve_region(&state, Some(&row.ip_address));
            let mut value = serde_json::to_value(row).unwrap();
            value["region"] = serde_json::json!(region);
            value["event_type"] = "page_view".into();
            value
        })
        .collect::<Vec<_>>();
    Ok(Json(
        serde_json::json!({"items":rows,"total":total,"page":page,"page_size":size}),
    ))
}
#[derive(FromQueryResult, Serialize)]
struct Counts {
    pv: i64,
    uv: i64,
}
async fn counts(
    db: &sea_orm::DatabaseConnection,
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
) -> ApiResult<Counts> {
    let mut q = Events::find();
    if let Some(start) = start {
        q = q.filter(Col::OccurredAt.gte(start));
    }
    if let Some(end) = end {
        q = q.filter(Col::OccurredAt.lt(end));
    }
    Ok(q.select_only()
        .column_as(Col::Id.count(), "pv")
        .column_as(Expr::col(Col::VisitorKey).count_distinct(), "uv")
        .into_model::<Counts>()
        .one(db)
        .await?
        .unwrap_or(Counts { pv: 0, uv: 0 }))
}
#[derive(FromQueryResult, Serialize)]
struct Trend {
    date: String,
    pv: i64,
    uv: i64,
}
#[derive(FromQueryResult, Serialize)]
struct TopPage {
    path: String,
    pv: i64,
    uv: i64,
}
#[derive(Default, Deserialize)]
struct DashboardQuery {
    days: Option<i64>,
}
async fn dashboard(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Query(query): Query<DashboardQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let days = query.days.unwrap_or(14).clamp(1, 90);
    let today = (chrono::Utc::now() + Duration::hours(8)).date_naive();
    let midnight = today.and_hms_opt(0, 0, 0).unwrap() - Duration::hours(8);
    let since = midnight - Duration::days(days - 1);
    let total = counts(&state.db, None, None).await?;
    let now_counts = counts(&state.db, Some(midnight), None).await?;
    let yesterday = counts(
        &state.db,
        Some(midnight - Duration::days(1)),
        Some(midnight),
    )
    .await?;
    let date: sea_orm::sea_query::SimpleExpr = Func::cust(Alias::new("date"))
        .arg(Expr::col(Col::OccurredAt))
        .arg(Expr::val("+8 hours"))
        .into();
    let rows = Events::find()
        .select_only()
        .column_as(date.clone(), "date")
        .column_as(Col::Id.count(), "pv")
        .column_as(Expr::col(Col::VisitorKey).count_distinct(), "uv")
        .filter(Col::OccurredAt.gte(since))
        .group_by(date)
        .order_by_asc(Expr::col(Alias::new("date")))
        .into_model::<Trend>()
        .all(&state.db)
        .await?;
    let map: std::collections::HashMap<_, _> =
        rows.into_iter().map(|r| (r.date.clone(), r)).collect();
    let trend = (0..days)
        .map(|d| {
            let date = (today - Duration::days(days - 1 - d)).to_string();
            map.get(&date)
                .map(|r| serde_json::json!(r))
                .unwrap_or_else(|| serde_json::json!({"date":date,"pv":0,"uv":0}))
        })
        .collect::<Vec<_>>();
    let top = Events::find()
        .select_only()
        .column(Col::Path)
        .column_as(Col::Id.count(), "pv")
        .column_as(Expr::col(Col::VisitorKey).count_distinct(), "uv")
        .filter(Col::OccurredAt.gte(since))
        .group_by(Col::Path)
        .order_by_desc(Col::Id.count())
        .order_by_asc(Col::Path)
        .limit(10)
        .into_model::<TopPage>()
        .all(&state.db)
        .await?;
    let recent = Events::find()
        .select_only()
        .columns([
            Col::Id,
            Col::Path,
            Col::Title,
            Col::Referrer,
            Col::UserAgent,
            Col::IpAddress,
            Col::OccurredAt,
        ])
        .order_by_desc(Col::OccurredAt)
        .order_by_desc(Col::Id)
        .limit(8)
        .into_model::<VisitRow>()
        .all(&state.db)
        .await?;
    let content = serde_json::json!({"documents":document::Entity::find().count(&state.db).await?,"users":user::Entity::find().count(&state.db).await?,"comments":comment::Entity::find().filter(comment::Column::DeletedAt.is_null()).count(&state.db).await?,"images":media_image::Entity::find().count(&state.db).await?,"backgrounds":background::Entity::find().count(&state.db).await?});
    let first = Events::find()
        .order_by_asc(Col::OccurredAt)
        .limit(1)
        .select_only()
        .column(Col::OccurredAt)
        .into_tuple::<NaiveDateTime>()
        .one(&state.db)
        .await?;
    Ok(Json(
        serde_json::json!({"today":now_counts,"yesterday":yesterday,"total":total,"trend":trend,"top_pages":top,"recent":recent,"content":content,"first_event_at":first,"timezone":"Asia/Shanghai"}),
    ))
}
