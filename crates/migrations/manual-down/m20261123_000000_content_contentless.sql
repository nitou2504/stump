-- Content index without text (migration m20261123_000000_content_contentless), manual rollback.
--
-- Rolling back by RESTORING the snapshot taken before the deploy (`config.backup-<ts>-db`) gives back
-- the first index exactly, with its text, and is what to do when the previous image must serve
-- searches at once. This file is the other way: it keeps the database as it is and makes the tables
-- the previous image expects, EMPTY: an index without content can't give the books' text back, so every
-- book is forgotten and the previous image's `content_index` job reads them all again (about 2.5 hours
-- at idle priority; `/api/v2/content/search` finds nothing until it has).
--
-- Do it with Stump stopped, with the host's sqlite3, then start the previous image. The migration's own
-- down() does the same, and its test runs THIS FILE on a migrated database.
--
-- Usage:   sqlite3 /mnt/data2/containers/stump-fork/config/stump.db < crates/migrations/manual-down/m20261123_000000_content_contentless.sql

BEGIN;
DROP TRIGGER IF EXISTS content_sections_idx_delete;
DROP TABLE IF EXISTS content_idx;
CREATE VIRTUAL TABLE IF NOT EXISTS content_fts USING fts5(
	title, text, tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER IF NOT EXISTS content_sections_fts_delete
	AFTER DELETE ON content_sections
	BEGIN
		DELETE FROM content_fts WHERE rowid = old.id;
	END;
DELETE FROM content_sections;
DELETE FROM content_books;
DELETE FROM seaql_migrations WHERE version = 'm20261123_000000_content_contentless';
COMMIT;

-- check: the first index's tables are there, empty, and the migration is not recorded
SELECT count(*) AS content_fts_present FROM sqlite_master WHERE name = 'content_fts';
SELECT count(*) AS content_idx_present FROM sqlite_master WHERE name = 'content_idx';
SELECT count(*) AS migration_still_recorded FROM seaql_migrations WHERE version = 'm20261123_000000_content_contentless';
