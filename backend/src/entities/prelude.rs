//! SeaORM Entity Prelude
pub use sea_orm::entity::prelude::*;

pub use super::music::Model as Music;
pub use super::music::Entity as MusicEntity;
pub use super::music::ActiveModel as MusicActiveModel;
pub use super::music::Column as MusicColumn;

pub use super::collection::Model as Collection;
pub use super::collection::Entity as CollectionEntity;
pub use super::collection::ActiveModel as CollectionActiveModel;
pub use super::collection::Column as CollectionColumn;

pub use super::collection_item::Model as CollectionItem;
pub use super::collection_item::Entity as CollectionItemEntity;
pub use super::collection_item::ActiveModel as CollectionItemActiveModel;
pub use super::collection_item::Column as CollectionItemColumn;
