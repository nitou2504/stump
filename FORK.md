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
| Web | **EPUB annotations, KOReader-like**: highlight, underline, strikethrough and invert, each with an optional note, private to each user. Selecting text shows a compact toolbar (*Highlight* in one tap, *Annotate* for the other styles and a note); tapping an annotation restyles it, edits its note or deletes it. Keys: `h` `u` `s` `i` `n`, `Del`, `Esc`, `b` bookmark, `?` shortcuts. Text is 22px by default |
| Web | **Remove books** from their book page, for the users an admin grants the *Remove Books* permission (*Settings → Users*, per user; the server owner always can): the file stays on disk, the book is hidden everywhere (web, search, smart lists, OPDS, library and series counts) and survives rescans, and you go back to the page you opened the book from (list, author, search, smart list, ...); library managers restore it from the library's *Settings → Danger zone → Deletion* |
| Web | Polish: book summaries expand inline; unimplemented controls are hidden (book *Reading timeline* tab, smart list *List rules* drawer, empty saved-view controls, the *Book clubs* sidebar section and the *Notifications* entries, which production builds don't implement) |
| Web | UI audit fixes: the smart list *Group* dialog keeps its title and buttons on screen and scrolls; the smart list filter builder shows field names (*Book meta › Genre*, not `book_meta.genres`) and keeps exact-match (`hasAnyOf`), `startsWith` and pattern rules instead of clearing them; on phones the list toolbars keep sort/filter/layout and Save/Group on screen while searching, the reader's contents tabs clear the close button and filter groups wrap their buttons; tables count the rows they show ("1 to 2 of 2"); readable values instead of raw ones (visibility, grouping, typeface *Default*, *Filters* drawer title); no avatar 404 on every page |
| Server | **PDF reader routes** (`/api/v2/pdf/{id}/layout`, `/page/{n}?w=`, `/text/{n}`, `/search`, no *Download* permission needed): pages as lossy WebP at a width tier that fits the screen (720–2000 px, `pdf_max_tier`), a per-page text layer from PDFium's chars (lines with their char indices and line ends, no duplicated text) for selection and annotations, accent-insensitive whole-book search with hit boxes. One render thread owns PDFium: a page a reader waits for goes first, identical requests render once, prefetches far from the reader are dropped, open documents are reused. The image reader's pages (`/media/{id}/page/{n}`, OPDS page streaming) use it too, lossy, with a +2/−1 background read-ahead instead of rendering ±5 pages at print quality on every request; the page cache is capped (`pdf_cache_max_mb`, 1024 MB). PDF annotation and bookmark locators are checked (page in range, rects inside the page) and PDF bookmarks keep their page |
| Web | **PDF reader with selectable text**: PDFs open in a new reader (the image reader stays a per-book choice, *Show as pages*, for comic-style PDFs; the browser's own viewer is one click away). Pages are the server's pictures at the width the screen needs, with the page's text laid over them, so it can be selected, copied, searched (*Ctrl+K*) and annotated like an EPUB: the same toolbar, styles (highlight, underline, strikethrough, invert), notes, shortcuts and help, private per user; tapping an annotation edits it. Bookmarks per page (`B`), the outline, bookmarks and annotations in one panel, go to page (`G`), fit width / fit page (`F`), zoom (`+ - 0`, Ctrl+wheel, pinch), continuous or single-page, reader themes (dark ones invert the pages, which can be turned off per book). Only the pages near the screen are kept (at most 7); the next two and the previous one are fetched ahead, and the first page's picture is asked for with the layout. Scans without text read the same and keep bookmarks |
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
