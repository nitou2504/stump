use models::catalog_revision;
use models::entity::{
	access_audit, access_device, access_invite, access_password_reset,
	access_signin_mark, age_restriction, api_key, book_club, book_club_book, book_club_member, bookmark,
	custom_emoji, held_file, intake_item, job, kobo_sync_session, last_library_visit,
	library, library_config, library_exclusion, library_intake_settings,
	library_scan_record, library_tag, media, media_analysis, media_annotation,
	media_metadata, media_metadata_edit, media_purge, media_tag, metadata_fetch_record,
	post_processing_task, reading_device, reading_list, reading_list_item,
	reading_list_rule, reading_session, refresh_token, series, series_metadata,
	series_tag, server_config, session, smart_list, smart_list_access_rule,
	smart_list_view, tag, user, user_discover_rules, user_login_activity,
	user_preferences, user_sharing_settings,
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
		schema.create_table_from_entity(held_file::Entity),
		schema.create_table_from_entity(post_processing_task::Entity),
		schema.create_table_from_entity(media_analysis::Entity),
		schema.create_table_from_entity(series::Entity),
		schema.create_table_from_entity(series_metadata::Entity),
		schema.create_table_from_entity(library_exclusion::Entity),
		schema.create_table_from_entity(kobo_sync_session::Entity),
		schema.create_table_from_entity(last_library_visit::Entity),
		schema.create_table_from_entity(book_club::Entity),
		schema.create_table_from_entity(book_club_member::Entity),
		schema.create_table_from_entity(book_club_book::Entity),
		schema.create_table_from_entity(age_restriction::Entity),
		schema.create_table_from_entity(user::Entity),
		schema.create_table_from_entity(user_preferences::Entity),
		schema.create_table_from_entity(user_login_activity::Entity),
		schema.create_table_from_entity(library::Entity),
		schema.create_table_from_entity(library_config::Entity),
		schema.create_table_from_entity(intake_item::Entity),
		schema.create_table_from_entity(library_intake_settings::Entity),
		schema.create_table_from_entity(reading_session::Entity),
		schema.create_table_from_entity(reading_device::Entity),
		schema.create_table_from_entity(tag::Entity),
		schema.create_table_from_entity(media_tag::Entity),
		schema.create_table_from_entity(series_tag::Entity),
		schema.create_table_from_entity(library_tag::Entity),
		schema.create_table_from_entity(user_discover_rules::Entity),
		schema.create_table_from_entity(user_sharing_settings::Entity),
		schema.create_table_from_entity(access_invite::Entity),
		schema.create_table_from_entity(access_device::Entity),
		schema.create_table_from_entity(access_audit::Entity),
		schema.create_table_from_entity(access_signin_mark::Entity),
		schema.create_table_from_entity(access_password_reset::Entity),
		schema.create_table_from_entity(server_config::Entity),
		schema.create_table_from_entity(refresh_token::Entity),
		schema.create_table_from_entity(session::Entity),
		schema.create_table_from_entity(bookmark::Entity),
		schema.create_table_from_entity(media_annotation::Entity),
		schema.create_table_from_entity(reading_list::Entity),
		schema.create_table_from_entity(reading_list_item::Entity),
		schema.create_table_from_entity(reading_list_rule::Entity),
		schema.create_table_from_entity(smart_list::Entity),
		schema.create_table_from_entity(smart_list_access_rule::Entity),
		schema.create_table_from_entity(smart_list_view::Entity),
		schema.create_table_from_entity(custom_emoji::Entity),
		schema.create_table_from_entity(metadata_fetch_record::Entity),
		schema.create_table_from_entity(job::Entity),
		schema.create_table_from_entity(library_scan_record::Entity),
	];

	for stmt in tables {
		db.execute(&stmt).await?;
	}

	// what the smart lists' cache stands on: its tables, and the triggers that bump the catalog's
	// revision (the same statements as the migration's)
	for sql in catalog_revision::create_statements()
		.into_iter()
		.chain(catalog_revision::trigger_statements())
	{
		db.execute_unprepared(&sql).await?;
	}

	Ok(())
}
