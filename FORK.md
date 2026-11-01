# Stump fork (`fork/main`)

A small fork of [Stump](https://github.com/stumpapp/stump) for a large, mostly-EPUB
library that is browsed from the web and from KOReader. It is based on the upstream
`metadata-based-libraries` branch (nightly + metadata organization) and keeps every
change small and upstreamable.

## What the fork adds

| Area | Feature |
|---|---|
| Libraries | *(upstream branch)* **Metadata organization**: series come from book metadata (EPUB `calibre:series` / EPUB 3 collections / ComicInfo), books without a series stay standalone, folders are ignored |
| OPDS 1.2 | **Shelves**: smart lists as navigation + book feeds |
| OPDS 1.2 | **Scoped search**: every book, library, series and shelf feed has its own `rel="search"` template, so KOReader's *Search* searches inside the feed being viewed |
| OPDS 1.2 | **Facets**: sort (best match while searching, title, rating, author, series order, recently added), language, minimum rating and age (general / adult), with `thr:count`; entries show `★ 4.1 · 18+` |
| OPDS 1.2 | **Authors / Genres** indexes: letter → value (with counts, searchable) → books; exact `?author=` / `?genre=` book filters |
| Web | **Authors** and **Genres** browse pages (`/browse/authors`, `/browse/genres`) with counts, letter jump bar and `Prefix:` grouping (e.g. `Estante: …` shelves) |
| Web | **Grid view for smart lists** (per-list table/grid toggle + size slider), including series/library-grouped lists (one section per group) |
| Web | **Smart list grouping dialog** (*Group: …* button, live preview): ungrouped, series, library, or any book detail (author, genre/tag, language, publisher, year, decade, rating, age rating) or a `Key: *` pattern over genres and tags (e.g. `Asignatura: *`); `Key: value` schemes found in the list are one-click options. Per viewer and browser, the list is unchanged; books default to series/title order |
| Web | **Filters on smart lists**: the same filter drawer as library/series books (genre, writer, language, tags, read status, ...), offering only the values present in the list |
| Web / GraphQL | **Exact list matching** (`hasAnyOf` / `hasNoneOf`): `Estante: Ciencia` no longer matches `Estante: Ciencia ficción`; genre/author badges and the filter drawer use it |
| Metadata | **Ratings** (0-5): read from EPUB `calibre:rating` (0-10, halved), ComicInfo `CommunityRating`, or the curator's precise `bcurator:rating`; new `rating` column (needs a *Rebuild metadata* scan to fill) |
| Metadata | **PDF series, age and rating**: PDFs have no such fields, so `bcurator:series=`, `bcurator:series_index=`, `bcurator:age=` (`18+` / `All ages`) and `bcurator:rating=` entries in the PDF Keywords are read like the EPUB fields; the other keywords are the PDF's **genres** (like EPUB `dc:subject`), so PDFs appear in the Genres pages, genre filters and genre smart-list rules; PDFs then join metadata series |
| Web | Filter drawer gains **language**, **tags**, **minimum rating** and **age** (general / adult); sort by rating or age rating; rating columns in tables; `rating` in smart list rules |
| Web | Book cards show **★ rating** and an **18+** badge; in a series they show the book's **#number** on the cover; the book page shows rating and age |
| Web | Book pages show the **whole series** (*Books in this series*, linking to the series): every book in series order, the current one outlined and scrolled into view; upstream's *Next in series* only listed the books after it. A **More by <author>** row follows (standalone books have no series row) |
| Web / OPDS | **Search**: split into words, every word must start a word of the book's file name, title, series, writers, publisher, genres or tags (any order, case- and accent-insensitive: `brown chemistry`, `quim`, `fisica diploma`); series match on name/title. Same rules in the web lists, the home box, smart lists (in the browser) and OPDS 1.2/2 |
| Web | **Back returns to the search**: searches live in the URL (`?search=`, home `?q=`, Authors/Genres `?q=`), typing doesn't add history entries; grouped smart lists open the matching groups and show only the matching books |
| Web | **EPUB reader**: themes (auto / light / sepia / dark / black) that colour the whole reader page (bars, margins, dialogs), a one-tap day/night button, and text alignment (book default / justified with hyphenation / left), kept across books |
| Web | **Remove books** from their book page (opt-in per user: *Settings → Preferences*, needs *Manage library*): the file stays on disk, the book is hidden everywhere (web, search, smart lists, OPDS) and survives rescans; restore it from the library's *Settings → Danger zone → Deletion* |
| Web | Polish: book summaries expand inline; unimplemented controls are hidden (book *Reading timeline* tab, smart list *List rules* drawer, empty saved-view controls) |
| Fixes | Home rows' seek buttons work right away (they waited for a manual scroll); tables show a Writers column by default |
| Fixes | The EPUB reader no longer crashes with React error #185 when going back into it (book → reader → book → browser back); the error screen's *Copy report* stack maps to source with `scripts/fork/map_stack.py` |
| Fixes | Continue-reading cards show titles again (metadata was lost since the SeaORM 2 upgrade); `?page=0` no longer crashes the server (offset underflow + `panic = "abort"`); OPDS 2 search group links no longer 404; OPDS 1.2 page-stream links keep the API key; Docker apt cache mounts are locked |

## KOReader tips

- Add the catalog as `http(s)://<host>/opds/v1.2/catalog` (or with the API key in the path).
- Open **Shelves → a shelf**, then use the top-left menu → **Search** to search inside it.
  The same menu shows the **Sort** and **Language** facets.
- Downloads are named `Author - Title` by KOReader. To keep the server file name instead,
  edit the catalog in KOReader and tick **Use server filenames**.

## Switching an existing library to metadata organization

Upstream only lets you choose the organization when a library is created. An existing
filesystem library can be switched in place (keeps book IDs, tags, progress, smart lists):

1. Back up `stump.db`, stop the server.
2. `UPDATE library_configs SET organization_mode = 'METADATA' WHERE library_id = '<id>';`
3. Start the server and run a custom library scan with **Rebuild metadata**
   (`scanLibrary(id, options: {config: {regenMeta: true}})`).
4. Run **Clean library** to remove the now-empty folder series.

## Building

The helper scripts in `scripts/fork/` run in containers on a build host:

- `check.sh` — `cargo check` of the server
- `test.sh '<cargo test args>'` — unit tests
- `schema.sh` — regenerate `crates/graphql/schema.graphql`, then
  `node.sh 'cd packages/graphql && yarn codegen'` for the TS client
- `build-image.sh <tag>` — the upstream multi-stage `docker/Dockerfile`
- `map_stack.py <assets dir>` — map a stack copied from the web error screen to source
  (`docker cp stump-fork:/app/client/assets /tmp/stump-assets` first; builds ship hidden source maps)
- `build-local.sh <tag>` — faster: builds the server and web client with cached
  crates/node_modules and assembles `docker/Dockerfile.fork`
