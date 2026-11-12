-- Public access: dismissed warnings (migration m20261116_000000_access_warning_reviews), manual rollback.
--
-- Do this with Stump stopped, run it with the host's sqlite3 against the live database, then start the
-- previous image (stump:fork-43fa3fe50 or whatever ran before this migration). The migration's own
-- down() does the same, and its test runs THIS FILE on a migrated database that holds a row in the new
-- table: the two can't drift apart.
--
-- What is lost: only which warnings the owner had dismissed (Public access -> Needs your attention). They all
-- show again. Nothing else of Stump's data is touched.
--
-- Usage:   sqlite3 /mnt/data2/containers/stump-fork/config/stump.db < crates/migrations/manual-down/m20261116_000000_access_warning_reviews.sql   (a copy is in research/2026-11-16-access-warning-reviews-manual-down.sql)

BEGIN;
DROP TABLE IF EXISTS access_warning_reviews;
DELETE FROM seaql_migrations WHERE version = 'm20261116_000000_access_warning_reviews';
COMMIT;

-- check: the table is gone and the migration is not recorded
SELECT count(*) AS tables_left FROM sqlite_master WHERE type = 'table' AND name = 'access_warning_reviews';
SELECT count(*) AS migration_still_recorded FROM seaql_migrations WHERE version = 'm20261116_000000_access_warning_reviews';
