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
| OPDS 1.2 | **Facets**: sort (best match while searching, title, author, series order, recently added) and language, with `thr:count` |
| OPDS 1.2 | **Authors / Genres** indexes: letter → value (with counts, searchable) → books; exact `?author=` / `?genre=` book filters |
| Web | **Authors** and **Genres** browse pages (`/browse/authors`, `/browse/genres`) with counts, letter jump bar and `Prefix:` grouping (e.g. `Estante: …` shelves) |
| Web | **Grid view for smart lists** (per-list table/grid toggle + size slider) |
| Web / GraphQL | **Exact list matching** (`hasAnyOf` / `hasNoneOf`): `Estante: Ciencia` no longer matches `Estante: Ciencia ficción`; genre/author badges and the filter drawer use it |
| Web | Filter drawer gains **language** and **tags**; the age-rating filter is fixed |
| Fixes | `?page=0` no longer crashes the server (offset underflow + `panic = "abort"`); OPDS 2 search group links no longer 404; OPDS 1.2 page-stream links keep the API key; Docker apt cache mounts are locked |

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
- `build-local.sh <tag>` — faster: builds the server and web client with cached
  crates/node_modules and assembles `docker/Dockerfile.fork`
