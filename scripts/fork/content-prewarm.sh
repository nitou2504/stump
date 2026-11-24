#!/bin/bash
# Copies the first content index (content_fts, with the books' text) into content_idx (no text) while
# Stump keeps running, so the deploy of migration m20261123_000000_content_contentless only has the
# changes since to copy. Safe to stop and run again (it copies only what content_idx lacks), safe on a
# database the migration has already converted (it says so and stops).
#
# usage: content-prewarm.sh [/path/to/stump.db] [ids per step, default 20000]
#   default db: /mnt/data2/containers/stump-fork/config/stump.db
#
# Needs the host's sqlite3 >= 3.43 (contentless_delete). Runs at idle priority; each step is one short
# write transaction, so Stump's own writes wait at most a few seconds (busy timeout 120 s here).
# The database file grows by the size of the new index (~3.5 GB for ~9,000 books) and is not shrunk
# until it is vacuumed (see FORK.md, content index).
set -euo pipefail
DB="${1:-/mnt/data2/containers/stump-fork/config/stump.db}"
STEP="${2:-20000}"
q() { sqlite3 -cmd '.timeout 120000' "$DB" "$@"; }

[ -f "$DB" ] || { echo "no database at $DB" >&2; exit 1; }
if [ "$(q "SELECT count(*) FROM sqlite_master WHERE name='content_fts'")" = 0 ]; then
  echo "no content_fts: nothing to copy (already converted, or never indexed)"; exit 0
fi
ver=$(sqlite3 --version | cut -d' ' -f1)
if ! printf '3.43.0\n%s\n' "$ver" | sort -V -C; then echo "sqlite3 $ver is older than 3.43" >&2; exit 1; fi

q "CREATE VIRTUAL TABLE IF NOT EXISTS content_idx USING fts5(
     title, text, content = '', contentless_delete = 1, tokenize = 'unicode61 remove_diacritics 2');"
max=$(q "SELECT coalesce(max(rowid), 0) FROM content_fts")
echo "copying sections up to id $max in steps of $STEP (content_idx has $(q 'SELECT count(*) FROM content_idx_docsize') of $(q 'SELECT count(*) FROM content_fts_docsize'))"
lo=0
while [ "$lo" -lt "$max" ]; do
  hi=$((lo + STEP))
  nice -n 19 ionice -c3 sqlite3 -cmd '.timeout 120000' "$DB" \
    "INSERT INTO content_idx (rowid, title, text)
       SELECT rowid, title, text FROM content_fts
       WHERE rowid > $lo AND rowid <= $hi AND rowid NOT IN (SELECT id FROM content_idx_docsize);"
  lo=$hi
  echo "  up to id $(( lo < max ? lo : max )) of $max  ($(date +%H:%M:%S))"
done
echo "done: content_idx has $(q 'SELECT count(*) FROM content_idx_docsize') sections; content_fts has $(q 'SELECT count(*) FROM content_fts_docsize')"
echo "the migration copies what changes from now on, then drops content_fts"
