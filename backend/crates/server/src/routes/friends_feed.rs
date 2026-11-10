//! 朋友圈从后台友链读取 RSS；后台刷新、公网地址校验与短期缓存。
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use yukina_runtime::state::AppState;
use yukina_shared::error::{ApiError, ApiResult};
#[derive(Default)]
struct Cache {
    key: String,
    items: Vec<Value>,
    loading: bool,
    updated: Option<Instant>,
    errors: usize,
}
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}
pub async fn feed(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let friends = super::site::data(&state.db, "friends")
        .await?
        .unwrap_or(json!([]));
    let key = friends.to_string();
    let mut c = cache().lock().await;
    if c.key != key {
        *c = Cache {
            key: key.clone(),
            ..Default::default()
        };
    }
    if !c.loading
        && c.updated
            .is_none_or(|t| t.elapsed() > Duration::from_secs(600))
    {
        c.loading = true;
        tokio::spawn(async move {
            let rows = friends.as_array().cloned().unwrap_or_default();
            let mut jobs = tokio::task::JoinSet::new();
            let mut items = vec![];
            let mut errors = 0;
            for row in rows
                .into_iter()
                .filter(|f| f["enabled"] != false && !f["rss"].as_str().unwrap_or("").is_empty())
                .take(100)
            {
                jobs.spawn(async move {
                    let url = reqwest::Url::parse(row["rss"].as_str().unwrap_or(""))
                        .map_err(|_| ApiError::BadRequest("RSS 地址无效".into()))?;
                    let bytes = yukina_shared::public_http::fetch_bytes(url).await?;
                    parse(&bytes, &row)
                });
                if jobs.len() >= 4 {
                    match jobs.join_next().await {
                        Some(Ok(Ok(rows))) => items.extend(rows),
                        _ => errors += 1,
                    }
                }
            }
            while let Some(r) = jobs.join_next().await {
                match r {
                    Ok(Ok(rows)) => items.extend(rows),
                    _ => errors += 1,
                }
            }
            items.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
            items.truncate(500);
            let mut c = cache().lock().await;
            if c.key == key {
                c.items = items;
                c.errors = errors;
                c.updated = Some(Instant::now());
                c.loading = false;
            }
        });
    }
    Ok(Json(
        json!({"items":c.items,"loading":c.loading,"failed_sources":c.errors}),
    ))
}
fn parse(bytes: &[u8], friend: &Value) -> ApiResult<Vec<Value>> {
    use quick_xml::{events::Event, Reader};
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut field = String::new();
    let mut item = serde_json::Map::new();
    let mut inside = false;
    let mut rows = vec![];
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if name == "item" || name == "entry" {
                    inside = true;
                    item.clear();
                } else if inside {
                    field = name;
                    if field == "link" {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"href" {
                                item.insert(
                                    "link".into(),
                                    Value::String(
                                        a.unescape_value()
                                            .map_err(|_| {
                                                ApiError::BadRequest("RSS 链接无效".into())
                                            })?
                                            .into_owned(),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) if inside && e.local_name().as_ref() == b"link" => {
                for a in e.attributes().flatten() {
                    if a.key.as_ref() == b"href" {
                        item.insert(
                            "link".into(),
                            Value::String(a.unescape_value().unwrap_or_default().into_owned()),
                        );
                    }
                }
            }
            Ok(Event::Text(e)) if inside => {
                let t = e
                    .unescape()
                    .map_err(|_| ApiError::BadRequest("RSS 文本无效".into()))?;
                let existing = item.entry(field.clone()).or_insert(json!(""));
                *existing = json!(format!("{}{}", existing.as_str().unwrap_or(""), t));
            }
            Ok(Event::CData(e)) if inside => {
                let t = e
                    .decode()
                    .map_err(|_| ApiError::BadRequest("RSS 编码无效".into()))?;
                item.insert(field.clone(), json!(t));
            }
            Ok(Event::End(e))
                if inside
                    && (e.local_name().as_ref() == b"item"
                        || e.local_name().as_ref() == b"entry") =>
            {
                let read = |keys: &[&str]| {
                    keys.iter()
                        .find_map(|k| item.get(*k).and_then(Value::as_str))
                        .unwrap_or("")
                };
                let link = read(&["link"]);
                if link.starts_with("https://") || link.starts_with("http://") {
                    let raw = read(&["published", "pubDate", "updated"]);
                    let date = chrono::DateTime::parse_from_rfc3339(raw)
                        .or_else(|_| chrono::DateTime::parse_from_rfc2822(raw))
                        .map(|d| d.to_utc().to_rfc3339())
                        .unwrap_or_default();
                    rows.push(json!({"title":read(&["title"]),"url":link,"date":date,"friend_title":friend["title"],"avatar":friend["imgurl"]}));
                }
                inside = false;
                if rows.len() >= 20 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(ApiError::BadRequest("RSS 格式无效".into())),
            _ => {}
        }
    }
    Ok(rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rss_and_atom_are_normalized() {
        let f = json!({"title":"Friend","imgurl":"/a.png"});
        let rss=br#"<rss><channel><item><title>A &amp; B</title><link>https://example.org/post</link><pubDate>Tue, 10 Nov 2026 00:00:00 +0000</pubDate></item></channel></rss>"#;
        assert_eq!(parse(rss, &f).unwrap()[0]["title"], "A & B");
        let atom=br#"<feed><entry><title>Atom</title><link href="https://example.org/atom"/><published>2026-11-10T00:00:00Z</published></entry></feed>"#;
        assert_eq!(
            parse(atom, &f).unwrap()[0]["url"],
            "https://example.org/atom"
        );
    }
}
