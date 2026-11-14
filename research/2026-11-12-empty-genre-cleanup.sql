-- Stump: take the blank items out of the comma-separated list columns (genres, writers, ...).
--
-- Why: an EPUB with an empty first <dc:subject/> was stored as ', History, ...' and the book page showed an
-- empty chip for the blank first item (71 books, all EPUBs, in the live database on 2026-11-12; one more has
-- ', Charlotte Brontë' as its writers). The fixed Stump reads and writes lists without blank items; this repairs
-- the rows that were stored before it.
--
-- What it does: only a row that has a blank item (a leading, trailing or doubled comma, or only blanks) is
-- touched, and only in the column that has it. The items and their order are kept, the separators become ', ',
-- and a list of only blanks becomes NULL (no list), as the fixed Stump stores it. Nothing else changes.
-- The update fires the catalog revision triggers (smart lists refresh), nothing else.
--
-- How to run (never on a database a Stump of the OLD build keeps scanning: a rescan of those EPUBs would bring
-- the blank back; deploy the fixed build first):
--   1. snapshot:  B=config.backup-$(date -u +%Y%m%dT%H%M%SZ)-db; mkdir -p $B && sqlite3 config/stump.db ".backup $B/stump.db"
--   2. run:       sqlite3 config/stump.db < research/2026-11-12-empty-genre-cleanup.sql
--      It prints the number of malformed lists per column before, then after: every 'after' line is 0.
-- It is idempotent: running it again changes nothing. Back out: restore the snapshot (nothing else was written).
-- It is ONE transaction: if the database stays busy for 5 seconds (Stump writing), or anything fails, it stops
-- before changing a thing (.bail), instead of running each UPDATE on its own.

