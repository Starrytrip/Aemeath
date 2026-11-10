//! 音乐配置持久化、Meting 主备解析与短期缓存。认证信息仅留在后端。
use axum::{
    extract::{DefaultBodyLimit, State},
    routing::{get, post},
    Json, Router,
};
use sea_orm::{sea_query::OnConflict, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::{
    error::{ApiError, ApiResult},
    serial::utc_now,
};
use yukina_store::entities::music_setting::{self, Column, Entity as Settings};

mod uploads;
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/music", get(get_config).put(save_config))
        .route("/admin/music/preview", post(preview))
        .route("/music/playlist", get(playlist))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .merge(uploads::router())
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Song {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub url: String,
    #[serde(default)]
    pub cover: String,
    #[serde(default)]
    pub lrc: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Meting {
    pub api: String,
    pub server: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub fallback_apis: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Configuration {
    pub mode: String,
    pub volume: f64,
    pub play_mode: String,
    pub show_lyrics: bool,
    pub songs: Vec<Song>,
    pub meting: Meting,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            mode: "local".into(),
            volume: 0.7,
            play_mode: "list".into(),
            show_lyrics: true,
            songs: vec![],
            meting: Meting {
                api: "https://api.i-meto.com/meting/api?server=:server&type=:type&id=:id&r=:r"
                    .into(),
                server: "netease".into(),
                kind: "playlist".into(),
                id: String::new(),
                fallback_apis: vec![
                    "https://api.injahow.cn/meting/?server=:server&type=:type&id=:id".into(),
                    "https://api.moeyao.cn/meting/?server=:server&type=:type&id=:id".into(),
                ],
            },
        }
    }
}
fn media_url(value: &str) -> bool {
    value.is_empty() || yukina_shared::validation::valid_url(value)
}
impl Configuration {
    fn validate(&self) -> ApiResult<()> {
        if !matches!(self.mode.as_str(), "local" | "meting")
            || !self.volume.is_finite()
            || !(0.0..=1.0).contains(&self.volume)
            || !matches!(self.play_mode.as_str(), "list" | "one" | "random")
            || self.songs.len() > 500
        {
            return Err(ApiError::Unprocessable(
                "音乐模式、音量、播放方式或歌单数量不正确（最多500首）".into(),
            ));
        }
        let mut ids = std::collections::HashSet::new();
        for song in &self.songs {
            if song.id.is_empty()
                || song.id.len() > 100
                || !ids.insert(&song.id)
                || song.name.trim().is_empty()
                || song.name.chars().count() > 200
                || song.artist.chars().count() > 200
                || song.url.is_empty()
                || !media_url(&song.url)
                || !media_url(&song.cover)
                || song.lrc.len() > 512 * 1024
                || !(song.lrc.is_empty()
                    || song.lrc.trim_start().starts_with('[')
                    || media_url(&song.lrc))
            {
                return Err(ApiError::Unprocessable(
                    "歌曲名称、音频地址、封面或歌词格式不正确".into(),
                ));
            }
        }
        if !matches!(
            self.meting.server.as_str(),
            "netease" | "tencent" | "kugou" | "xiami" | "baidu"
        ) || !matches!(
            self.meting.kind.as_str(),
            "song" | "playlist" | "album" | "artist" | "search"
        ) || self.meting.id.len() > 300
            || self.meting.fallback_apis.len() > 4
        {
            return Err(ApiError::Unprocessable(
                "Meting 平台、类型、ID或备用接口配置不正确".into(),
            ));
        }
        if self.mode == "meting" && self.meting.id.trim().is_empty() {
            return Err(ApiError::Unprocessable(
                "请填写歌单、歌曲ID或搜索关键词".into(),
            ));
        }
        for template in std::iter::once(&self.meting.api).chain(self.meting.fallback_apis.iter()) {
            validate_template(template)?;
        }
        Ok(())
    }
}
fn validate_template(template: &str) -> ApiResult<()> {
    let url = reqwest::Url::parse(template)
        .map_err(|_| ApiError::Unprocessable("Meting 接口地址不正确".into()))?;
    if template.len() > 2000
        || url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::Unprocessable(
            "Meting 接口须为不带账号、密码或片段的HTTPS地址".into(),
        ));
    }
    Ok(())
}
#[derive(Deserialize)]
struct SaveInput {
    configuration: Configuration,
    revision: i64,
    auth: Option<String>,
    #[serde(default)]
    clear_auth: bool,
}
async fn row(state: &AppState) -> ApiResult<Option<music_setting::Model>> {
    Ok(Settings::find_by_id(1).one(&state.db).await?)
}
fn decode(row: &music_setting::Model) -> ApiResult<Configuration> {
    serde_json::from_str(&row.configuration)
        .map_err(|_| ApiError::Internal("音乐配置读取失败".into()))
}
async fn get_config(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> ApiResult<Json<serde_json::Value>> {
    let row = row(&state).await?;
    let config = if let Some(row) = &row {
        decode(row)?
    } else {
        Configuration::default()
    };
    Ok(Json(
        serde_json::json!({"configured":row.is_some(),"configuration":config,"revision":row.as_ref().map(|r|r.revision).unwrap_or(0),"has_auth":row.is_some_and(|r|!r.meting_auth.is_empty())}),
    ))
}
async fn save_config(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(input): Json<SaveInput>,
) -> ApiResult<Json<serde_json::Value>> {
    input.configuration.validate()?;
    if input.revision < 0
        || input.revision == i64::MAX
        || input
            .auth
            .as_ref()
            .is_some_and(|s| s.len() > 4096 || s.chars().any(char::is_control))
    {
        return Err(ApiError::Unprocessable("配置版本或认证信息不正确".into()));
    }
    let old = row(&state).await?;
    let auth = if input.clear_auth {
        String::new()
    } else {
        input.auth.filter(|s| !s.is_empty()).unwrap_or_else(|| {
            old.as_ref()
                .map(|r| r.meting_auth.clone())
                .unwrap_or_default()
        })
    };
    let json = serde_json::to_string(&input.configuration)
        .map_err(|_| ApiError::BadRequest("音乐配置格式错误".into()))?;
    let affected = if old.is_some() {
        Settings::update_many()
            .col_expr(Column::Configuration, sea_orm::sea_query::Expr::value(json))
            .col_expr(Column::MetingAuth, sea_orm::sea_query::Expr::value(auth))
            .col_expr(
                Column::Revision,
                sea_orm::sea_query::Expr::value(input.revision + 1),
            )
            .col_expr(
                Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(utc_now()),
            )
            .filter(Column::Id.eq(1))
            .filter(Column::Revision.eq(input.revision))
            .exec(&state.db)
            .await?
            .rows_affected
    } else if input.revision == 0 {
        Settings::insert(music_setting::ActiveModel {
            id: Set(1),
            configuration: Set(json),
            meting_auth: Set(auth),
            revision: Set(1),
            updated_at: Set(utc_now()),
        })
        .on_conflict(OnConflict::column(Column::Id).do_nothing().to_owned())
        .exec_without_returning(&state.db)
        .await?
    } else {
        0
    };
    if affected == 0 {
        return Err(ApiError::Conflict(
            "音乐配置已被其他窗口修改，请重新加载后保存".into(),
        ));
    }
    Ok(Json(
        serde_json::json!({"saved":true,"revision":input.revision+1}),
    ))
}
#[derive(Clone, Serialize)]
pub struct Track {
    pub name: String,
    pub artist: String,
    pub url: String,
    pub pic: String,
    pub lrc: String,
}
#[derive(Clone, Serialize)]
pub struct Resolved {
    pub mode: String,
    pub volume: f64,
    pub play_mode: String,
    pub show_lyrics: bool,
    pub playlist: Vec<Track>,
    pub warning: Option<String>,
}
struct Cache {
    key: String,
    until: Instant,
    value: Resolved,
}
static CACHE: OnceLock<Mutex<Option<Cache>>> = OnceLock::new();
async fn resolve(state: &AppState, force: bool) -> ApiResult<Resolved> {
    let row = row(state)
        .await?
        .ok_or_else(|| ApiError::NotFound("尚未保存后台音乐配置".into()))?;
    let config = decode(&row)?;
    let key = hex::encode(Sha256::digest(format!(
        "{}:{}",
        row.configuration, row.meting_auth
    )));
    let mut cache = tokio::time::timeout(
        Duration::from_secs(28),
        CACHE.get_or_init(|| Mutex::new(None)).lock(),
    )
    .await
    .map_err(|_| ApiError::too_many(5))?;
    if !force {
        if let Some(cached) = cache
            .as_ref()
            .filter(|c| c.key == key && c.until > Instant::now())
        {
            return Ok(cached.value.clone());
        }
    }
    let mut result = Resolved {
        mode: config.mode.clone(),
        volume: config.volume,
        play_mode: config.play_mode.clone(),
        show_lyrics: config.show_lyrics,
        playlist: vec![],
        warning: None,
    };
    if config.mode == "local" {
        result.playlist = config
            .songs
            .into_iter()
            .map(|s| Track {
                name: s.name,
                artist: s.artist,
                url: s.url,
                pic: s.cover,
                lrc: s.lrc,
            })
            .collect();
    } else {
        match tokio::time::timeout(
            Duration::from_secs(22),
            fetch_meting(&config.meting, &row.meting_auth),
        )
        .await
        .unwrap_or_else(|_| Err(ApiError::BadRequest("音乐接口超时".into())))
        {
            Ok(songs) => result.playlist = songs,
            Err(_) => {
                result.warning =
                    Some("Meting 主备接口均未能返回可播放歌曲，请在后台检查配置或稍后重试".into())
            }
        }
    }
    let ttl = if result.warning.is_some() { 15 } else { 300 };
    *cache = Some(Cache {
        key,
        until: Instant::now() + Duration::from_secs(ttl),
        value: result.clone(),
    });
    Ok(result)
}
async fn playlist(State(state): State<AppState>) -> ApiResult<Json<Resolved>> {
    Ok(Json(resolve(&state, false).await?))
}
async fn preview(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> ApiResult<Json<Resolved>> {
    Ok(Json(resolve(&state, true).await?))
}
fn encode(value: &str) -> String {
    let mut url = reqwest::Url::parse("https://encoding.invalid").unwrap();
    url.query_pairs_mut().append_pair("v", value);
    url.query().unwrap().trim_start_matches("v=").to_string()
}
async fn fetch_meting(config: &Meting, auth: &str) -> ApiResult<Vec<Track>> {
    for api in std::iter::once(&config.api).chain(config.fallback_apis.iter()) {
        let address = api
            .replace(":server", &encode(&config.server))
            .replace(":type", &encode(&config.kind))
            .replace(":id", &encode(&config.id))
            .replace(":r", &rand::random::<u64>().to_string());
        let Ok(mut url) = reqwest::Url::parse(&address) else {
            continue;
        };
        if !auth.is_empty() {
            url.query_pairs_mut().append_pair("auth", auth);
        }
        if let Ok(songs) = fetch_one(url).await {
            return Ok(songs);
        }
    }
    Err(ApiError::BadRequest("音乐解析失败".into()))
}
async fn fetch_one(url: reqwest::Url) -> ApiResult<Vec<Track>> {
    parse_tracks(&yukina_shared::public_http::fetch_bytes(url).await?)
}
fn parse_tracks(bytes: &[u8]) -> ApiResult<Vec<Track>> {
    let rows = serde_json::from_slice::<Vec<serde_json::Value>>(bytes)
        .map_err(|_| ApiError::BadRequest("音乐接口格式不正确".into()))?;
    let tracks = rows
        .into_iter()
        .take(500)
        .filter_map(|row| {
            let read = |keys: &[&str]| {
                keys.iter()
                    .find_map(|k| row.get(*k).and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string()
            };
            let url = read(&["url"]);
            if url.is_empty() || !media_url(&url) {
                return None;
            }
            let pic = read(&["pic", "cover"]);
            let lrc = read(&["lrc"]);
            Some(Track {
                name: read(&["title", "name"]).chars().take(200).collect(),
                artist: read(&["author", "artist"]).chars().take(200).collect(),
                url,
                pic: if media_url(&pic) { pic } else { String::new() },
                lrc: if lrc.len() <= 512 * 1024
                    && (lrc.is_empty() || lrc.trim_start().starts_with('[') || media_url(&lrc))
                {
                    lrc
                } else {
                    String::new()
                },
            })
        })
        .collect::<Vec<_>>();
    if tracks.is_empty() {
        return Err(ApiError::BadRequest("音乐接口返回空歌单".into()));
    }
    Ok(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_modes_tracks_and_public_upstreams() {
        let mut config = Configuration::default();
        assert!(config.validate().is_ok());
        config.mode = "meting".into();
        assert!(config.validate().is_err());
        config.meting.id = "123".into();
        assert!(config.validate().is_ok());
        config.meting.api = "http://localhost/api".into();
        assert!(config.validate().is_err());
        assert_eq!(encode("id&auth=secret"), "id%26auth%3Dsecret");
    }
    #[test]
    fn meting_normalizes_aliases_and_drops_invalid_media() {
        let tracks = parse_tracks(br#"[{"title":"A","author":"B","url":"https://example.com/a.mp3","pic":"javascript:alert(1)","lrc":"[00:00.00]hello"},{"name":"C","artist":"D","url":"/api/v1/music/media/test.mp3"},{"url":"javascript:alert(1)"}]"#).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].name, "A");
        assert_eq!(tracks[0].artist, "B");
        assert!(tracks[0].pic.is_empty());
        assert!(tracks[0].lrc.starts_with('['));
        assert_eq!(tracks[1].name, "C");
        assert!(parse_tracks(b"[]").is_err());
        assert!(parse_tracks(b"{\"error\":true}").is_err());
    }
}
