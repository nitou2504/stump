use sea_orm_migration::prelude::*;

/// Adds a community/reader rating (0-5) to media metadata, read from e.g. EPUB
/// `calibre:rating` or ComicInfo `CommunityRating`
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(MediaMetadata::Table)
					.add_column(ColumnDef::new(MediaMetadata::Rating).float())
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(MediaMetadata::Table)
					.drop_column(MediaMetadata::Rating)
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum MediaMetadata {
	Table,
	Rating,
}
