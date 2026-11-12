-- Friend onboarding (migration m20261115_000000_friend_onboarding), manual rollback.
--
-- Do this with Stump stopped, run it with the host's sqlite3 against the live database, then start the
-- previous image (stump:fork-93eda9ca8 or whatever ran before this migration). The migration's own
-- down() does the same, and its test runs THIS FILE on a migrated database that holds rows in the new
-- tables: the two can't drift apart.
--
-- What is lost: the signup links, each person's public sign-in switch / device limit / "hide new
-- libraries" choice, the record of which libraries were decided for whom, and the reading-app keys
-- (they are meaningless without the column).
-- What stays, on purpose: the accounts the links made, with their permissions and their library
-- exclusions (so they stay "Biblioteca only"), and every browser device (pass). Their passes keep
-- working through the edge; the older image does not know the friend rules, so before you start it
-- check Settings -> Users for who may be on the internet, or switch Funnel off while you work.
--
-- Two things to do around it (they are in deploy/public-edge/DEPLOY.md too):
--   * restore the edge files from the backup made before the deploy (the new nginx.conf serves a sign-in
--     page and reading-app addresses an older image can't answer);
--   * a snapshot restore forgets revocations made since: re-check Settings -> Public access -> Devices.
--
-- Usage:   sqlite3 /mnt/data2/containers/stump-fork/config/stump.db < crates/migrations/manual-down/m20261115_000000_friend_onboarding.sql   (a copy is in research/2026-11-15-friend-onboarding-manual-down.sql)
--
-- DROP COLUMN needs SQLite 3.35 or newer (the host's is 3.45, the one Stump bundles 3.46).
-- If you ever add an index on access_devices.kind, drop it first: DROP COLUMN refuses an indexed column.

BEGIN;
DELETE FROM access_devices WHERE kind = 'opds';
ALTER TABLE access_devices DROP COLUMN kind;
DROP TABLE IF EXISTS access_library_decisions;
DROP TABLE IF EXISTS access_people;
DROP TABLE IF EXISTS access_signup_links;
DELETE FROM seaql_migrations WHERE version = 'm20261115_000000_friend_onboarding';
COMMIT;

-- check: the three tables are gone, the column is gone, and the migration is not recorded
SELECT count(*) AS tables_left FROM sqlite_master WHERE type = 'table' AND name IN ('access_signup_links', 'access_people', 'access_library_decisions');
SELECT count(*) AS kind_column_left FROM pragma_table_info('access_devices') WHERE name = 'kind';
SELECT count(*) AS migration_still_recorded FROM seaql_migrations WHERE version = 'm20261115_000000_friend_onboarding';