.bail on
.timeout 5000
.headers on
.mode column
SELECT 'before' AS stage, * FROM (
  SELECT 'media_metadata.genres' AS list, COUNT(*) AS malformed FROM media_metadata WHERE genres IS NOT NULL AND (LTRIM(genres, ' ') LIKE ',%' OR RTRIM(genres, ' ') LIKE '%,' OR REPLACE(genres, ' ', '') LIKE '%,,%' OR TRIM(genres) = '')
  UNION ALL SELECT 'media_metadata.writers' AS list, COUNT(*) AS malformed FROM media_metadata WHERE writers IS NOT NULL AND (LTRIM(writers, ' ') LIKE ',%' OR RTRIM(writers, ' ') LIKE '%,' OR REPLACE(writers, ' ', '') LIKE '%,,%' OR TRIM(writers) = '')
  UNION ALL SELECT 'media_metadata.pencillers' AS list, COUNT(*) AS malformed FROM media_metadata WHERE pencillers IS NOT NULL AND (LTRIM(pencillers, ' ') LIKE ',%' OR RTRIM(pencillers, ' ') LIKE '%,' OR REPLACE(pencillers, ' ', '') LIKE '%,,%' OR TRIM(pencillers) = '')
  UNION ALL SELECT 'media_metadata.inkers' AS list, COUNT(*) AS malformed FROM media_metadata WHERE inkers IS NOT NULL AND (LTRIM(inkers, ' ') LIKE ',%' OR RTRIM(inkers, ' ') LIKE '%,' OR REPLACE(inkers, ' ', '') LIKE '%,,%' OR TRIM(inkers) = '')
  UNION ALL SELECT 'media_metadata.colorists' AS list, COUNT(*) AS malformed FROM media_metadata WHERE colorists IS NOT NULL AND (LTRIM(colorists, ' ') LIKE ',%' OR RTRIM(colorists, ' ') LIKE '%,' OR REPLACE(colorists, ' ', '') LIKE '%,,%' OR TRIM(colorists) = '')
  UNION ALL SELECT 'media_metadata.letterers' AS list, COUNT(*) AS malformed FROM media_metadata WHERE letterers IS NOT NULL AND (LTRIM(letterers, ' ') LIKE ',%' OR RTRIM(letterers, ' ') LIKE '%,' OR REPLACE(letterers, ' ', '') LIKE '%,,%' OR TRIM(letterers) = '')
  UNION ALL SELECT 'media_metadata.cover_artists' AS list, COUNT(*) AS malformed FROM media_metadata WHERE cover_artists IS NOT NULL AND (LTRIM(cover_artists, ' ') LIKE ',%' OR RTRIM(cover_artists, ' ') LIKE '%,' OR REPLACE(cover_artists, ' ', '') LIKE '%,,%' OR TRIM(cover_artists) = '')
  UNION ALL SELECT 'media_metadata.editors' AS list, COUNT(*) AS malformed FROM media_metadata WHERE editors IS NOT NULL AND (LTRIM(editors, ' ') LIKE ',%' OR RTRIM(editors, ' ') LIKE '%,' OR REPLACE(editors, ' ', '') LIKE '%,,%' OR TRIM(editors) = '')
  UNION ALL SELECT 'media_metadata.links' AS list, COUNT(*) AS malformed FROM media_metadata WHERE links IS NOT NULL AND (LTRIM(links, ' ') LIKE ',%' OR RTRIM(links, ' ') LIKE '%,' OR REPLACE(links, ' ', '') LIKE '%,,%' OR TRIM(links) = '')
  UNION ALL SELECT 'media_metadata.characters' AS list, COUNT(*) AS malformed FROM media_metadata WHERE characters IS NOT NULL AND (LTRIM(characters, ' ') LIKE ',%' OR RTRIM(characters, ' ') LIKE '%,' OR REPLACE(characters, ' ', '') LIKE '%,,%' OR TRIM(characters) = '')
  UNION ALL SELECT 'media_metadata.teams' AS list, COUNT(*) AS malformed FROM media_metadata WHERE teams IS NOT NULL AND (LTRIM(teams, ' ') LIKE ',%' OR RTRIM(teams, ' ') LIKE '%,' OR REPLACE(teams, ' ', '') LIKE '%,,%' OR TRIM(teams) = '')
  UNION ALL SELECT 'series_metadata.characters' AS list, COUNT(*) AS malformed FROM series_metadata WHERE characters IS NOT NULL AND (LTRIM(characters, ' ') LIKE ',%' OR RTRIM(characters, ' ') LIKE '%,' OR REPLACE(characters, ' ', '') LIKE '%,,%' OR TRIM(characters) = '')
  UNION ALL SELECT 'series_metadata.genres' AS list, COUNT(*) AS malformed FROM series_metadata WHERE genres IS NOT NULL AND (LTRIM(genres, ' ') LIKE ',%' OR RTRIM(genres, ' ') LIKE '%,' OR REPLACE(genres, ' ', '') LIKE '%,,%' OR TRIM(genres) = '')
  UNION ALL SELECT 'series_metadata.links' AS list, COUNT(*) AS malformed FROM series_metadata WHERE links IS NOT NULL AND (LTRIM(links, ' ') LIKE ',%' OR RTRIM(links, ' ') LIKE '%,' OR REPLACE(links, ' ', '') LIKE '%,,%' OR TRIM(links) = '')
  UNION ALL SELECT 'series_metadata.writers' AS list, COUNT(*) AS malformed FROM series_metadata WHERE writers IS NOT NULL AND (LTRIM(writers, ' ') LIKE ',%' OR RTRIM(writers, ' ') LIKE '%,' OR REPLACE(writers, ' ', '') LIKE '%,,%' OR TRIM(writers) = '')
);

BEGIN IMMEDIATE;

