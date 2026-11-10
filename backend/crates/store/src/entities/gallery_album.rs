use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "gallery_albums")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub slug: String,
    pub name: String,
    pub description: String,
    pub date: String,
    pub location: String,
    pub cover: String,
    pub tags: String,
    pub photos: String,
    pub published: bool,
    pub sort_order: i64,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
