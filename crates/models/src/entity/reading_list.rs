use async_graphql::SimpleObject;
use sea_orm::{
	prelude::*,
	sea_query::{Query, SelectStatement},
	Condition, FromQueryResult, QueryOrder,
};

use super::{
	reading_list_item, reading_list_rule,
	user::{self, AuthUser},
};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[graphql(name = "ReadingListModel")]
#[sea_orm(table_name = "reading_lists")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub id: String,
	#[sea_orm(column_type = "Text")]
	pub name: String,
	#[sea_orm(column_type = "Text", nullable)]
	pub description: Option<String>,
	#[sea_orm(column_type = "custom(\"DATETIME\")")]
	pub updated_at: DateTimeWithTimeZone,
	#[sea_orm(column_type = "Text")]
	pub visibility: String,
	#[sea_orm(column_type = "Text")]
	pub ordering: String,
	#[sea_orm(column_type = "Text")]
	pub creating_user_id: String,
	#[sea_orm(has_many, relation_enum = "ReadingListItems")]
	#[graphql(skip)]
	pub reading_list_items: HasMany<reading_list_item::Entity>,
	#[sea_orm(has_many, relation_enum = "ReadingListRules")]
	#[graphql(skip)]
	pub reading_list_rules: HasMany<reading_list_rule::Entity>,
	#[sea_orm(
		belongs_to,
		from = "creating_user_id",
		to = "id",
		on_update = "Cascade",
		on_delete = "Cascade"
	)]
	#[graphql(skip)]
	pub user: BelongsTo<user::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

/// A list only its creator sees
pub const PRIVATE: &str = "PRIVATE";
/// A list every account on the server sees (each one with the books they may read)
pub const PUBLIC: &str = "PUBLIC";

/// The people whose reading lists can be shown: not deleted and not locked. A list stays in the
/// database when its creator goes, and is back when they are unlocked
fn active_users() -> SelectStatement {
	Query::select()
		.column(user::Column::Id)
		.from(user::Entity)
		.and_where(user::Column::DeletedAt.is_null())
		.and_where(user::Column::IsLocked.eq(false))
		.to_owned()
}

/// A user sees their own lists and the public lists of everyone else, whoever made them being
/// still around. There's no per-person sharing: `SHARED` lists (and `reading_list_rules`) are
/// not offered, so a list with that visibility is seen by nobody but its creator
fn visible_to(user_id: &str) -> Condition {
	Condition::all()
		.add(
			Condition::any()
				.add(Column::CreatingUserId.eq(user_id.to_owned()))
				.add(Column::Visibility.eq(PUBLIC)),
		)
		.add(Column::CreatingUserId.in_subquery(active_users()))
}

#[derive(Debug, FromQueryResult)]
pub struct ReadingListIdCmpSelect {
	pub id: String,
}

impl Entity {
	/// The lists the user sees: their own and everyone's public ones
	pub fn find_for_user(user: &AuthUser) -> Select<Entity> {
		Entity::find()
			.filter(visible_to(&user.id))
			.order_by_asc(Column::Id)
	}

	/// One list, if the user sees it. A list that doesn't exist and one that isn't theirs to see
	/// are the same: nothing
	pub fn find_for_user_and_id(user: &AuthUser, id: &str) -> Select<Entity> {
		Entity::find()
			.filter(Column::Id.eq(id))
			.filter(visible_to(&user.id))
	}

	/// The lists the user made
	pub fn find_mine(user: &AuthUser) -> Select<Entity> {
		Entity::find()
			.filter(Column::CreatingUserId.eq(user.id.clone()))
			.filter(Column::CreatingUserId.in_subquery(active_users()))
			.order_by_asc(Column::Id)
	}

	/// The public lists other people made
	pub fn find_by_others(user: &AuthUser) -> Select<Entity> {
		Entity::find()
			.filter(Column::CreatingUserId.ne(user.id.clone()))
			.filter(Column::Visibility.eq(PUBLIC))
			.filter(Column::CreatingUserId.in_subquery(active_users()))
			.order_by_asc(Column::Id)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::common::*;
	use pretty_assertions::assert_eq;

	const ACTIVE: &str = r#""reading_lists"."creating_user_id" IN (SELECT "id" FROM "users" WHERE "users"."deleted_at" IS NULL AND "users"."is_locked" = FALSE)"#;

	#[test]
	fn test_visible_to() {
		let condition = visible_to("42");
		assert_eq!(
			condition_to_string(&condition),
			r#"SELECT  WHERE "#.to_string()
				+ r#"("reading_lists"."creating_user_id" = '42' OR "reading_lists"."visibility" = 'PUBLIC') AND "#
				+ ACTIVE
		);
	}

	#[test]
	fn test_find_for_user() {
		let user = get_default_user();
		let stmt = Entity::find_for_user(&user);
		assert_eq!(
			select_no_cols_to_string(stmt),
			r#"SELECT  FROM "reading_lists" WHERE "#.to_string()
				+ r#"("reading_lists"."creating_user_id" = '42' OR "reading_lists"."visibility" = 'PUBLIC') AND "#
				+ ACTIVE + r#" ORDER BY "reading_lists"."id" ASC"#
		);
	}

	#[test]
	fn test_find_for_user_and_id() {
		let user = get_default_user();
		let stmt = Entity::find_for_user_and_id(&user, "314");
		assert_eq!(
			select_no_cols_to_string(stmt),
			r#"SELECT  FROM "reading_lists" WHERE "#.to_string()
				+ r#""reading_lists"."id" = '314' AND "#
				+ r#"("reading_lists"."creating_user_id" = '42' OR "reading_lists"."visibility" = 'PUBLIC') AND "#
				+ ACTIVE
		);
	}

	#[test]
	fn test_find_mine() {
		let user = get_default_user();
		let stmt = Entity::find_mine(&user);
		assert_eq!(
			select_no_cols_to_string(stmt),
			r#"SELECT  FROM "reading_lists" WHERE "#.to_string()
				+ r#""reading_lists"."creating_user_id" = '42' AND "#
				+ ACTIVE + r#" ORDER BY "reading_lists"."id" ASC"#
		);
	}

	#[test]
	fn test_find_by_others() {
		let user = get_default_user();
		let stmt = Entity::find_by_others(&user);
		assert_eq!(
			select_no_cols_to_string(stmt),
			r#"SELECT  FROM "reading_lists" WHERE "#.to_string()
				+ r#""reading_lists"."creating_user_id" <> '42' AND "#
				+ r#""reading_lists"."visibility" = 'PUBLIC' AND "#
				+ ACTIVE + r#" ORDER BY "reading_lists"."id" ASC"#
		);
	}
}
