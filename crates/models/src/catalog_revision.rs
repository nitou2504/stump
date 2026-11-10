//! What the smart lists' cache stands on (package SL-3).
//!
//! A smart list's rules are worked out over the whole catalog once, and the books they match are
//! kept in `smart_list_members`. Every read then narrows that set to what the reader may see, so
//! the cache holds what the list *is* and never what somebody gets of it. To know when a list has
//! to be worked out again, the catalog has a revision (`catalog_revision`, one row), bumped by
//! triggers whenever something changes that a rule can look at. A list's
//! `smart_list_materializations` row says which revision its members were worked out at.
//!
//! The triggers (SQLite only: on any other database the cache is off and every list is worked
//! out when it is read) are all named `catalog_rev_*`, so a manual rollback can find them:
//!
//! ```sql
//! SELECT 'DROP TRIGGER ' || name || ';' FROM sqlite_master
//! WHERE type = 'trigger' AND name LIKE 'catalog_rev_%';
//! ```
//!
//! The migration and the test database both create this schema from the functions below, and a
//! test in `graphql` lists every column the rule builder can filter on and fails if one of them
//! isn't in [`WATCHED`] (or is one of the [`LIVE`] columns that a cached list can't use).

/// The members of every smart list that is kept. A book is a member of a list when the list's
/// rules match it, whoever asks: soft-deleted books are in it too, and every read leaves them out
/// together with what the reader may not see.
pub const MEMBERS_TABLE: &str = "smart_list_members";
/// One row for each list that is kept: the revision of the catalog its members were worked out
/// at, how many there are, and when
pub const MATERIALIZATIONS_TABLE: &str = "smart_list_materializations";
/// One row (`id = 1`): the revision of the catalog
pub const REVISION_TABLE: &str = "catalog_revision";
/// Every trigger of the cache starts with this
pub const TRIGGER_PREFIX: &str = "catalog_rev_";

/// A table the rule builder can read, and the columns an update of which can change what a rule
/// matches. An insert or a delete of a row of the table always counts.
#[derive(Debug, Clone, Copy)]
pub struct Watched {
	pub table: &'static str,
	pub columns: &'static [&'static str],
}

/// Everything a smart list's rules can look at (see `crates/graphql/src/filter`). What is left
/// out on purpose is what changes all the time without changing what a rule matches: thumbnails,
/// hashes, `modified_at` and `updated_at`, scan bookkeeping, and `deleted_at` (a removed book is
/// still a member: the readers' own filter leaves it out, and so restoring it needs no new work).
pub const WATCHED: &[Watched] = &[
	Watched {
		table: "media",
		columns: &[
			"id",
			"name",
			"size",
			"extension",
			"created_at",
			"series_id",
			"library_id",
			"status",
			"path",
			"pages",
		],
	},
	Watched {
		table: "media_metadata",
		columns: &[
			"media_id",
			"title",
			"publisher",
			"genres",
			"language",
			"characters",
			"colorists",
			"writers",
			"pencillers",
			"letterers",
			"cover_artists",
			"inkers",
			"editors",
			"age_rating",
			"rating",
			"year",
			"month",
			"day",
			"links",
			"teams",
			"summary",
			"series",
		],
	},
	Watched {
		table: "series",
		columns: &["id", "name", "path", "library_id", "is_oneshot"],
	},
	Watched {
		table: "series_metadata",
		columns: &[
			"series_id",
			"age_rating",
			"meta_type",
			"title",
			"summary",
			"publisher",
			"imprint",
			"comicid",
			"booktype",
			"volume",
			"status",
			"year",
		],
	},
	Watched {
		table: "libraries",
		columns: &["id", "name", "path"],
	},
	Watched {
		table: "library_configs",
		columns: &["library_id", "library_type"],
	},
	Watched {
		table: "tags",
		columns: &["id", "name"],
	},
	Watched {
		table: "media_tags",
		columns: &["id", "media_id", "tag_id"],
	},
];

/// The columns a rule can filter on that are **not** watched, because their value is different
/// for every reader or changes without any trigger being worth it. A list whose rules use one of
/// them is never cached: it is worked out whenever it is read.
pub const LIVE: &[(&str, &str)] = &[
	// a reader's own progress
	("reading_sessions", "*"),
	// bumped by thumbnail generation, post-processing and every metadata write
	("media", "updated_at"),
];

