use models::entity::{
	intake_item, library_intake_settings,
	age_restriction, api_key, bookmark, kobo_sync_session, library, library_config,
	library_exclusion, media, media_analysis, media_annotation, media_metadata,
	media_metadata_edit, media_purge, job, log,
	media_tag, reading_device, reading_session, refresh_token, series, series_metadata,
	server_config, session, tag, user, user_preferences,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, DbConn, DbErr, Schema};
pub async fn test_database() -> DbConn {
	let db = Database::connect("sqlite::memory:")
		.await
		.expect("failed to connect to test database");

	create_database_tables(&db)
		.await
		.expect("failed to create test database tables");

	db
}

pub async fn create_database_tables(db: &DbConn) -> Result<(), DbErr> {
	let schema = Schema::new(DbBackend::Sqlite);

	let tables = [
		schema.create_table_from_entity(api_key::Entity),
		schema.create_table_from_entity(media::Entity),
		schema.create_table_from_entity(media_metadata::Entity),
		schema.create_table_from_entity(media_metadata_edit::Entity),
		schema.create_table_from_entity(media_purge::Entity),
		schema.create_table_from_entity(job::Entity),
		schema.create_table_from_entity(log::Entity),
		schema.create_table_from_entity(media_analysis::Entity),
		schema.create_table_from_entity(series::Entity),
		schema.create_table_from_entity(series_metadata::Entity),
		schema.create_table_from_entity(library_exclusion::Entity),
		schema.create_table_from_entity(kobo_sync_session::Entity),
		schema.create_table_from_entity(age_restriction::Entity),
		schema.create_table_from_entity(user::Entity),
		schema.create_table_from_entity(user_preferences::Entity),
		schema.create_table_from_entity(library::Entity),
		schema.create_table_from_entity(library_config::Entity),
		schema.create_table_from_entity(intake_item::Entity),
		schema.create_table_from_entity(library_intake_settings::Entity),
		schema.create_table_from_entity(reading_session::Entity),
		schema.create_table_from_entity(reading_device::Entity),
		schema.create_table_from_entity(tag::Entity),
		schema.create_table_from_entity(media_tag::Entity),
		schema.create_table_from_entity(server_config::Entity),
		schema.create_table_from_entity(refresh_token::Entity),
		schema.create_table_from_entity(session::Entity),
		schema.create_table_from_entity(bookmark::Entity),
		schema.create_table_from_entity(media_annotation::Entity),
	];

	for stmt in tables {
		db.execute(&stmt).await?;
	}

	Ok(())
}
