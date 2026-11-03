use sea_orm_migration::prelude::*;

/// The history of metadata edits made by hand: one row per changed field, grouped by the
/// save that changed it, so a save can be reverted
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(MediaMetadataEdits::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(MediaMetadataEdits::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(MediaMetadataEdits::MediaId)
							.text()
							.not_null(),
					)
					.col(ColumnDef::new(MediaMetadataEdits::SaveId).text().not_null())
					.col(ColumnDef::new(MediaMetadataEdits::Field).text().not_null())
					.col(ColumnDef::new(MediaMetadataEdits::OldValue).text().null())
					.col(ColumnDef::new(MediaMetadataEdits::NewValue).text().null())
					.col(ColumnDef::new(MediaMetadataEdits::EditedBy).text().null())
					.col(
						ColumnDef::new(MediaMetadataEdits::EditedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(MediaMetadataEdits::WrittenToFile)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(MediaMetadataEdits::RevertsSaveId)
							.text()
							.null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-media_metadata_edits-media")
							.from(MediaMetadataEdits::Table, MediaMetadataEdits::MediaId)
							.to(Media::Table, Media::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-media_metadata_edits-user")
							.from(MediaMetadataEdits::Table, MediaMetadataEdits::EditedBy)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::SetNull)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("media_metadata_edits_media_idx")
					.table(MediaMetadataEdits::Table)
					.col(MediaMetadataEdits::MediaId)
					.col(MediaMetadataEdits::EditedAt)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("media_metadata_edits_save_idx")
					.table(MediaMetadataEdits::Table)
					.col(MediaMetadataEdits::SaveId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(MediaMetadataEdits::Table).to_owned())
			.await?;
		Ok(())
	}
}

#[derive(DeriveIden)]
enum MediaMetadataEdits {
	Table,
	Id,
	MediaId,
	SaveId,
	Field,
	OldValue,
	NewValue,
	EditedBy,
	EditedAt,
	WrittenToFile,
	RevertsSaveId,
}

#[derive(DeriveIden)]
enum Media {
	Table,
	Id,
}

#[derive(DeriveIden)]
enum Users {
	Table,
	Id,
}
