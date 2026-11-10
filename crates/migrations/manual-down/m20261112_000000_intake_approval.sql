-- MANUAL DOWN of m20261112_000000_intake_approval (uploads that wait for an approver).
--
-- Why it exists: an older image refuses to start on a database whose seaql_migrations holds a version it
-- doesn't know ("Migration file of version '...' is missing"), so going back to the image before this one
-- takes this SQL (or a restore of the snapshot taken before the deploy, which loses what was read or
-- filed since). The migration's own down() does the same statements, and the test
-- `the_manual_down_leaves_a_database_the_previous_image_can_use` in the migration runs this file against a
-- migrated database.
--
-- Run with Stump STOPPED, on a database that has its pre-deploy snapshot, with the host's sqlite3 (3.35 or
-- later, for DROP COLUMN), then start the previous image:
--   sqlite3 /path/to/config/stump.db < crates/migrations/manual-down/m20261112_000000_intake_approval.sql
--
-- What it does: puts the new states back to ones an older version reads (waiting and half-filed items are
-- READY again, a sent-back one needs review, items of the isolated checker, if it was ever on, failed), drops
-- the five columns added to intake_items and the table of users' own limits, and forgets the migration.
-- Nothing else is touched: no book, user or list.
--
-- AFTER rolling back, take ADD_BOOKS away from friends again: the older image lets every holder of it file
-- books straight into a library.
--
-- If a later version ever puts a foreign key or an index on one of these columns, the DROP COLUMN lines fail:
-- then rebuild intake_items (SQLite's create, copy, drop, rename procedure) instead.
BEGIN;
UPDATE intake_items SET state = CASE state
	WHEN 'PENDING_APPROVAL' THEN 'READY'
	WHEN 'FILING' THEN 'READY'
	WHEN 'REJECTED' THEN 'NEEDS_REVIEW'
	WHEN 'SCREENING' THEN 'FAILED'
	WHEN 'BLOCKED' THEN 'FAILED'
	ELSE state END;
DROP TABLE IF EXISTS user_intake_limits;
ALTER TABLE intake_items DROP COLUMN submitted_at;
ALTER TABLE intake_items DROP COLUMN reviewed_at;
ALTER TABLE intake_items DROP COLUMN reviewed_by;
ALTER TABLE intake_items DROP COLUMN review_note;
ALTER TABLE intake_items DROP COLUMN cover_file;
DELETE FROM seaql_migrations WHERE version = 'm20261112_000000_intake_approval';
COMMIT;
