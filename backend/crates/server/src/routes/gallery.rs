use axum::{
    extract::{DefaultBodyLimit, Path, State},
    routing::get,
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use yukina_runtime::{auth::deps::AdminUser, state::AppState};
use yukina_shared::error::{ApiError, ApiResult};
use yukina_store::entities::gallery_album::{self, Column, Entity as Albums};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/gallery/albums", get(public_list))
        .route("/admin/gallery/albums", get(admin_list).post(create))
        .route(
            "/admin/gallery/albums/{id}",
            axum::routing::put(update).delete(remove),
        )
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Photo {
    pub id: String,
    pub url: String,
    pub caption: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumInput {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub date: String,
    pub location: String,
    pub cover: String,
    pub tags: Vec<String>,
    pub photos: Vec<Photo>,
    pub published: bool,
    pub sort_order: i64,
}
#[derive(Serialize)]
pub struct Album {
    id: i64,
    #[serde(flatten)]
    data: AlbumInput,
}
impl AlbumInput {
    fn validate(&self) -> ApiResult<()> {
        let bad = || {
            ApiError::Unprocessable("相册名称、标识、日期或图片格式不正确（最多500张照片）".into())
        };
        if self.slug.is_empty()
            || self.slug.len() > 120
            || !self
                .slug
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || self.name.trim().is_empty()
            || self.name.chars().count() > 200
            || self.description.chars().count() > 5000
            || self.location.chars().count() > 200
            || (!self.date.is_empty()
                && chrono::NaiveDate::parse_from_str(&self.date, "%Y-%m-%d").is_err())
            || !yukina_shared::validation::valid_url(&self.cover)
            || self.tags.len() > 30
            || self
                .tags
                .iter()
                .any(|t| t.trim().is_empty() || t.chars().count() > 50)
            || self.photos.len() > 500
            || !(0..=1000000).contains(&self.sort_order)
        {
            return Err(bad());
        }
        let mut ids = std::collections::HashSet::new();
        for photo in &self.photos {
            if photo.id.is_empty()
                || photo.id.len() > 100
                || !ids.insert(&photo.id)
                || photo.url.is_empty()
                || !yukina_shared::validation::valid_url(&photo.url)
                || photo.caption.chars().count() > 1000
            {
                return Err(bad());
            }
        }
        Ok(())
    }
    fn model(self) -> ApiResult<gallery_album::ActiveModel> {
        Ok(gallery_album::ActiveModel {
            slug: Set(self.slug),
            name: Set(self.name),
            description: Set(self.description),
            date: Set(self.date),
            location: Set(self.location),
            cover: Set(self.cover),
            tags: Set(serde_json::to_string(&self.tags)
                .map_err(|_| ApiError::Internal("标签序列化失败".into()))?),
            photos: Set(serde_json::to_string(&self.photos)
                .map_err(|_| ApiError::Internal("照片序列化失败".into()))?),
            published: Set(self.published),
            sort_order: Set(self.sort_order),
            ..Default::default()
        })
    }
}
fn output(row: gallery_album::Model) -> ApiResult<Album> {
    Ok(Album {
        id: row.id,
        data: AlbumInput {
            slug: row.slug,
            name: row.name,
            description: row.description,
            date: row.date,
            location: row.location,
            cover: row.cover,
            tags: serde_json::from_str(&row.tags)
                .map_err(|_| ApiError::Internal("相册标签数据损坏".into()))?,
            photos: serde_json::from_str(&row.photos)
                .map_err(|_| ApiError::Internal("相册照片数据损坏".into()))?,
            published: row.published,
            sort_order: row.sort_order,
        },
    })
}
async fn list(db: &DatabaseConnection, public: bool) -> ApiResult<Vec<Album>> {
    let mut query = Albums::find();
    if public {
        query = query.filter(Column::Published.eq(true));
    }
    query
        .order_by_asc(Column::SortOrder)
        .order_by_desc(Column::Id)
        .all(db)
        .await?
        .into_iter()
        .map(output)
        .collect()
}
async fn public_list(State(state): State<AppState>) -> ApiResult<Json<Vec<Album>>> {
    Ok(Json(list(&state.db, true).await?))
}
async fn admin_list(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> ApiResult<Json<Vec<Album>>> {
    Ok(Json(list(&state.db, false).await?))
}
async fn ensure_unique(db: &DatabaseConnection, slug: &str, id: Option<i64>) -> ApiResult<()> {
    if Albums::find()
        .filter(Column::Slug.eq(slug))
        .one(db)
        .await?
        .is_some_and(|row| Some(row.id) != id)
    {
        return Err(ApiError::BadRequest("相册标识已存在，请换一个".into()));
    }
    Ok(())
}
async fn create(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(input): Json<AlbumInput>,
) -> ApiResult<Json<Album>> {
    input.validate()?;
    ensure_unique(&state.db, &input.slug, None).await?;
    Ok(Json(output(input.model()?.insert(&state.db).await?)?))
}
async fn update(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    Json(input): Json<AlbumInput>,
) -> ApiResult<Json<Album>> {
    input.validate()?;
    if Albums::find_by_id(id).one(&state.db).await?.is_none() {
        return Err(ApiError::NotFound("相册不存在".into()));
    }
    ensure_unique(&state.db, &input.slug, Some(id)).await?;
    let mut model = input.model()?;
    model.id = Set(id);
    Ok(Json(output(model.update(&state.db).await?)?))
}
async fn remove(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    if Albums::delete_by_id(id)
        .exec(&state.db)
        .await?
        .rows_affected
        == 0
    {
        return Err(ApiError::NotFound("相册不存在".into()));
    }
    Ok(Json(serde_json::json!({"deleted": true})))
}

#[cfg(test)]
mod tests;
