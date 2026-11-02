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
| Web | **Grid view for smart lists** (table/grid toggle + size slider), including series/library-grouped lists (one section per group) |
| Web | **One grid/table layout for every list**: the grid/table buttons switch libraries, series, search (incl. author/genre lists), smart lists and files together; saved per user on the server and shown in *Settings → Preferences → Book and series lists* |
| Web | **Tables use the full width**: single-line rows (ellipsis, full value on hover), small covers, names/people/series/genres share the free width while numbers and dates stay compact, and on phones the table scrolls sideways instead of growing tall rows; new *Series* column (name and number). *Adjust columns* scrolls and keeps Cancel/Save in view on phones, has drag handles, lists the series table's columns, and opens on the columns actually shown. Fixes the library series table crash and smart list rows that kept the previous columns' cells |
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
| Web | **Remove books** from their book page, for the users an admin grants the *Remove Books* permission (*Settings → Users*, per user; the server owner always can): the file stays on disk, the book is hidden everywhere (web, search, smart lists, OPDS, library and series counts) and survives rescans, and you go back to the page you opened the book from (list, author, search, smart list, ...); library managers restore it from the library's *Settings → Danger zone → Deletion* |
| Server / GraphQL | **Add books** (intake, API): users an admin grants *Add books* upload books, folders of volumes or zips into a private inbox. Each book's metadata is read from the file and, when enabled per library, looked up on Open Library / Google Books; the server proposes the author folder (matching how the library already writes the author: `Eco, Umberto` → `Umberto_Eco/`), the file name in the library's style and tags from the library's own vocabulary with reasons (file subjects, the author's and series' other books, similar descriptions) and a few similar books to compare; manga and light novels go to their series folder (`_Manga/<Series>/<Series> vNN.cbz`). Accepting writes the metadata into the file the way book-curator does (EPUB OPF, PDF Info/Keywords, ComicInfo.xml), never overwrites, appends a row to `stump_intake.csv` (catalogo.csv columns + `added_by`, `added_at`, next to `catalogo.csv`) and scans just that folder; `recentlyAddedBooks` lists who added what |
| Web | **Add books** page (sidebar, and a button on library pages, for users with *Add books*): drop books, folders of volumes or zips (each upload with its own progress) into your inbox; one card per book or series shows the proposed path prominently, title / authors / series / language / year with where each came from (file, file name, Open Library, Google Books, you), an author or series folder picker (suggestions first, existing folders as you type, new folder), manga / light novel filing with volume numbers, tag chips with how sure the server is and why plus any tag from the library's vocabulary, *Does this look right?* with similar library books, duplicates and what blocks filing; every proposed tag says why in one line; tags only other books vote for (the author's or series' other books, similar descriptions) are *guessed*: hollow, in italics and, under 85% confidence, unticked until you tick them; items to check are highlighted (*Looks right, add it* confirms and adds in one click), *Add all ready* lists each book's destination and tags with toggles to untick any before adding, and filed books link to the library once scanned. Settings: *Add Books* in the user permissions, the library's *Add books* settings (series folders, CSV, online lookups) and the admin *Recently added* page |
| Web | Polish: book summaries expand inline; unimplemented controls are hidden (book *Reading timeline* tab, smart list *List rules* drawer, empty saved-view controls, the *Book clubs* sidebar section and the *Notifications* entries, which production builds don't implement) |
| Web | UI audit fixes: the smart list *Group* dialog keeps its title and buttons on screen and scrolls; the smart list filter builder shows field names (*Book meta › Genre*, not `book_meta.genres`) and keeps exact-match (`hasAnyOf`), `startsWith` and pattern rules instead of clearing them; on phones the list toolbars keep sort/filter/layout and Save/Group on screen while searching, the reader's contents tabs clear the close button and filter groups wrap their buttons; tables count the rows they show ("1 to 2 of 2"); readable values instead of raw ones (visibility, grouping, typeface *Default*, *Filters* drawer title); no avatar 404 on every page |
| Server | **Lower memory use**: the whole process (SQLite, libwebp and PDFium included) allocates with jemalloc, which returns freed memory after about a second, where glibc's malloc kept a growing arena per thread. Thumbnails are generated at half the CPUs, the blocking thread pool is capped at 4 threads per core (16 to 64, `STUMP_MAX_BLOCKING_THREADS`), and OPDS 1.2 feeds open 4 books at a time. On a copy of this library, generating 300 thumbnails peaked at 227 MiB (was 1.98 GiB) and left 57 MiB (was 1.9 GiB); browsing, OPDS, smart lists, readers and a scan end around 100-130 MiB (was 586 MiB; 0.1.11 used as much or more) |
| Fixes | Home rows' seek buttons work right away (they waited for a manual scroll); tables show a Writers column by default; removing a book or deleting a library no longer ends on a 404 page (deleting a library goes back to the page before it) |
| Fixes | The EPUB reader no longer crashes with React error #185 when going back into it (book → reader → book → browser back); the error screen's *Copy report* stack maps to source with `scripts/fork/map_stack.py` |
| Fixes | Continue-reading cards show titles again (metadata was lost since the SeaORM 2 upgrade); `?page=0` no longer crashes the server (offset underflow + `panic = "abort"`); OPDS 2 search group links no longer 404; OPDS 1.2 page-stream links keep the API key; Docker apt cache mounts are locked |
| Fixes | EPUBs without a cover image (it is optional) are no longer retried and logged as errors (with the whole server config) after every scan: the miss is logged once and remembered (`thumbnails/.no-cover/`) until the file changes or thumbnails are force-regenerated; they keep the default cover, a series or library takes its cover from a book that has one, and *not found* API responses (no cover, no avatar) are no longer logged as errors |
| Fixes | EPUBs whose reading order lists a file they don't contain, or one next to the package folder (`../toc.xhtml`), are read again instead of failing with *Failed to get chapter from epub file*: missing files are skipped (one warning per book version) and left out of the web reader, which no longer opens such a book on an HTTP 500 page. A changed book that still can't be read keeps its previous data and is retried only when its file changes again or a rebuild is forced, not on every scan |

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
