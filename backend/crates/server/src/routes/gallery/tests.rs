use super::*;
use sea_orm::{ConnectOptions, Database};

fn input(slug: &str, published: bool) -> AlbumInput {
    AlbumInput {
        slug: slug.into(),
        name: "旅行相册".into(),
        description: "旅途记录".into(),
        date: "2026-11-05".into(),
        location: "杭州".into(),
        cover: String::new(),
        tags: vec!["风景".into()],
        photos: vec![Photo {
            id: "one".into(),
            url: "/uploads/photo.jpg".into(),
            caption: "山间".into(),
        }],
        published,
        sort_order: 0,
    }
}
#[test]
fn invalid_urls_ids_and_dates_are_rejected() {
    let mut album = input("travel", true);
    assert!(album.validate().is_ok());
    for url in [
        "javascript:alert(1)",
        "//evil.example/image.jpg",
        "http://example.com/a.jpg",
        "/bad\\path.jpg",
    ] {
        album.photos[0].url = url.into();
        assert!(album.validate().is_err(), "{url}");
    }
    album = input("travel", true);
    album.photos.push(album.photos[0].clone());
    assert!(album.validate().is_err());
    album = input("travel", true);
    album.date = "2026-99-99".into();
    assert!(album.validate().is_err());
    album = input("../bad", true);
    assert!(album.validate().is_err());
    album = input("travel", true);
    album.name = "  ".into();
    assert!(album.validate().is_err());
}
#[tokio::test]
async fn albums_persist_filter_drafts_order_update_and_delete() {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    yukina_store::migrations::run(&db).await.unwrap();
    yukina_store::migrations::run(&db).await.unwrap();
    let public = input("public", true)
        .model()
        .unwrap()
        .insert(&db)
        .await
        .unwrap();
    let draft = input("draft", false)
        .model()
        .unwrap()
        .insert(&db)
        .await
        .unwrap();
    assert_eq!(list(&db, true).await.unwrap().len(), 1);
    assert_eq!(list(&db, false).await.unwrap().len(), 2);
    assert!(ensure_unique(&db, "public", None).await.is_err());
    assert!(ensure_unique(&db, "public", Some(public.id)).await.is_ok());
    let mut updated = input("draft", true);
    updated.sort_order = 4;
    updated.photos[0].caption = "更新说明".into();
    let mut model = updated.model().unwrap();
    model.id = Set(draft.id);
    model.update(&db).await.unwrap();
    let rows = list(&db, true).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, public.id);
    assert_eq!(rows[1].data.photos[0].caption, "更新说明");
    Albums::delete_by_id(public.id).exec(&db).await.unwrap();
    assert_eq!(list(&db, true).await.unwrap().len(), 1);
}