/// The statements that create the three tables (SQLite)
pub fn create_statements() -> Vec<String> {
	vec![
		format!(
			"CREATE TABLE IF NOT EXISTS {MEMBERS_TABLE} (
				smart_list_id TEXT NOT NULL REFERENCES smart_lists(id) ON DELETE CASCADE ON UPDATE CASCADE,
				media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE ON UPDATE CASCADE,
				PRIMARY KEY (smart_list_id, media_id)
			) WITHOUT ROWID"
		),
		// a deleted book takes its memberships with it: the foreign key finds them here
		format!(
			"CREATE INDEX IF NOT EXISTS idx_{MEMBERS_TABLE}_media_id ON {MEMBERS_TABLE} (media_id)"
		),
		format!(
			"CREATE TABLE IF NOT EXISTS {MATERIALIZATIONS_TABLE} (
				smart_list_id TEXT NOT NULL PRIMARY KEY REFERENCES smart_lists(id) ON DELETE CASCADE ON UPDATE CASCADE,
				revision INTEGER NOT NULL,
				member_count INTEGER NOT NULL,
				computed_at TEXT NOT NULL
			)"
		),
		format!(
			"CREATE TABLE IF NOT EXISTS {REVISION_TABLE} (
				id INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),
				revision INTEGER NOT NULL
			)"
		),
		format!("INSERT OR IGNORE INTO {REVISION_TABLE} (id, revision) VALUES (1, 0)"),
	]
}

/// The name of the trigger that bumps the revision after `event` on `table`
fn trigger_name(table: &str, event: &str) -> String {
	format!("{TRIGGER_PREFIX}{table}_{event}")
}

/// The name of the trigger that forgets a list's members when its rules change
pub const RULES_TRIGGER: &str = "catalog_rev_smart_list_rules";

/// The names of every trigger [`trigger_statements`] creates
pub fn trigger_names() -> Vec<String> {
	let mut names: Vec<String> = WATCHED
		.iter()
		.flat_map(|watched| {
			["insert", "delete", "update"]
				.into_iter()
				.map(|event| trigger_name(watched.table, event))
		})
		.collect();
	names.push(RULES_TRIGGER.to_string());
	names
}

/// The statements that create the triggers (SQLite)
pub fn trigger_statements() -> Vec<String> {
	let bump =
		format!("UPDATE {REVISION_TABLE} SET revision = revision + 1 WHERE id = 1");
	let mut statements = Vec::new();
	for watched in WATCHED {
		let table = watched.table;
		for (event, name) in [("INSERT", "insert"), ("DELETE", "delete")] {
			statements.push(format!(
				"CREATE TRIGGER {} AFTER {event} ON {table} BEGIN {bump}; END",
				trigger_name(table, name)
			));
		}
		statements.push(format!(
			"CREATE TRIGGER {} AFTER UPDATE OF {} ON {table} BEGIN {bump}; END",
			trigger_name(table, "update"),
			watched.columns.join(", ")
		));
	}
	// new rules make the list's members wrong at once, whatever the revision says: they are
	// dropped, and the next read works the list out
	statements.push(format!(
		"CREATE TRIGGER {RULES_TRIGGER} AFTER UPDATE OF filters, joiner ON smart_lists BEGIN
			DELETE FROM {MEMBERS_TABLE} WHERE smart_list_id = NEW.id;
			DELETE FROM {MATERIALIZATIONS_TABLE} WHERE smart_list_id = NEW.id;
		END"
	));
	statements
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashSet;

	#[test]
	fn every_trigger_has_the_prefix_and_a_name_of_its_own() {
		let names = trigger_names();
		assert!(names.iter().all(|name| name.starts_with(TRIGGER_PREFIX)));
		let distinct: HashSet<&String> = names.iter().collect();
		assert_eq!(distinct.len(), names.len(), "{names:?}");
		// three for each watched table, and the one that forgets a list's members
		assert_eq!(names.len(), WATCHED.len() * 3 + 1);
	}

	#[test]
	fn the_statements_create_exactly_the_named_triggers() {
		let statements = trigger_statements();
		let names = trigger_names();
		assert_eq!(statements.len(), names.len());
		for name in &names {
			assert!(
				statements
					.iter()
					.any(|sql| sql.starts_with(&format!("CREATE TRIGGER {name} "))),
				"no statement creates {name}"
			);
		}
	}

	#[test]
	fn a_table_is_watched_once_with_columns_that_are_named_once() {
		let tables: HashSet<&str> = WATCHED.iter().map(|w| w.table).collect();
		assert_eq!(tables.len(), WATCHED.len());
		for watched in WATCHED {
			assert!(!watched.columns.is_empty(), "{}", watched.table);
			let columns: HashSet<&&str> = watched.columns.iter().collect();
			assert_eq!(columns.len(), watched.columns.len(), "{}", watched.table);
		}
	}

	#[test]
	fn what_a_cached_list_can_not_use_is_not_watched() {
		for (table, column) in LIVE {
			let watched = WATCHED.iter().find(|w| w.table == *table);
			assert!(
				watched.map_or(true, |w| !w.columns.contains(column)),
				"{table}.{column} is both live and watched"
			);
		}
	}
}
