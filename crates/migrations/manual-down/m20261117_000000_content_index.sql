-- Content index (migration m20261117_000000_content_index), manual rollback.
--
-- Do this with Stump stopped, run it with the host's sqlite3 against the live database, then start the
-- previous image. The migration's own down() does the same, and its test runs THIS FILE on a migrated
-- database: the two can't drift apart.
--
-- What is lost: only the text index of the books (`/api/v2/content`); `POST /api/v2/content/reindex`
-- builds it again. Nothing else of Stump's data is touched.
--
-- Usage:   sqlite3 /mnt/data2/containers/stump-fork/config/stump.db < crates/migrations/manual-down/m20261117_000000_content_index.sql

BEGIN;
DROP TRIGGER IF EXISTS content_sections_fts_delete;
DROP TABLE IF EXISTS content_fts;
DROP TABLE IF EXISTS content_sections;
DROP TABLE IF EXISTS content_books;
DROP TABLE IF EXISTS content_index_runs;
DELETE FROM seaql_migrations WHERE version = 'm20261117_000000_content_index';
COMMIT;

-- check: the tables are gone and the migration is not recorded
SELECT count(*) AS tables_left FROM sqlite_master WHERE name LIKE 'content\_%' ESCAPE '\';
SELECT count(*) AS migration_still_recorded FROM seaql_migrations WHERE version = 'm20261117_000000_content_index';
