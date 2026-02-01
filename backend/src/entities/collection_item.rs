//! SeaORM Entity for collection_items table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "collection_item")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub collection_id: i32,
    pub music_id: i32,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::collection::Entity",
        from = "Column::CollectionId",
        to = "super::collection::Column::Id"
    )]
    Collection,
    #[sea_orm(
        belongs_to = "super::music::Entity",
        from = "Column::MusicId",
        to = "super::music::Column::Id"
    )]
    Music,
}

impl Related<super::collection::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Collection.def()
    }
}

impl Related<super::music::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Music.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
