//! SeaORM Entity for collections table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "collection")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub name: String,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::collection_item::Entity")]
    CollectionItems,
}

impl Related<super::collection_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CollectionItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
