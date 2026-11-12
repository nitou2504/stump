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
  the series page is only useful with `EditMetadata`.
- **Fix:** pass `settingsLink` only with `ManageLibrary` (library) / `EditMetadata` (series). **Risk:** low (owner unchanged).

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
