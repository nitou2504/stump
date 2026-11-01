use sea_orm_migration::prelude::*;

/// Post-processing: a JSON settings column on `library_configs` (16 greys for manga, PDF OCR
/// and shrinking; `NULL` = everything off) and a `post_processing_tasks` table with one row per
/// book and kind, showing what happened to it
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(LibraryConfigs::Table)
					.add_column(
						ColumnDef::new(LibraryConfigs::PostProcessing).json().null(),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(PostProcessingTasks::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(PostProcessingTasks::Id)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(PostProcessingTasks::LibraryId)
							.text()
							.not_null(),
					)
					.col(ColumnDef::new(PostProcessingTasks::MediaId).text().null())
					.col(ColumnDef::new(PostProcessingTasks::Kind).text().not_null())
					.col(ColumnDef::new(PostProcessingTasks::Steps).text().null())
					.col(
						ColumnDef::new(PostProcessingTasks::Status)
							.text()
							.not_null(),
					)
					.col(ColumnDef::new(PostProcessingTasks::Path).text().not_null())
					.col(ColumnDef::new(PostProcessingTasks::Reason).text().null())
					.col(
						ColumnDef::new(PostProcessingTasks::BytesBefore)
							.big_integer()
							.null(),
					)
					.col(
						ColumnDef::new(PostProcessingTasks::BytesAfter)
							.big_integer()
							.null(),
					)
					.col(ColumnDef::new(PostProcessingTasks::Details).json().null())
					.col(
						ColumnDef::new(PostProcessingTasks::CreatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(PostProcessingTasks::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(PostProcessingTasks::FinishedAt)
							.timestamp_with_time_zone()
							.null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-post_processing_tasks-library")
							.from(
								PostProcessingTasks::Table,
								PostProcessingTasks::LibraryId,
							)
							.to(Libraries::Table, Libraries::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-post_processing_tasks-media")
							.from(
								PostProcessingTasks::Table,
								PostProcessingTasks::MediaId,
							)
							.to(Media::Table, Media::Id)
							.on_delete(ForeignKeyAction::SetNull)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_post_processing_tasks_media_kind")
					.table(PostProcessingTasks::Table)
					.col(PostProcessingTasks::MediaId)
					.col(PostProcessingTasks::Kind)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_post_processing_tasks_library_status")
					.table(PostProcessingTasks::Table)
					.col(PostProcessingTasks::LibraryId)
					.col(PostProcessingTasks::Status)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(PostProcessingTasks::Table).to_owned())
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(LibraryConfigs::Table)
					.drop_column(LibraryConfigs::PostProcessing)
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum LibraryConfigs {
	Table,
	PostProcessing,
}

#[derive(DeriveIden)]
enum PostProcessingTasks {
	Table,
	Id,
	LibraryId,
	MediaId,
	Kind,
	Steps,
	Status,
	Path,
	Reason,
	BytesBefore,
	BytesAfter,
	Details,
	CreatedAt,
	UpdatedAt,
	FinishedAt,
}

#[derive(DeriveIden)]
enum Libraries {
	Table,
	Id,
}

#[derive(DeriveIden)]
enum Media {
	Table,
	Id,
}
