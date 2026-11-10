use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "backgrounds")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub url: String,
    pub poster_url: String,
    pub media_type: String,
    pub theme: String,
    pub device: String,
    pub sort_order: i64,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
