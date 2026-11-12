# Stump app — common-sense usability report (2026-11-12)

Usability lead, branch `feat/app-ux` (from `43fa3fe50`). Walked real tasks in an isolated sandbox: a copy of the live
database, books read-only, internal Docker network, image `stump:fork-71310fb5a` (the live UI; `43fa3fe50` only changes
the public sign-in page). Users: the owner `nitou` (English, dark theme) and a test friend `amiga` (Spanish, Biblioteca
only, Change password / avatar, Download files, Add books). Viewports 1440×900 and 390×844.

Screenshots: `research/appux-2026-11-12/before/<id>.png` (descriptions blurred; no passwords, keys or tokens).

Out of scope here (other squads): the public sign-in/signup screens, Settings → Public access, Book approvals, My
devices, Reading apps, Add books / intake, the user permission table, the sidebar warning badge. Issues found there are
in **For the other squads** at the end.

**Legend.** P1 breaks a task or misleads · P2 annoying · P3 polish. Effort S / M / L. **QW** = safe quick win (S effort,
low risk, no product decision). **Decision** = needs the owner's call before building.

## Status after implementation (branch `feat/app-ux`, 2026-11-12)

All 14 quick-win items are **done** (U21 only partly: the EPUB and PDF readers still say just "Stump"), each in its own commit with a test where the code has them (the new tests
fail on the old behaviour, checked by reverting each fix once). Evidence: `after/<same name as before>.png`, taken in
the same isolated sandbox with this branch's web build; the tab titles were read back with `page.title()`
(`after/titles.json`). Verification: the whole browser suite (172 files, 1,324 tests), `tsc -b` for `browser`,
`i18n` and `components`, and eslint on the 40 files this branch changed, all clean. No Rust, API or data change.

