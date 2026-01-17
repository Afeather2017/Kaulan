use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Music::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Music::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Music::Filename).string().not_null())
                    .col(ColumnDef::new(Music::FilePath).string().not_null())
                    .col(ColumnDef::new(Music::Lufs).double())
                    .col(
                        ColumnDef::new(Music::CreatedAt)
                            .timestamp()
                            .extra("DEFAULT CURRENT_TIMESTAMP"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Music::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Music {
    Table,
    Id,
    Filename,
    FilePath,
    Lufs,
    CreatedAt,
}