use std::collections::HashMap;

use async_graphql::Result;
use models::{
	entity::user::AuthUser,
	shared::alphabet::{AvailableAlphabet, EntityLetter},
};
use sea_orm::{
	sea_query::Expr, DatabaseConnection, EntityTrait, QueryOrder, QuerySelect, Select,
	Statement, Value,
};
use tower_sessions::Session;

pub async fn save_user_session(session: &Session, user: AuthUser) {
	if let Err(error) = session.insert("user", user).await {
		tracing::error!(?error, "Failed to save user session");
	}
}

/// Build a raw SQL [`Statement`] using the actual database backend of `conn`.
/// Write SQL with `$1`, `$2`, … placeholders — they work for both PostgreSQL
/// and SQLite (SQLite treats them as named parameters).
pub fn db_statement(
	conn: &DatabaseConnection,
	sql: impl Into<String>,
	values: impl IntoIterator<Item = Value>,
) -> Statement {
	Statement::from_sql_and_values(conn.get_database_backend(), sql, values)
}

/// How many of the entities `select` finds start with each letter (an "alphabet", see
/// [`AvailableAlphabet`]). `title` is the SQL for the text whose first letter counts, e.g.
/// `COALESCE(media_metadata.title, media.name)`, and `id` the SQL of the entity's id, so one that
/// joins to several rows counts once. Start from a `find_for_user` select, so only what the user
/// may see is counted: an alphabet that counted everything gave away the first letters of
/// the books and libraries hidden from them.
pub async fn letter_counts<E: EntityTrait>(
	conn: &DatabaseConnection,
	select: Select<E>,
	title: &str,
	id: &str,
) -> Result<HashMap<String, i64>> {
	let letters = select
		.select_only()
		.expr_as(Expr::cust(format!("substr({title}, 1, 1)")), "letter")
		.expr_as(Expr::cust(format!("COUNT(DISTINCT {id})")), "count")
		.group_by(Expr::cust("letter"))
		.order_by_asc(Expr::cust("letter"))
		.into_model::<EntityLetter>()
		.all(conn)
		.await?;

	Ok(AvailableAlphabet::from(letters).get())
}