| id | status | commit | evidence |
|---|---|---|---|
| U2 | done | `fe179288b` | `after/p1-read-button-leido.png` ("Leer") |
| U3 | done | `9b103719e` | `after/p1-raw-key-settings-menu.png` (Book approvals), `-es.png` (Spanish: Añadidos recientemente, Libros borrados, Aprobar libros: the same words as `feat/access-ux`'s commit `c8ed54667`, so the two branches merge without a conflict) |
| U4 | done | `629a1f0c4` | `after/p2-language-picker-select.png` ("Español"), `-owner.png` ("English (US)") |
| U6 | done | `6005124c5` | `after/p2-english-in-spanish-menu.png` ("Bibliotecas") |
| U7 | done | `e471a8625`, `4f0a31e2c` | `after/p2-library-gear-friend.png`, `p2-series-gear-friend-deadend.png` (no gear for a friend), `p2-series-gear-editor-nonowner.png` (no series gear for a non-owner who holds Edit metadata), `p2-series-gear-owner.png` (the owner keeps it) |
| U8 | done | `06921fa62` | `after/p2-reading-lists-double-active.png` (one highlight) |
| U5a | done | `8c6ef95da` | `after/p2-english-in-spanish-menu.png` (Autores, Géneros, Añadir libros) |
| U5b | done | `f355553c8` | `after/p1-read-button-leido.png` ("de William Ospina"), `p2-english-books-in-this-series.png` ("Libros de esta serie", phone) |
| U5c | done | `bad3fd77f` | `after/p2-english-in-spanish-search.png`, `-results.png` |
| U5d | done | `ff9adf842` | `after/p2-pagination-spanish.png` ("20 por página", "1 de 41") |
| U5e | done | `af6e6458a` | no shot (owner-only page; covered by a Spanish render test) |
| U5f + U21 | done; U21 partial | `d59e3bce0` | `after/titles.json` (Inicio, Bibliotecas, Explorar, the smart list's and the comic's own names) |
| U14a | done | `9e7e3e20c` | `after/p2-new-friend-empty-home.png` ("Nada pendiente por ahora") |
| U17 | done | `2ab4c3a08` | `after/p2-language-picker-select.png` (no banner for a friend), `-owner.png` (banner stays) |
| U13a | done | `d9721504a`, `9b3169f41` | `after/p2-series-page-controls-file-size.png` (6 books: no A–Z, search, footer), `-big-series.png` (58 books: they stay), `p2-series-page-past-the-end.png` (page 2 of 1: they stay, the way back) |
| U22 | done | `2ac760370` | `after/p2-english-books-in-this-series.png` ("Leer más") |
| U24 | done | `d40edd2d7` | `after/p3-my-reading.png` ("Search books…" fits) |
| U19 | done | `6bd8e1127` | `after/p3-comic-page-count-twice.png` (bar up: the count once), `-bar-away.png` (bar away: the corner number) |

**Second pass (after the review).** Three fixes: the series gear is the owner's (`4f0a31e2c`), a page past the end keeps its
controls (`9b3169f41`), the Spanish settings-menu words equal `feat/access-ux`'s so the branches merge cleanly (`deb1a8961`;
a dry `git merge-tree` with `feat/access-ux` and with `feat/public-login` has no conflicts). Evidence refreshed in the same
isolated sandbox (this branch's rebuilt web bundle, a copy of the database, internal-only network, books read-only): four new
shots (`p1-raw-key-settings-menu-es`, `p2-series-gear-editor-nonowner`, `p2-series-gear-owner`, `p2-series-page-past-the-end`),
and the six pairs whose before-shot is phone-size but whose after-shot was desktop-size now have phone-size after-shots (390x844: `p1-read-button-leido`, `p2-english-in-spanish-menu`,
`p2-english-in-spanish-search`, `p2-language-picker-select`, `p2-library-gear-friend`, `p2-new-friend-empty-home`). Before this
pass `p2-english-in-spanish-menu.png` was a copy of the desktop home shot and the other five were desktop-size. Each scene
asserted what it shows in the browser (no gear link for the editor, one for the owner; the Previous button on page 2 of 1 leads
back to the nine books; the three Spanish menu labels; "Leer" and no "Leído").

Decisions made inside the quick wins (small, reversible):
- **U13a** hides the controls only when the list is loaded, fits on one page **and nothing narrows it**: while a search,
  a letter or a filter is applied they stay, so it can always be undone. They also stay on a page past the end (a bigger
  page size kept the old `?page=`: "page 2 of 1", an empty list), where the footer's Previous arrow is the only way out. In the table layout the footer stays (it holds
  the column controls). A short series still shows its sort, filter and layout buttons.
- **U19** applies to all three readers (EPUB, comic, PDF), which share the corner number and the top bar's position.
- **U5b** also translates the neighbouring "Story" / "Art" credit labels (comic and manga libraries), same lines.
- **U14a** also fixes the typo "recomendaciónes" in the next line.
- **U24** shortened the text (en "Search books…", es "Buscar libros…") instead of widening the compact box.
- **U7** is hidden, not disabled: the library gear shows to the owner and to anyone with Manage library (the library settings' own rule, `LibraryAdminLayout.tsx`); the series gear shows to the server owner only (the series settings route's own rule).
- **U21** is partial: the comic reader, smart lists, Home, Books and Libraries now say what you are on; the EPUB and PDF readers (`EpubReaderScene.tsx`, `PDFReaderScene.tsx`) still set no title, so their tab says just "Stump".
- **U17** needed `dismissAlert` exported from the `@stump/components` root (one word) so the banner is testable.
- **U5c** adds `src/__mocks__/locale.tsx`, a test helper that renders the app's real translations (en-US, es-ES).

