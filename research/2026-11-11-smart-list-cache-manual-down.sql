-- SL-3: smart list cache (migration m20261111_000000_smart_list_members), manual rollback.
--
-- Plan Appendix B style: do this with Stump stopped, run it with the host's sqlite3, then start the
-- previous image (the one before the smart list cache). The migration's own down() does the same;
-- its test runs this SQL on a migrated database too. Nothing outside the cache is touched: the smart
-- lists, the books and their metadata stay as they are, and the cache is rebuilt by the next image
-- that has it (an empty cache is simply worked out again).
--
--   1. list the triggers (they are all named catalog_rev_*, so this finds every one of them,
--      whatever version created them):
SELECT 'DROP TRIGGER ' || name || ';' FROM sqlite_master
WHERE type = 'trigger' AND name LIKE 'catalog_rev_%';
--
--   2. paste the DROP TRIGGER lines it printed in place of the marker below, then run the block:
BEGIN;
/* paste the DROP TRIGGER lines here */
DROP TABLE IF EXISTS smart_list_members;
DROP TABLE IF EXISTS smart_list_materializations;
DROP TABLE IF EXISTS catalog_revision;
DELETE FROM seaql_migrations WHERE version = 'm20261111_000000_smart_list_members';
COMMIT;
--
--   3. check: no cache trigger or table is left, and the migration is not recorded
SELECT count(*) AS cache_triggers_left FROM sqlite_master WHERE type = 'trigger' AND name LIKE 'catalog_rev_%';
SELECT count(*) AS cache_tables_left FROM sqlite_master
WHERE type = 'table' AND name IN ('smart_list_members', 'smart_list_materializations', 'catalog_revision');
SELECT count(*) AS migration_still_recorded FROM seaql_migrations WHERE version = 'm20261111_000000_smart_list_members';
