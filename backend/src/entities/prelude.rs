//! SeaORM Entity Prelude
pub use sea_orm::entity::prelude::*;

pub use super::music::ActiveModel as MusicActiveModel;
pub use super::music::Column as MusicColumn;
pub use super::music::Entity as MusicEntity;
pub use super::music::Model as Music;

pub use super::db_meta::ActiveModel as DbMetaActiveModel;
pub use super::db_meta::Column as DbMetaColumn;
pub use super::db_meta::Entity as DbMetaEntity;
pub use super::db_meta::Model as DbMeta;
