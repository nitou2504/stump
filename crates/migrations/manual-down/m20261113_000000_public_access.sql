-- Public access (migration m20261113_000000_public_access), manual rollback.
--
-- Do this with Stump stopped, run it with the host's sqlite3 against the live database, then start the
-- previous image (the one before Public access). The migration's own down() does the same, and its test
-- runs THIS FILE on a migrated database: the two can't drift apart.
--
-- Every table of the migration is new, so nothing else is touched: the users, their sign-in history
-- (user_login_activity), the books and the other migrations stay as they are. What is lost is the
-- access data only: invites, the list of devices with a pass and their revocations, the audit log, the
-- sign-in marks (which pass a sign-in came with and why it failed) and the "choose your own password"
-- requirements.
--
-- Two things to do around it (they are in deploy/public-edge/DEPLOY.md too):
--   * back up config/Stump.toml with the database snapshot (the previous image rewrites it at start);
--   * after the rollback, passes already handed out keep working through the edge (their format did not
--     change), but no revocation made from the admin page is known any more: restore
--     edge/revoked.map with the ids you want dead, and reload the edge.
--
-- Usage:   sqlite3 /mnt/data2/containers/stump-fork/config/stump.db < crates/migrations/manual-down/m20261113_000000_public_access.sql   (a copy is in research/2026-11-13-public-access-manual-down.sql)

BEGIN;
DROP TABLE IF EXISTS access_password_resets;
DROP TABLE IF EXISTS access_signin_marks;
DROP TABLE IF EXISTS access_audit;
DROP TABLE IF EXISTS access_devices;
DROP TABLE IF EXISTS access_invites;
DELETE FROM seaql_migrations WHERE version = 'm20261113_000000_public_access';
COMMIT;

-- check: no access table is left, and the migration is not recorded
SELECT count(*) AS access_tables_left FROM sqlite_master WHERE type = 'table' AND name LIKE 'access\_%' ESCAPE '\';
SELECT count(*) AS migration_still_recorded FROM seaql_migrations WHERE version = 'm20261113_000000_public_access';