UPDATE media_metadata SET genres = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(genres, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE genres IS NOT NULL AND (LTRIM(genres, ' ') LIKE ',%' OR RTRIM(genres, ' ') LIKE '%,' OR REPLACE(genres, ' ', '') LIKE '%,,%' OR TRIM(genres) = '');

UPDATE media_metadata SET writers = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(writers, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE writers IS NOT NULL AND (LTRIM(writers, ' ') LIKE ',%' OR RTRIM(writers, ' ') LIKE '%,' OR REPLACE(writers, ' ', '') LIKE '%,,%' OR TRIM(writers) = '');

UPDATE media_metadata SET pencillers = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(pencillers, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE pencillers IS NOT NULL AND (LTRIM(pencillers, ' ') LIKE ',%' OR RTRIM(pencillers, ' ') LIKE '%,' OR REPLACE(pencillers, ' ', '') LIKE '%,,%' OR TRIM(pencillers) = '');

UPDATE media_metadata SET inkers = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(inkers, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE inkers IS NOT NULL AND (LTRIM(inkers, ' ') LIKE ',%' OR RTRIM(inkers, ' ') LIKE '%,' OR REPLACE(inkers, ' ', '') LIKE '%,,%' OR TRIM(inkers) = '');

UPDATE media_metadata SET colorists = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(colorists, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE colorists IS NOT NULL AND (LTRIM(colorists, ' ') LIKE ',%' OR RTRIM(colorists, ' ') LIKE '%,' OR REPLACE(colorists, ' ', '') LIKE '%,,%' OR TRIM(colorists) = '');

UPDATE media_metadata SET letterers = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(letterers, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE letterers IS NOT NULL AND (LTRIM(letterers, ' ') LIKE ',%' OR RTRIM(letterers, ' ') LIKE '%,' OR REPLACE(letterers, ' ', '') LIKE '%,,%' OR TRIM(letterers) = '');

UPDATE media_metadata SET cover_artists = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(cover_artists, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE cover_artists IS NOT NULL AND (LTRIM(cover_artists, ' ') LIKE ',%' OR RTRIM(cover_artists, ' ') LIKE '%,' OR REPLACE(cover_artists, ' ', '') LIKE '%,,%' OR TRIM(cover_artists) = '');

UPDATE media_metadata SET editors = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(editors, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE editors IS NOT NULL AND (LTRIM(editors, ' ') LIKE ',%' OR RTRIM(editors, ' ') LIKE '%,' OR REPLACE(editors, ' ', '') LIKE '%,,%' OR TRIM(editors) = '');

UPDATE media_metadata SET links = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(links, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE links IS NOT NULL AND (LTRIM(links, ' ') LIKE ',%' OR RTRIM(links, ' ') LIKE '%,' OR REPLACE(links, ' ', '') LIKE '%,,%' OR TRIM(links) = '');

UPDATE media_metadata SET characters = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(characters, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE characters IS NOT NULL AND (LTRIM(characters, ' ') LIKE ',%' OR RTRIM(characters, ' ') LIKE '%,' OR REPLACE(characters, ' ', '') LIKE '%,,%' OR TRIM(characters) = '');

UPDATE media_metadata SET teams = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(teams, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE teams IS NOT NULL AND (LTRIM(teams, ' ') LIKE ',%' OR RTRIM(teams, ' ') LIKE '%,' OR REPLACE(teams, ' ', '') LIKE '%,,%' OR TRIM(teams) = '');

UPDATE series_metadata SET characters = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(characters, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE characters IS NOT NULL AND (LTRIM(characters, ' ') LIKE ',%' OR RTRIM(characters, ' ') LIKE '%,' OR REPLACE(characters, ' ', '') LIKE '%,,%' OR TRIM(characters) = '');

UPDATE series_metadata SET genres = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(genres, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE genres IS NOT NULL AND (LTRIM(genres, ' ') LIKE ',%' OR RTRIM(genres, ' ') LIKE '%,' OR REPLACE(genres, ' ', '') LIKE '%,,%' OR TRIM(genres) = '');

UPDATE series_metadata SET links = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(links, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE links IS NOT NULL AND (LTRIM(links, ' ') LIKE ',%' OR RTRIM(links, ' ') LIKE '%,' OR REPLACE(links, ' ', '') LIKE '%,,%' OR TRIM(links) = '');

UPDATE series_metadata SET writers = NULLIF(REPLACE(TRIM(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(writers, ' ,', ','), ' ,', ','), ', ', ','), ', ', ','), ', ', ','), ', ', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ',,', ','), ', '), ',', ', '), '')
 WHERE writers IS NOT NULL AND (LTRIM(writers, ' ') LIKE ',%' OR RTRIM(writers, ' ') LIKE '%,' OR REPLACE(writers, ' ', '') LIKE '%,,%' OR TRIM(writers) = '');

COMMIT;

SELECT 'after' AS stage, * FROM (
  SELECT 'media_metadata.genres' AS list, COUNT(*) AS malformed FROM media_metadata WHERE genres IS NOT NULL AND (LTRIM(genres, ' ') LIKE ',%' OR RTRIM(genres, ' ') LIKE '%,' OR REPLACE(genres, ' ', '') LIKE '%,,%' OR TRIM(genres) = '')
  UNION ALL SELECT 'media_metadata.writers' AS list, COUNT(*) AS malformed FROM media_metadata WHERE writers IS NOT NULL AND (LTRIM(writers, ' ') LIKE ',%' OR RTRIM(writers, ' ') LIKE '%,' OR REPLACE(writers, ' ', '') LIKE '%,,%' OR TRIM(writers) = '')
  UNION ALL SELECT 'media_metadata.pencillers' AS list, COUNT(*) AS malformed FROM media_metadata WHERE pencillers IS NOT NULL AND (LTRIM(pencillers, ' ') LIKE ',%' OR RTRIM(pencillers, ' ') LIKE '%,' OR REPLACE(pencillers, ' ', '') LIKE '%,,%' OR TRIM(pencillers) = '')
  UNION ALL SELECT 'media_metadata.inkers' AS list, COUNT(*) AS malformed FROM media_metadata WHERE inkers IS NOT NULL AND (LTRIM(inkers, ' ') LIKE ',%' OR RTRIM(inkers, ' ') LIKE '%,' OR REPLACE(inkers, ' ', '') LIKE '%,,%' OR TRIM(inkers) = '')
  UNION ALL SELECT 'media_metadata.colorists' AS list, COUNT(*) AS malformed FROM media_metadata WHERE colorists IS NOT NULL AND (LTRIM(colorists, ' ') LIKE ',%' OR RTRIM(colorists, ' ') LIKE '%,' OR REPLACE(colorists, ' ', '') LIKE '%,,%' OR TRIM(colorists) = '')
  UNION ALL SELECT 'media_metadata.letterers' AS list, COUNT(*) AS malformed FROM media_metadata WHERE letterers IS NOT NULL AND (LTRIM(letterers, ' ') LIKE ',%' OR RTRIM(letterers, ' ') LIKE '%,' OR REPLACE(letterers, ' ', '') LIKE '%,,%' OR TRIM(letterers) = '')
  UNION ALL SELECT 'media_metadata.cover_artists' AS list, COUNT(*) AS malformed FROM media_metadata WHERE cover_artists IS NOT NULL AND (LTRIM(cover_artists, ' ') LIKE ',%' OR RTRIM(cover_artists, ' ') LIKE '%,' OR REPLACE(cover_artists, ' ', '') LIKE '%,,%' OR TRIM(cover_artists) = '')
  UNION ALL SELECT 'media_metadata.editors' AS list, COUNT(*) AS malformed FROM media_metadata WHERE editors IS NOT NULL AND (LTRIM(editors, ' ') LIKE ',%' OR RTRIM(editors, ' ') LIKE '%,' OR REPLACE(editors, ' ', '') LIKE '%,,%' OR TRIM(editors) = '')
  UNION ALL SELECT 'media_metadata.links' AS list, COUNT(*) AS malformed FROM media_metadata WHERE links IS NOT NULL AND (LTRIM(links, ' ') LIKE ',%' OR RTRIM(links, ' ') LIKE '%,' OR REPLACE(links, ' ', '') LIKE '%,,%' OR TRIM(links) = '')
  UNION ALL SELECT 'media_metadata.characters' AS list, COUNT(*) AS malformed FROM media_metadata WHERE characters IS NOT NULL AND (LTRIM(characters, ' ') LIKE ',%' OR RTRIM(characters, ' ') LIKE '%,' OR REPLACE(characters, ' ', '') LIKE '%,,%' OR TRIM(characters) = '')
  UNION ALL SELECT 'media_metadata.teams' AS list, COUNT(*) AS malformed FROM media_metadata WHERE teams IS NOT NULL AND (LTRIM(teams, ' ') LIKE ',%' OR RTRIM(teams, ' ') LIKE '%,' OR REPLACE(teams, ' ', '') LIKE '%,,%' OR TRIM(teams) = '')
  UNION ALL SELECT 'series_metadata.characters' AS list, COUNT(*) AS malformed FROM series_metadata WHERE characters IS NOT NULL AND (LTRIM(characters, ' ') LIKE ',%' OR RTRIM(characters, ' ') LIKE '%,' OR REPLACE(characters, ' ', '') LIKE '%,,%' OR TRIM(characters) = '')
  UNION ALL SELECT 'series_metadata.genres' AS list, COUNT(*) AS malformed FROM series_metadata WHERE genres IS NOT NULL AND (LTRIM(genres, ' ') LIKE ',%' OR RTRIM(genres, ' ') LIKE '%,' OR REPLACE(genres, ' ', '') LIKE '%,,%' OR TRIM(genres) = '')
  UNION ALL SELECT 'series_metadata.links' AS list, COUNT(*) AS malformed FROM series_metadata WHERE links IS NOT NULL AND (LTRIM(links, ' ') LIKE ',%' OR RTRIM(links, ' ') LIKE '%,' OR REPLACE(links, ' ', '') LIKE '%,,%' OR TRIM(links) = '')
  UNION ALL SELECT 'series_metadata.writers' AS list, COUNT(*) AS malformed FROM series_metadata WHERE writers IS NOT NULL AND (LTRIM(writers, ' ') LIKE ',%' OR RTRIM(writers, ' ') LIKE '%,' OR REPLACE(writers, ' ', '') LIKE '%,,%' OR TRIM(writers) = '')
);