**Left alone on purpose** (not quick wins, or someone else's screen): U1 (EPUB images over plain http: needs the
owner's decision), U9–U12, U13b, U14b, U15, U16, U18, U20, U23, U25–U27.

**New small findings while verifying** (not fixed, all cosmetic):
- The library page's own search box still says English "Search" in Spanish (`components/filters/Search.tsx`).
- The EPUB and PDF readers' tab titles stay "Stump": their components load the book themselves; a title there is a
  small follow-up in `ReadiumWebReader` / `NativePDFViewer`.
- "Vol." / "Ch." position labels (manga, webtoon) and "Book N of M" on the home page are English
  (`components/book/presentation.ts`, `scenes/home/OnDeck.tsx`).
- The Deleted books and Libraries empty states ("No libraries to display") are English, and so is the whole Deleted books page.
- The empty state "It doesn't look like there are any books here / Do you have any books in your library?" is hard-coded English
  in `SeriesBooksScene.tsx`, `LibraryBooksScene.tsx` and `BookSearchScene.tsx`; on a page past the end of a series
  (`after/p2-series-page-past-the-end.png`) it is also the wrong message (the series has books, only the page is empty).
- The Spanish settings menu cuts "Añadidos recientemente" to "Añadidos…" in its menu slot (`after/p1-raw-key-settings-menu-es.png`);
  "Recién añadidos" fits. Both branches carry the same words now, so change it once after the merge.
- `shared.common.xBooks` has no Spanish plural forms (reading-list and user-stat cards read "3 Books" in Spanish).

**For the other squads / the merge:** this branch touches `AppSidebar.tsx`, `LibraryHeader.tsx` (gear and the Add books
label), `en-US.json` / `es-ES.json` (distinct keys: `sidebar.buttons.{authors,genres,addBooks}`, `globalSearch.*`,
`pagination.*`, `seriesSettingsScene.*`, `bookOverviewScene.{by,moreBy,credits}`, `settingsScene.app/account.sections.language.placeholder`;
the three settings-menu keys `management.{recently added,deleted books,book approvals}` are NOT distinct: `feat/access-ux`
adds the same three, so this branch's Spanish is set to its words ("Añadidos recientemente", "Libros borrados", "Aprobar libros"),
which makes the two changes identical and the merge clean) and adds `URLFilterContainer`'s `hideFooter`. It does not touch `public_pages`, `deploy/public-edge`, the
auth scene, `settings/server/access`, `bookApprovals`, `settings/app/devices`, the Add books / intake screens or the sidebar badge.

## Summary

| id | P | screen | issue | effort | class |
|---|---|---|---|---|---|
| U1 | P1 | EPUB reader | images, styles and fonts of every EPUB fail on a plain-http address (home `http://192.168.0.17:10801`) | M | Decision (approach) |
| U2 | P1 | book page (es) | the main button says **"Leído"** (already read) instead of **"Leer"** | S | **QW** |
| U3 | P1 | settings menu | raw key `webApp.settings…` shown for Book approvals; Spanish also lacks Deleted books / Recently added | S | **QW** |
| U4 | P2 | Account → Language | picker shows "Select…" for every account (stored `en`/`es` never matches `en-US`/`es-ES`) | S | **QW** |
| U5 | P2 | everywhere (es) | hard-coded English: sidebar, byline "by", "More by", search dialog, pagination, series settings, tab titles | S each | **QW** |
| U6 | P2 | everywhere (es) | "Librerías" (= bookshops) for "Libraries" in ~45 strings | S | **QW** |
| U7 | P2 | library / series header (friend) | a gear that leads a friend to a dead end ("Esto es incómodo", or bounced to Libraries) | S | **QW** |
| U8 | P2 | sidebar | on Reading lists, both "Reading lists" and "My reading" are highlighted | S | **QW** |
| U9 | P2 | search | the exact title is not first ("dune" → *Dune* is 3rd); only 6 books are fetched, alphabetically | M | not QW |
| U10 | P2 | search | no Authors group; an author search shows 6 books, "See all" is below the fold | M | not QW |
| U11 | P2 | library / series / smart lists | unlabeled number chips (813 · 2 · 2/8756 · 20 hours · 56.6 GB; 1 · 89 · 344 · Private) | S–M | Decision |
| U12 | P2 | book cards (all grids) | file size (990 KB) under the title instead of the author | M | Decision |
| U13 | P2 | series page | A–Z bar, search box and "20 per page · 1 of 1" for a 6-book series; no "Continue series" | S / M | a: **QW**, b: Decision |
| U14 | P2 | home (new friend) | two empty boxes open the page; "No hay libros a mano" is an odd Spanish for "on deck" | S | a: **QW**, b: Decision |
| U15 | P2 | sidebar | all 45 smart lists are listed in the sidebar (and the phone menu) | M | Decision |
| U16 | P2 | Authors | sorted by first name (Isaac Asimov under I, Agatha Christie under A) | M | Decision |
| U17 | P2 | Account settings | "Contribute to Stump's translations" banner shown to every friend | S | **QW** |
| U18 | P2 | everywhere (es) | 650 of 2,770 UI strings have no Spanish (23%): reader 116, common 84, settings 90, permissions 43, PDF 28… | L | not QW |
| U19 | P3 | comic reader | the page count shows twice (top bar and bottom-right corner) | S | **QW** |
| U20 | P3 | book page | the action row (download, add to list, more) is icons only | S | Decision |
| U21 | P3 | browser tab | "Stump \| Home" in Spanish; readers and smart lists show just "Stump" | S | **QW** |
| U22 | P3 | book page (es) | "Leer Más" (Spanish uses sentence case: "Leer más") | S | **QW** |
| U23 | P3 | home carousels | ‹ › arrows shown when every card already fits | S | verify first |
| U24 | P3 | My reading | "Search finished books" placeholder is cut ("Search finished book") | S | **QW** |
| U25 | P3 | book page | curator tags ("Level: General Reader", "Canon: Reference") shown to friends | S | Decision |
| U26 | P3 | library page | two search entries (the library's own field and the global search icon) | S | Decision |
| U27 | P3 | library page | 813 series at 20 per page = 41 pages; 20 is not a multiple of the 7-column grid | S–M | Decision |

**Safe quick wins, in order:** U2, U3, U4, U6, U7, U8, U5 (a–f), U14a, U17, U13a, U21, U22, U24, U19.

## P1

### U1 — EPUB pictures and styles fail on the home address (plain http) · M · Decision (approach)
- **Screen:** EPUB reader, opened from `http://…:10801` (the owner's LAN address). `before/p1-epub-images-http.png`.
- **What happens:** the book opens at its cover page as a broken image ("Cover_Page" alt text); every image, the book's
  CSS and the reader fonts fail. The text pages themselves load.
- **Why:** the Readium navigator gives each book frame a Content-Security-Policy that starts with
  `upgrade-insecure-requests` (in the bundle: `EpubReaderScene-*.js`, the CSP builder `[\`upgrade-insecure-requests\`,
  \`default-src ${origins} blob:\` …]`). On an http page the browser rewrites the book's `http://` resources to
  `https://`, which the server doesn't answer and the frame's own `style-src http://…` then refuses. Verified in the
  sandbox: the server's manifest gives `http://` URLs; the browser requested `https://…/resource/OEBPS/images/cover.jpg`
  and blocked it (`FAILED … csp`). The tailnet (`:10802`) and the public address are https and are not affected.
- **Fix options:** (a) patch the Readium frame CSP (yarn patch) to omit `upgrade-insecure-requests` when the app itself
  runs on `http:` — it adds no protection there, the frame stays sandboxed; (b) tell the owner to use the tailnet https
  address at home (no code); (c) serve the home listener over https. **Risk:** medium for (a) (a dependency patch in the
  reader's isolation code; needs a test that the policy is unchanged on https).

### U2 — Spanish "Read" button says "Leído" (already read) · S · QW
- **Screen:** book page, Spanish. `before/p1-read-button-leido.png`.
- **What happens:** the primary button reads **"Leído"**; a Spanish reader thinks the book is already finished and
  doesn't know how to open it.
- **Why:** `shared.common.read` is the verb in both of its uses (`components/book/BookActions.tsx:128`,
  `components/readers/imageBased/container/NextInSeries.tsx:64`), but es-ES translated the past participle.
- **Fix:** es-ES `shared.common.read` → `"Leer"`. **Risk:** none.

### U3 — Settings menu shows a raw translation key · S · QW
- **Screen:** Settings, Management group. `before/p1-raw-key-settings-menu.png` (last item, `webApp.settings…`).
- **Why:** `SettingsSideBar.tsx:43` builds `settingsScene.sidebar.<group>.<label lowercased>`; `management.book approvals`
  is missing in en-US and es-ES. es-ES also lacks `management.deleted books` and `management.recently added` (English
  shows in the Spanish menu).
- **Fix:** add the three keys (en: "Book approvals"; es: "Aprobación de libros", "Libros eliminados", "Añadidos
  recientemente"). **Risk:** none. (Book approvals is the access-ux squad's screen; this is only the menu key in the
  shared locale file.)

## P2

### U4 — Language picker always shows "Select…" · S · QW
- **Screen:** Settings → Account → Language, both users. `before/p2-language-picker-select.png`.
- **Why:** the server stores `en` / `es`; the picker's options are `en-US`, `es-ES`…; `LocaleSelector.tsx:32` passes the
  raw value, so nothing matches. The app itself already maps `en`→`en-US` (`resolveLocale`, `i18n/src/config.ts:150`),
  which isn't exported. The ComboBox placeholder "Select…" is English too.
- **Fix:** export `resolveLocale` from `@stump/i18n`, use `value={resolveLocale(locale)}`; translate the placeholder.

### U5 — Hard-coded English in the Spanish UI · S each · QW
`before/p2-english-in-spanish-menu.png`, `before/p2-english-in-spanish-search.png`,
`before/p2-english-books-in-this-series.png`, `before/p2-library-gear-friend.png` ("per page").
- **a. Sidebar:** "Authors", "Genres", "Add books" literals (`components/navigation/sidebar/AppSidebar.tsx:132, 139,
  163`); the library header's "Add books" button label (`scenes/library/LibraryHeader.tsx`, `HeaderIconButton
  label="Add books"`).
- **b. Book page:** byline `by {writers}` (`components/book/BookOverviewDetails.tsx:112`); `More by ${author}`
  (`scenes/book/MoreByAuthor.tsx:55`); es-ES has no `booksInSeries` ("Books in This Series").
- **c. Search dialog** (`components/search/GlobalSearchDialog.tsx`): title "Search the library", placeholder "Search
  books, authors, series…", headings Discover / Books / Series, "See all N books", "Open in Explore", "N books", the
  footer hints "to move / to open / Ctrl K anywhere".
- **d. Pagination:** "per page", aria "Books per page" (`components/filters/URLPageSize.tsx:35, 41`) and "of".
- **e. Series settings:** "Metadata", "Extra information about your series" (`scenes/series/tabs/settings/
  SeriesSettingsScene.tsx`).
- **f. Page titles:** "Stump | Home", "Libraries" (see U21).
- **Fix:** move each to i18n keys with en-US and es-ES. **Risk:** low (text only; keep test ids/aria where tests use them).

### U6 — "Librerías" means bookshops · S · QW
- **Screen:** sidebar "Librerías", plus ~45 es-ES strings (library settings, permissions, `shared.common.libraries`,
  `shared.bookActions.goToLibrary`…). `before/p2-english-in-spanish-menu.png`.
- **Fix:** in es-ES replace librería(s) → biblioteca(s), Librería(s) → Biblioteca(s) (both feminine: articles and
  adjectives stay correct). **Risk:** none.

### U7 — Friends get a gear that goes nowhere · S · QW
- **Screens:** library header and series header as a friend. `before/p2-library-gear-friend.png` (the gear),
  `before/p2-library-gear-friend-result.png` (bounced to an English "Libraries" page),
  `before/p2-series-gear-friend-deadend.png` (series gear → "Esto es incómodo… en desarrollo").
- **Why:** `LibraryHeader.tsx:144` and `components/series/SeriesHeader.tsx:234` always pass `settingsLink`;
  `BookHeader.tsx:64` already gates it (`canManage`). Library settings need `ManageLibrary` (`LibraryAdminLayout.tsx:19`);
  the series settings page is the server owner's (`SeriesRouter.tsx` wraps its route in `ServerOwnerRouteWrapper`, which sends
  everybody else to `/series`, the under-construction page).
- **Fix:** pass `settingsLink` only with `ManageLibrary` (library) / to the server owner (series). **Risk:** low (owner unchanged).
  (The first version gated the series gear on `EditMetadata`, which still led a non-owner editor to the dead end; the review
  caught it and `4f0a31e2c` gates it on the owner, as the route does.)

### U8 — Two sidebar items active at once · S · QW
- `before/p2-reading-lists-double-active.png`. `AppSidebar.tsx` uses `pathname.startsWith(paths.myReading())`, and
  `'/reading-lists'.startsWith('/reading')`. **Fix:** match `/reading` exactly or `/reading/…`. **Risk:** none.

### U9 — Search doesn't put the exact match first · M
- `before/p2-search-exact-match-third.png`. The palette asks for `media(pageSize: 6)` with no order (alphabetical);
  "dune" lists *Chapterhouse: Dune*, *Children of Dune*, then *Dune*. With a common word the exact title may not be
  among the 6 at all. **Fix:** a relevance order (exact → starts with → contains) on the server, or a second exact-title
  query merged first. **Risk:** low–medium (query cost).

### U10 — No authors in search · M
- `before/p2-search-no-authors.png`. "asimov" gives 6 books and a series; no way to jump to the author; "See all N books"
  sits below the visible list. **Fix:** an Authors group (to `/browse/authors` filtered or the author's page), keep "See
  all" visible. **Risk:** low.

### U11 — Unlabeled number chips · S–M · Decision
- `before/p2-unlabeled-stat-chips-library.png`, `before/p2-unlabeled-chips-smart-lists.png`. Library header: 813 · 2 ·
  2 / 8756 · 20 hours · 56.6 GB; smart list cards: 1 · 89 · 344 · Private (on all 45). Readers can't tell what they count.
- **Proposal:** keep two meaningful ones with words ("813 series · 8,756 books", "2 read"), move the rest to the info
  sheet; don't show "Private" when it's the only case. Owner picks which numbers matter.

### U12 — File size on book cards · M · Decision
- `before/p2-series-page-controls-file-size.png`. Cards show "3 MB", "376 KB" under the title when not started
  (`components/book/BookCard.tsx:209`). Readers look for the author. **Proposal:** first author (needs the writers in the
  card fragment + codegen); keep size on the book page.

### U13 — Series page: controls that can't help, and no "continue" · S / M
- `before/p2-series-page-controls-file-size.png`. A 6-book series shows an A–Z bar (mostly disabled), a search field and
  a "20 per page · 1 of 1" footer.
- **a (QW):** hide the A–Z bar, the search field and the pagination footer when everything fits on one page.
- **b (Decision):** a primary "Continue series" / "Start with #1" button at the top.

### U14 — A new friend's home starts with two empty boxes · S
- `before/p2-new-friend-empty-home.png`. "No hay libros en progreso" and "No hay libros a mano" fill the first screen;
  the real content (Picks of the week, Recently added) is below.
- **a (QW):** better Spanish for `homeSections.onDeck.emptyText` ("Nada pendiente por ahora").
- **b (Decision):** hide Continue reading / Your next read while they're empty (they come back with the first book).

### U15 — 45 smart lists in the sidebar · M · Decision
- `before/p2-sidebar-45-smart-lists.png`. The sidebar (and the phone menu) lists every smart list; it dwarfs the rest and
  pushes Settings to the bottom. **Proposal:** show pinned ones (or the first 5) + "All smart lists", or collapse the
  group by default.

### U16 — Authors sorted by first name · M · Decision
- `before/p2-authors-sorted-by-first-name.png`. "A" holds "A. A. Kotov", "Aaron…", "Adam…"; Asimov is under I.
  **Proposal:** sort and group by surname (heuristic for particles: de, van, al-; keep "Most books").

### U17 — Translation-contribution banner for friends · S · QW
- `before/p2-language-picker-select.png`. Upstream's "Contribute / No thanks" box sits in every friend's account page.
  **Fix:** show it to the server owner only. **Risk:** none.

### U18 — 23% of the UI has no Spanish · L
- 650 of 2,770 keys are missing in es-ES and fall back to English: reader 116, common 84, settingsScene 48, settings 42,
  userPermissions 43, pdfReader 28, readerAnnotations 26, epubReader 21, sorting 20, readingStats 18… **Fix:** a
  translation pass, readers and common first (friends see those). Not a quick win.

## P3
- **U19 (QW)** comic reader: "1 / 194" in the top bar and again bottom-right (`before/p3-comic-page-count-twice.png`);
  hide the corner counter while the bar is shown.
- **U20 (Decision)** book page action row is three bare icons (`before/p3-book-actions-unlabeled.png`); on phones a
  short label under each icon would help.
- **U21 (QW)** tab titles: "Stump | Home" in Spanish; the readers and smart lists say only "Stump" — use translated
  titles and the book's title in the reader.
- **U22 (QW)** es-ES "Leer Más" → "Leer más".
- **U23 (verify)** home carousels show ‹ › when all cards fit (Continue reading with 2 books).
- **U24 (QW)** My reading: "Search finished books" is cut off (`before/p3-my-reading.png`); widen the field or shorten.
- **U25 (Decision)** curator tags ("Level: …", "Canon: …") on book pages for friends.
- **U26 (Decision)** library page has its own search field and the global search icon.
- **U27 (Decision)** library paging: 41 pages of 20; 20 isn't a multiple of the 7-column grid (last row of 6). Infinite
  scroll or a page size that fills rows.

## Tasks walked (result)
- Find a book by title: works, but the exact match isn't first (U9). By author: books only, no author entry (U10).
- Open and continue: Continue reading on home and My reading work; EPUB at home over http shows no images (U1).
- Browse a series in order: numbered #1–#6, correct order; extra controls, no continue button (U13).
- See what's new: "Recently added" on home works.
- Make a reading list and share it: works; the only "new list" control is an unlabeled + in the corner (owner decision
  whether to label it); sidebar double highlight (U8).
- Download: icon on the book page works (U20 for the label).
- Reader settings: comic/PDF readers fine; EPUB controls appear on tap.
- Switch language: the picker never shows the current language (U4); Spanish UI leaks English (U5, U6, U18) and the
  Read button is wrong (U2).

## For the other squads
- **access-ux:** the Settings button's warning badge is cut off at the bottom edge of the sidebar
  (`before/p3-gear-badge-clipped.png`, zoomed); the Settings button itself has no label. The Book approvals menu key is fixed here
  as U3 (shared locale file). The "Add books" sidebar label and library-header button are translated here as U5a
  (shared files); the Add books screen's own strings weren't inspected.
- **public-login:** none found on the in-app screens. U1 does not affect the public address (https).
- **Merge note:** U2–U6, U14a, U22 edit `packages/i18n/src/locales/en-US.json` / `es-ES.json`, which the other squads
  also edit; keep the edits to distinct keys so the merge stays mechanical.
