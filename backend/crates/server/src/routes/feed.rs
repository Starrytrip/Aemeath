//! RSS 与站点地图实时读取已发布文章，禁止回退到前端文章快照。
use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use yukina_runtime::state::AppState;
use yukina_shared::error::ApiResult;
use yukina_store::entities::document::{Column, Entity as Documents};
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/feed.xml", get(feed))
        .route("/sitemap.xml", get(sitemap))
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .chars()
        .filter(|c| {
            matches!(*c, '\t' | '\n' | '\r') || (*c >= ' ' && *c != '\u{fffe}' && *c != '\u{ffff}')
        })
        .collect()
}
async fn site(state: &AppState) -> ApiResult<(String, String, String)> {
    let v = super::site::data(&state.db, "settings")
        .await?
        .unwrap_or_default();
    let b = &v["basic"];
    Ok((
        b["site_url"]
            .as_str()
            .unwrap_or("https://example.com")
            .trim_end_matches('/')
            .into(),
        b["title"].as_str().unwrap_or("Yukina").into(),
        b["description"].as_str().unwrap_or("").into(),
    ))
}
fn response(body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}
async fn feed(State(state): State<AppState>) -> ApiResult<Response> {
    let (base, title, description) = site(&state).await?;
    let rows = Documents::find()
        .filter(Column::Visibility.eq("public"))
        .filter(Column::Slug.is_not_null())
        .order_by_desc(Column::Date)
        .all(&state.db)
        .await?;
    let mut items = String::new();
    for d in rows {
        let link = format!(
            "{base}/reader/?slug={}",
            d.slug.as_deref().unwrap_or_default()
        );
        let date = chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|d| format!("<pubDate>{}</pubDate>", d.and_utc().to_rfc2822()))
            .unwrap_or_default();
        items.push_str(&format!("<item><title>{}</title><link>{}</link><guid isPermaLink=\"false\">yukina-article-{}</guid>{}<description>{}</description><content:encoded>{}</content:encoded></item>",xml(&d.title),xml(&link),d.id,date,xml(&d.description),xml(&super::content::markdown_html(&d.markdown).replace("src=\"/",&format!("src=\"{base}/")).replace("href=\"/",&format!("href=\"{base}/")))));
    }
    Ok(response(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\" xmlns:content=\"http://purl.org/rss/1.0/modules/content/\"><channel><title>{}</title><link>{}</link><description>{}</description>{}</channel></rss>",xml(&title),xml(&base),xml(&description),items)))
}
async fn sitemap(State(state): State<AppState>) -> ApiResult<Response> {
    let (base, _, _) = site(&state).await?;
    let rows = Documents::find()
        .filter(Column::Visibility.eq("public"))
        .filter(Column::Slug.is_not_null())
        .all(&state.db)
        .await?;
    let mut urls = format!("<url><loc>{}/</loc></url>", xml(&base));
    for path in [
        "archive/",
        "categories/",
        "tags/",
        "talks/",
        "friends/",
        "gallery/",
        "moments/",
        "rss/",
    ] {
        urls.push_str(&format!("<url><loc>{}/{path}</loc></url>", xml(&base)));
    }
    for slug in ["about", "guestbook", "about-open-source", "friends"] {
        if super::site::data(&state.db, &format!("page-{slug}"))
            .await?
            .is_some_and(|v| v["published"] == true)
        {
            let path = if matches!(slug, "about" | "guestbook") {
                format!("{slug}/")
            } else {
                format!("page/{slug}/")
            };
            urls.push_str(&format!("<url><loc>{}/{path}</loc></url>", xml(&base)));
        }
    }
    for d in rows {
        urls.push_str(&format!(
            "<url><loc>{}</loc></url>",
            xml(&format!(
                "{base}/reader/?slug={}",
                d.slug.as_deref().unwrap_or_default()
            ))
        ));
    }
    Ok(response(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">{urls}</urlset>")))
}
