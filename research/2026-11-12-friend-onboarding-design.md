# Friend onboarding for Stump: self-signup links, sign-in without a pass, reading-app keys and owner warnings

*Design only, 2026-11-12. Branch `feat/friend-onboarding` from `93eda9ca8` (the live `stump:fork-93eda9ca8`). No code was
written and nothing live was touched: this was done by reading the code, the live edge files in `deploy/public-edge/`, the
earlier Public access design (`research/2026-11-11-stump-public-access-design.md`, §13 "as built" included) and a read-only
look at the live database (library names, users; nothing printed that is secret). It builds on what is deployed: passes,
the state file, the invite pages, the sign-in marks, the audit log, `NotViaPublicEdgeGuard`, the device-bound sign-in.*

**Owner request (all of it).** (1) Nobody creates accounts by hand: in *Settings → Public access* the owner makes an
**invite link**, for one person or **open** (several people), valid N days (default 3), with a **budget** (how many accounts
it may create). The visitor opens it, chooses a username and password, and is in: account made from the link's **preset**,
device given its pass, signed in. (2) **Default preset**, editable per link and shown clearly: Change password, Change
avatar, Download files, Add books (never Scan library, Upload files, Manage *); **only the library "Biblioteca"**, and
libraries made later stay hidden; optional age limit; **3 devices per person**. (3) **New devices just sign in**: the
public address serves, without a pass, only the sign-in page, its assets and the sign-in endpoint; a good sign-in by a
friend-class person gives that device its pass, counted against the limit (refused with a clear message at the limit, and the
owner is warned); owner/admin/manage-* accounts are still refused through the edge; per-IP limits, per-name progressive
backoff (never a lock an attacker can use against friends), a global failure budget, no username enumeration, constant-ish
timing, a minimum password length of 10 for invite-made accounts, every failure audited and surfaced; *My devices* for
people. (4) **OPDS for reading apps** through the edge: a *Reading apps* page gives a personal, revocable, read-only URL
with a key in the path (and a QR code), no cookie needed, rate limited, Basic stays blocked, each key counts as a device,
revocation instant. (5) **Owner warnings**: people at/over the limit, 4+ sessions, failure bursts, links near their budget;
a badge in the sidebar. (6) **Scan hardening**: the user editor stops ticking Scan library for Upload files; every scan
trigger is refused through the public edge.

---

## 0. Decisions

| # | Decision | Why |
|---|---|---|
| D1 | **The public sign-in page is the app's own small server-rendered page, not the web app.** The edge answers a pass-less `GET /` by rewriting it internally to `/api/_i/signin`; `POST /api/_i/signin` signs in. Nothing of the web app (HTML shell, JS, GraphQL, `/api/v2/claim`) is reachable without a pass. | The invite pages already prove the pattern: no JavaScript, strict CSP, English/Spanish by `Accept-Language`, a few KB, unit-testable. The web app needs `/api/v2/claim`, GraphQL, ~440 KB of JS and a service worker before its own login screen draws, and each of those would have to be opened to strangers (§1 F1). |
| D2 | **One switch, `STUMP_EDGE_PUBLIC_SIGNIN`** (environment only, default `false`). Off: the new pages answer 404 and the app behaves as today. On: pass-less sign-in and signup links work. | The deploy order (app, then edge, then switch) keeps every step safe while Funnel is on, and turning it off is the instant rollback (§10). |
| D3 | **Who may sign in without a pass**: `access_people.public_signin` is true, **and** the account is not the owner, **and** every permission it holds is in the *friend set* (D12), **and** it is not locked, **and** it has a password. Anything else gets exactly the answer of a wrong password, after the same work. | The owner's "non-owner, non-admin, enabled" rule, as a default-deny list. `book-curator`, the owner and anybody with a Manage permission can never come in this way, whatever their password. |
| D4 | **No permanent lock from the internet.** A pass-less failure never counts towards `is_locked`; it feeds (a) a **per-name progressive backoff** and (b) a **global failure budget**, both in memory and bounded. Only a failure made *through a device bound to that same person* still counts towards Stump's lock (as today). | An attacker who could lock `ana` by typing 10 wrong passwords would turn the sign-in page into a lock-out weapon. A backoff only delays further *new-device* attempts for that name; a friend's known devices keep working. |
| D5 | **A device is a pass of either kind** (a browser's pass, or a reading-app key). Each person has a limit (default 3, 1 to 10). It is enforced where a device is made, by one atomic conditional `INSERT` (so ten parallel sign-ins cannot exceed it). Devices unseen for 60 days no longer count. At the limit the sign-in is refused after a **correct** password, with a clear message, an audit row and an owner warning. | The owner's "3 devices per person", without a dead end for people who cleared their cookies (D16). |
| D6 | **Signup links are a new table** (`access_signup_links`: kind `single` or `open`, expiry, budget, preset). Redeeming is one transaction: spend one use (conditional `UPDATE`), create the account, its access row, its library decisions and its first device. The preset is validated **on the server** against the friend set; a link can never grant more than that. | One conditional spend makes the budget race-free; a failed username rolls the spend back; a tampered preset (or a client that skips the UI) cannot hand out Manage permissions. |
| D7 | **Libraries for people with a preset**: an allowlist at signup (their exclusions are every library but the preset's); libraries that appear **later** are hidden by a *decision log* (`access_library_decisions`: each person × library is decided exactly once). The decision runs in a hook on library creation, at start-up and every 10 minutes, and at every public sign-in. The owner's own later changes to a person's access are never overwritten. | Stump's visibility is an exclusion model, so a new library is visible to everybody by default (F3: Dani sees all four libraries today). The log keeps every existing code path that reads `library_exclusions` unchanged. |
| D8 | **Names and passwords for accounts made through a link**: username 3 to 32 characters of `a-z 0-9 . _ -`, starting with a letter or digit, stored lowercase, unique **case-insensitively** against every account (removed ones included), a reserved list refused. Password 10 to 72 bytes, at least 5 different characters, not in an embedded list of common ones, not containing the username. The same password rules apply when a *public-sign-in* account changes its own password. | The owner's "10 for invite-made accounts"; the 72 is bcrypt's own limit (it truncates silently beyond it, F7); lowercase names stop `Ana`/`ana` impersonation and phone auto-capitalisation surprises. |
| D9 | **Reading-app keys are devices of kind `opds`.** The key is `<id>.<mac>` with `mac = HMAC-SHA256(STUMP_EDGE_KEY, "opds\|" + id)`: derived, not stored, so nothing secret is in the database and the *Reading apps* page can show the URL again. It travels in the path (`/opds/<key>/v1.2/…`). The **edge** verifies it (njs, revoked set from the state file) and the **app** verifies it again; it is accepted **only** by the OPDS 1.2 middleware, for `GET`/`HEAD`, as the person with only *Download files*. | The owner's "scoped, revocable, read-only, counts as a device, instant revocation, no cookie". Domain separation (`opds\|` against `dev\|`) means a pass is not a key and a key is not a pass; derived tokens mean a database leak yields nothing. |
| D10 | **Warnings are computed, not stored** (devices, sessions, the sign-in marks, a few audit rows, the links). Event-type warnings fade after 3 days; state-type ones last while true. The sidebar badge counts warnings and alerts, not info. | No new state to keep consistent; the badge always agrees with the page. |
| D11 | **Scan hardening**: the web user editor stops implying Scan library from Upload files; **every** GraphQL mutation that enqueues a job gets `NotViaPublicEdgeGuard` (not only the three scan mutations); a test walks `src/mutation/*.rs` so a future trigger without the guard fails the build. | "Any other scan trigger" is answered by a rule the build enforces, not by a list that goes stale. |
| D12 | **The friend set** (what a public-sign-in account may hold, and what a signup link may grant): Change password, Change username, Change avatar, Download files, Upload files, Add books, Access smart lists. Anything else (Scan library included) takes an account out of public sign-in. Enforced at link creation, at public sign-in, when a device link is redeemed, and **on every public request** for accounts that have `public_signin` (a Manage permission granted later cuts the account off from the internet at once, still fine at home). | The owner's "owner/admin/manage-* are refused through the edge", made default-deny and kept true after the fact. |
| D13 | **A signup link only creates accounts.** An account that already exists becomes public only by the owner's decision: the *Public sign-in* switch in People, or a device link made for that person (which now also turns it on for friend-class accounts). The signup page offers "Already have an account? Sign in", and that works only if the owner enabled it. | The owner's "or signs in with an existing account they already own", without letting whoever holds a link switch an account on to the internet by knowing its password. |
| D14 | **The wrong-person rule becomes a gate**: with the switch on, a sign-in through a device bound to somebody else (or a legacy pass with no person) is treated like a pass-less one: same eligibility, same backoff, no lock counting. | Otherwise two people sharing a computer cannot sign in on the second one's account (D6 of the earlier design protected links, which now have a better answer in the gates). |
| D15 | **Migration is additive**: three new tables and one new column (`access_devices.kind`); nothing else of Stump's schema moves; a manual-down SQL is tested like the last one. | Same safety as `m20261113_000000_public_access`. |
| D16 | **At the limit, the page can also offer "sign out one of your other devices and continue"** (recommended, separable): after the correct password, a short-lived single-use ticket lets the person remove one of their own devices and finish the sign-in. The baseline (a message pointing to *My devices* or the owner) is complete without it. | A friend whose three slots are old browsers they cleared cannot reach *My devices* from the new one. Costs about half a day; the lead may cut it. |
| D17 | **A daily cap on file downloads per reading-app key** at the edge (default 300 a day; *Reset budget* in Devices resets it). | A leaked key could otherwise pull a whole library at line speed; the cap bounds it and is a few lines in the gate. Cuttable. |
| D18 | **Terminal tools stay as break-glass**: `edge-common/invite.sh stump …`, `edge/revoked.map`, and `STUMP_EDGE_PUBLIC_SIGNIN=false`. | If the pages cannot be reached the owner is not locked out of controlling access. |

---

## 1. What was read, and what it means

**Read:** `routers/invite.rs`, `middleware/edge.rs`, `middleware/auth.rs` (`auth_middleware`, `authorize`, `api_key_middleware`,
`validate_api_key`), `routers/api/v2/auth.rs` (`login`, `track_login_attempt`), `routers/opds/{mod,v1_2}.rs`,
`core/src/opds/v1_2/{entry,feed,link}.rs`, `core/src/access/{pass,state}.rs`, `core/src/config/edge_config.rs`,
`crates/models/src/services/{access,user}.rs`, `crates/models/src/entity/{library_exclusion,session,user,api_key}.rs`,
`crates/models/src/shared/{enums,permission_set,api_key}.rs`, `crates/graphql/src/{guard,data}.rs` and the access,
user, library, series, media, upload and intake mutations, `crates/migrations/src/m20261113…` and its manual-down,
`deploy/public-edge/{nginx.conf,stump-gate.js,vectors.json,verify-edge.sh,DEPLOY.md}`, the web settings (`routes.ts`,
`AccessScene.tsx`, `UserPermissionsTable.tsx`), the sidebar badge (`ApprovalsBadge.tsx`), and (read-only) the live
database's libraries and users.

| # | Finding | Consequence |
|---|---|---|
| F1 | Without a pass the edge answers 404 to everything except `/api/_i/…`, the manifest and the favicons; the app's `is_open_public_path` says the same. The web app's own login screen first calls `GET /api/v2/claim` and GraphQL. | A sign-in people can reach without a pass has to be a page of the app's own under `/api/_i/` (already open, already rate limited, already hidden from logs), not the web app (D1). |
| F2 | `login()` lets a device bound to person *P* sign in only as *P*; other names get `wrong_person` (recorded, not counted). A failure through a bound device counts towards the lock (the 10th in 24 h). | The binding was the protection while passes came only from the owner's links. With pass-less sign-in the protection has to move into gates (D3, D4, D14). |
| F3 | Library visibility is `library_exclusions` (user × library, "hidden"). A library made later is visible to everybody. Live: the libraries are Biblioteca, Guides, Manga, Light Novel; `dani` has **no** exclusions, so sees all four, and holds Change password/username/avatar, Download, **Upload files, Scan library**, Add books. | The preset's "Biblioteca only, and later ones hidden" needs the decision log (D7). Dani needs Scan library taken off before public sign-in can be turned on for her (D12; Upload files is in the friend set), and her libraries narrowed (one action in People). |
| F4 | OPDS 1.2 already has a key-in-the-path route: `/opds/{api_key}/v1.2/…` behind `api_key_middleware`, **shared with the KOReader and Kobo sync routes**. The feed builders take the key string from the request (`req.api_key()`) and build every link as `/opds/{key}/v1.2/…`, so any token string works. The download handler enforces *Download files* and the user's visibility from the user it is given. | The reading-app key can occupy the same path slot with its own middleware in front, and be refused by every other route structurally (D9). |
| F5 | The web permissions table maps `UploadFile → [ScanLibrary]` (`UserPermissionsTable.tsx`, `associatedPermissions`); the server's `AssociatedPermission` has **no** such link. | The auto-tick is a web-only convenience and is simply removed (D11). |
| F6 | Job-enqueuing mutations (`scanAllLibraries`, `scanLibrary`, `scanSeries`, `analyzeMedia`, `analyzeSeries`, library analysis and thumbnail jobs, post-processing, upload scans, metadata fetch) are guarded by permissions only; `NotViaPublicEdgeGuard` already exists and is used on approvals and uploads. No REST handler enqueues a job. | One guard on each, plus a coverage test (D11). |
| F7 | Passwords are hashed with `bcrypt` at `password_hash_cost`; bcrypt silently uses only the first 72 bytes. `verify_dummy_password` exists and costs the same as a real check. | Max 72 bytes in the policy; unknown, ineligible and wrong all do one bcrypt. |
| F8 | `users.username` is `UNIQUE` with SQLite's default (case-sensitive) collation. | Uniqueness against `lower(username)` is checked explicitly (D8). |
| F9 | The sidebar already shows a badge for pending approvals on the Settings button (`ApprovalsBadge`, polls every 60 s, for those who approve). | The warnings badge follows the same pattern, for the owner only. |
| F10 | `TraceLayer` redacts `/api/_i/…` (as built); the edge's `$log_uri` map hides it and `error_log … crit` keeps it out of the error log. | The same three places must also hide `/opds/<key>/…` (§3). |
| F11 | `authorize()` is the one place every credential passes before a handler runs; it already refuses the owner on public requests and enforces "must change password". | The friend-class request check (D12) belongs there, next to them. |
| F12 | A 6-hour housekeeping loop and a state file (`state.json`: `revoked` ids, `budget` epochs) already exist, with the edge following it once a second. | Reading-app key revocation and budget resets reuse both unchanged. |

---

## 2. Architecture

```
 anybody / a friend ─▶ Funnel :8443 ─▶ stump-edge (public :8081, nginx + stump-gate.js) ─▶ stump-fork (the app)

   GET  /                 no pass ── internal rewrite ─▶ /api/_i/signin         the sign-in page (app, server-rendered)
   POST /api/_i/signin    6/min per client, 30/min all ─▶ gates → password → eligibility → device (limit) → session → pass
   GET  /api/_i/<token>   ─▶ signup form (a signup link) | one-button page (a device link) | the same "doesn't work" page
   POST /api/_i/join      3/min per client ─▶ spend a use + create the account + preset + device + session → pass
   GET  /opds/<key>/v1.2/… the edge verifies the key itself (njs) ─▶ app: reading-app middleware, GET/HEAD, Download files only
   a valid pass           ─▶ everything as today (the web app, GraphQL, uploads …)
   everything else without a pass: 404
```

Three things mint a device, and each makes exactly one row in `access_devices`: the **sign-in page** (browser pass), a
**signup link** (the first browser pass of a new account) and **a reading-app key** (kind `opds`). The old device link
for an existing person keeps working and makes a browser pass as before.

### 2.1 Public sign-in (the page, and the shared gate)

`GET /api/_i/signin` renders the form (username, password, a "have an invite link? paste it here" box that posts to
`/api/_i/`, and "forgot your password? ask the person who runs this server"). `POST /api/_i/signin` (`application/x-www-form-urlencoded`,
the edge allows 4 KB) runs `public_signin::attempt(…)`, **the same function the web app's own `login()` calls for a request
that is not bound to the person it names** (D14). Order of checks, and what each answers:

1. **Available?** `via_public` (the edge marker) ∧ a usable key ∧ `STUMP_EDGE_PUBLIC_SIGNIN`. Otherwise **404**, as if the route did not exist.
2. **Same site?** `Sec-Fetch-Site` same-origin/none (or `Origin` equal to the public address); else the generic failure page. Field lengths: username ≤ 64 characters, password ≤ 128 bytes; longer is the generic failure *without* hashing (no CPU for a megabyte "password").
3. `name_key = sha256(lowercase(trim(username)))[..16]`: the key of the backoff table. It exists for **every** typed name, real or not, and never holds the text.
4. **Global gate.** 40 failures in the last 10 minutes across all names → **429**, "too many attempts, try again in a few minutes", no password work. (Refused attempts are not counted, so the window slides and recovers.)
5. **Name gate.** `blocked_until(name_key) > now` → the same **429**, no password work. The message is identical for names that exist and names that do not (no enumeration).
6. **Find the account.** Exact username; else exactly one case-insensitive match; else none.
7. **One bcrypt, always**: the real hash if the account exists and has a password, `verify_dummy_password` otherwise (unknown, no password, OIDC-only). Same cost either way.
8. `ok = password is right ∧ eligible`, where *eligible* is D3. (A person whose password the owner set and who must choose their own may sign in and is then held to that screen, as today.)
9. **Not ok** → record, count, answer. Record (account exists only): a `user_login_activity` row with a mark (`wrong_password`, `not_allowed_publicly` for a right password on an ineligible account, or `locked`), `counts_toward_lock = false`, no device. Count: `tracker.fail(name_key)` and the global window. Answer: the **same 401 page** for all of them: "That username or password isn't right. Still stuck? Ask the person who runs this server." The password is never echoed; the username is.
10. **Ok** → **device gate**: one conditional insert (§2.4). Refused → **403** "You already use 3 devices" page (§2.4); an audit row `signin.device_limit` and a warning; *not* a failure for the backoff (the password was right).
11. **Signed in**: `tracker.reset(name_key)`; a success row with the new device id in the mark; `enforce_max_sessions` and the session exactly as `login()` does (one shared function); the pass cookie from `pass::mint_pass` + `pass_cookie_header`; **303 to `/`** with both cookies. If making the session fails, the device row made in step 10 is deleted (no ghost).

A request that **already carries a valid pass bound to this very person** is not pass-less: it skips the name and global gates (steps 3 to 5), reuses that device (no new row, no limit check), and its failures count towards the lock as today (with the edge's 8 sign-ins per device per day in front). A pass bound to somebody else, or a legacy pass, is *ignored* for this decision (D14).

**Backoff schedule** (per name, window of 60 minutes since the first failure, reset by a success): failures 1 to 4 cost nothing (typos);
from the 5th, further pass-less attempts for that name are refused for 1, 2, 4, 8, 16 and then 30 minutes (cap). The owner is told
once per name per hour (`signin.backoff` audit row; the audit never holds the typed text for an unknown name, only a short hash prefix and a count).
The table holds at most 4096 names and forgets idle ones after an hour; a process restart clears it (an attacker cannot cause one).
The edge's own limits (6/min per client, 30/min overall) bound how fast the table can be filled, and the global gate trips long before it overflows.

**What this does and does not give.** An attacker can make a name wait, never lock it, and cannot touch a friend's *known* devices (they carry a bound pass
and are outside the name gate). A slow distributed guess is bounded to about 100 tries a day per name by the backoff and to 40 failures per 10 minutes overall,
against passwords of at least 10 characters that are not on the common list; and the owner sees it (warnings, §2.6).

### 2.2 Signup links and the account they make

`GET /api/_i/<token>` looks the token's SHA-256 up in `access_invites` (device links, as today) and then in `access_signup_links`.
A valid, unspent signup link shows the form: **username, password, password again** (no JavaScript, so no show/hide; `autocomplete="new-password"`, `minlength=10`,
`autocapitalize="none"`; large touch targets; light and dark; en/es). Anything invalid (unknown, expired, used up, revoked, no libraries left) shows the one
generic "this link doesn't work" page. A GET spends nothing and sets nothing (a chat app's link preview cannot use a link up). (The device-link page of the earlier design checked only the token's *shape* on GET; this one needs the lookup to know which page to show, which tells a visitor nothing they could use: a 128-bit random token that is not in the table gets the same page as an expired one.)

`POST /api/_i/join` (`t`, `username`, `password`, `password2`), in this order:

1. available (as §2.1 step 1), same site, field lengths; the generic page on any failure so far.
2. The link is looked up and checked *before* looking at the name or password, and a bad link gives the generic page: no oracle on names without a valid link.
3. Name and password rules (D8). A violation re-renders the form with the specific message (the visitor holds a valid link, so this reveals nothing to strangers); "that name is taken" is inherent to choosing a name.
4. **bcrypt hash, outside the transaction** (`spawn_blocking`, `password_hash_cost`), so the writer lock is never held for 250 ms.
5. **One transaction**: (a) `UPDATE access_signup_links SET uses = uses + 1 WHERE id = ? AND uses < max_uses AND expires_at > now AND revoked_at IS NULL`, zero rows → the generic page; (b) re-read the preset and keep only the libraries that still exist (none left → the generic page, nothing spent); (c) insert the user (`user::ActiveModel`, not owner, permissions from the preset through `PermissionSet`, `max_sessions_allowed = 2 × maxDevices`) with `create_user_with_preferences` and the preset's age restriction; (d) `access_people` (`public_signin = true`, the preset's `max_devices`, `new_libraries_hidden = true`, the link id); (e) exclusions for every library not in the preset and a decision row for every library (§2.3); (f) the first device (conditional insert, label from the browser); (g) audit `signup.created` (target the user id, detail the link id; never a password). A `UNIQUE` violation on the name rolls the whole thing back, **the use is not spent**, and the form is shown again with "taken".
6. Session and pass cookie exactly as §2.1 step 11; 303 to `/`.

**Budget and abuse of an open link.** The budget is the cap (single = 1; open = 2 to 25, default 5); the expiry is the other (1 to 30 days, default 3); the edge limits are 3 a minute per client and a small global burst; each account is a normal friend-class account with the preset's Biblioteca-only view; adding books needs the owner's approval; and the owner is warned when a link is within one use of its budget. If a link leaks, **Revoke** stops new accounts at once and its optional **"also turn off the accounts it made"** sets `public_signin = false`, revokes their devices and ends their sessions (the accounts remain for the owner to keep or remove). The create dialog shows a plain warning when an *open* link carries *Add books*.

**The token is never logged**: not in the edge's access or error log (`$log_uri`, `error_log … crit` in the token locations), not in the app's trace spans (`/api/_i/…` is redacted), not echoed in the page except as a hidden field when it is token-shaped, and the POST carries it in the body.

**Preset** (JSON in the link row, validated at creation and again at redemption):

```json
{"permissions": ["CHANGE_PASSWORD", "CHANGE_AVATAR", "DOWNLOAD_FILE", "ADD_BOOKS"],
 "libraryIds": ["501b2b44-edc4-4083-a7e9-c26d0e3082bc"],
 "ageRestriction": null,
 "maxDevices": 3}
```

Creation refuses permissions outside the friend set (D12), unknown libraries, and `maxDevices` outside 1 to 10. The form pre-ticks the library **named** "Biblioteca" (case-insensitive) and warns "they would see nothing" if none is ticked.

### 2.3 Libraries for people with a preset, now and later (D7)

* **Signup** (`apply_library_preset(txn, user_id, allowed_ids)`): for every library, one `access_library_decisions` row; for every library not allowed, a `library_exclusions` row. The same function serves *People → Public sign-in → "Only these libraries"* for an existing account such as Dani's (the owner sees what will be hidden before confirming).
* **A library made later**: `on_library_created(txn, library_id)` runs inside `createLibrary`'s transaction: for each person with `new_libraries_hidden` it inserts the exclusion and the decision. `reconcile_libraries(conn)` finds, for each such person, libraries with no decision row and applies the same rule; it runs **at start-up, every 10 minutes (its own task next to `spawn_housekeeping`, which returns early when no edge is configured: libraries must stay hidden even if the edge settings are removed), and at every public sign-in of that person**, so a library made outside the hook (a script that inserts into `libraries`, as the Manga/Light Novel split did) is hidden before the next sign-in and within 10 minutes for a person already signed in.
* **Never overwritten**: a (person, library) pair is decided once. If the owner later shows Manga to Dani in the library's access settings, no reconcile hides it again. Deleting a library or a user cascades the log.
* *Enabling* public sign-in for an existing account marks all current libraries decided (leaving their exclusions as they are, unless the owner picked "Only these libraries"), so only libraries that appear afterwards are hidden. The switch is per person (`new_libraries_hidden`, on by default, the owner can turn it off for a trusted friend).

### 2.4 Devices: counting, the limit, self-service

* **Counting.** `active devices of P` = rows with `person_id = P`, `revoked_at IS NULL` and `COALESCE(last_seen_at, created_at) > now - 60 days`, of either kind. A device that comes back after 60 days simply counts again (it is not re-checked against the limit; the owner is warned if that makes it exceed).
* **The limit is applied by one statement**: `INSERT INTO access_devices (…) SELECT … WHERE (SELECT count(*) FROM access_devices WHERE person_id = ? AND revoked_at IS NULL AND COALESCE(last_seen_at, created_at) > ?) < ?`; `rows_affected = 0` means "at the limit". Ten simultaneous sign-ins at 2 of 3 make exactly one device (tested).
* **At the limit** (after a correct password only): the 403 page says *"You already use 3 devices. To sign in here, remove one from Settings → My devices on a device where you are signed in, or ask the person who runs this server."* The audit row `signin.device_limit` feeds a warning (`DEVICE_LIMIT_HIT`).
* **D16, the self-service exit (recommended).** The same page lists the person's active devices (label, kind, last seen) with a button each, **"Sign this one out and continue"**. A hidden **ticket** proves the password was just right: `base64url(user_id | expiry | nonce | HMAC(STUMP_EDGE_KEY, "tkt|" + user_id + "|" + expiry + "|" + nonce))`, valid 5 minutes, single use (the nonce is remembered in memory until it expires), never containing the password. `POST /api/_i/replace` checks the ticket, same site, that the chosen device belongs to that person and is theirs, revokes it (audit `device.replaced`), then completes the sign-in of §2.1 step 10 (eligibility and the lock state are checked again, and the limit can still refuse if somebody else raced). The password is not in the page at any time.
* **Labels** come from the browser's `User-Agent`, reduced to "Chrome on Android", "Safari on iOS", "Firefox on Windows" (a tiny table; fallback "Browser"); a reading-app key is named by the person ("Kobo"). People can rename (40 characters, plain text). The device id is `<slug>-<5 random base32>` as today, so two "Chrome on Android" never collide.
* **Self-service** (`myDevices`, `renameMyDevice`, `removeMyDevice`; §5): each person sees **only their own** devices (the owner's legacy passes with no person are invisible to everybody but the owner). Removing the *current* device also ends the current session (the request's session id is in the GraphQL context), so the next load shows the sign-in page.

### 2.5 Reading-app keys (OPDS) (D9)

**Format.** `key = <id>.<mac>`, `id` as a device id (`[a-z0-9-]{1,24}`), `mac` the 43-character base64url `HMAC-SHA256(STUMP_EDGE_KEY, "opds|" + id)`, compared in constant time. There is no issued-at: a key lives until it is revoked (or the edge key is rotated, which invalidates every pass and key at once). The URL is `<public base>/opds/<key>/v1.2/catalog` (the public address when the request that made it came through the edge, the request's own address otherwise); the feed builders already put the key into every link they generate (F4), so downloads, covers and page streams stay under the same prefix.

**Creating** (`createReadingApp(label)`, any signed-in non-owner person with *Download files*): the device row is made by the same conditional insert as every device (the limit applies: a fourth device or key is refused with the same message), kind `opds`, person bound, label as typed. The page shows the URL, a QR code (made in the browser from the URL: it never leaves the page) and three lines of instructions ("In KOReader: Search → OPDS catalog → + → paste the address; **leave username and password empty**"; for other readers "add an OPDS catalog"). **Replace** revokes the key and makes a new one in one step; **Remove** revokes it. The URL can be shown again at any time (it is derived), only to that person.

**Verification, twice.**
* *At the edge* (njs, `opdsKey(r)`): the URI must match `^/opds/(<id>)\.(<mac>)/v1\.2/(catalog|search|keep-reading|smart-lists|reading-lists|authors|genres|libraries|series|books)(/|$)` (the real OPDS 1.2 routes, pinned by a test, §9), the MAC must verify, and the id must not be in the revoked set from the state file (so a revoke bites within about 2 seconds, with no reload); until the first good state read every key is refused (fail closed, like passes). A valid key makes `$edge_device` the key's id for that request, which the existing machinery treats as "a valid pass" for that one URI, and which also keys the per-key limits and shows in the log as `dev=<id>`.
* *In the app* (`opds_key_middleware`, which **replaces** `api_key_middleware` on the `/{api_key}/v1.2` nest as a wrapper: it either authenticates a reading-app key itself or calls `api_key_middleware` unchanged; stacking the two would make the second reject the key): if the path segment parses as a reading-app key and its MAC verifies, the app looks the device row up (kind `opds`, not revoked: **a revoked key is refused at once**, the app asks its own database), checks the person (exists, not locked, not the owner; `authorize()` then applies the friend-set rule of §2.7), notes "last seen" (once a minute, the same throttle as passes), and builds the `AuthContext` from the person with **permissions reduced to `DownloadFile` if they hold it** and `is_server_owner = false`; then `authorize()` runs as for every credential. Anything that does not parse as a key falls through to `api_key_middleware` and the ordinary API keys, unchanged.

**Why nothing else can use a key.** The token is not a `PrefixedApiKey` (`stump_<short>_<long>`) and not a JWT, so `handle_bearer_auth` and `validate_api_key` reject it; `auth_middleware` (sessions, Bearer, Basic) never looks at a path segment; the KOReader and Kobo sync routes use `api_key_middleware` alone, which does not know the format; the middleware accepts only `GET` and `HEAD`; and at the edge the key only lets that one URI shape through. A pass is not a key (`dev|…` against `opds|…`) and a key is not a pass.

**Public limits for keys.** Per client the existing `pub_req` (20 r/s) still applies; per key `pub_opds_key` 10 r/s with a burst of 40 (a feed page loads a screen of covers); concurrent: 4 per client, 12 in all; **300 file downloads per key per UTC day** (D17: `/books/<id>/file/…` counts, `Reset budget` resets); `Authorization: Basic` stays 403 for everyone; `/opds/v2.0`, `/opds/v1.2` (no key), the KOReader and Kobo routes stay 403.

### 2.6 Owner warnings (D10)

`accessWarnings` is computed on request from small tables; `accessWarningCount` (WARN and ALERT only) is what the badge polls (every 60 s, owner only, never through the edge).

| Kind | Severity | Source | Fades |
|---|---|---|---|
| `AT_DEVICE_LIMIT` | info | a person's active devices = their limit | when it stops being true |
| `OVER_DEVICE_LIMIT` | warn | active devices > limit (limit lowered, or a dormant device came back) | when it stops being true |
| `DEVICE_LIMIT_HIT` | warn | `signin.device_limit` audit rows (who, how many times) | 3 days |
| `MANY_SESSIONS` | warn | ≥ 4 unexpired rows in `sessions` for a person | when it stops being true |
| `SIGNIN_FAILURES` | warn | ≥ 10 failed sign-in rows for one person in the last hour (marks) | 3 days |
| `NAME_BACKOFF` | warn | `signin.backoff` audit rows (the person if the name exists, else "an unknown name") | 3 days |
| `GLOBAL_BUDGET` | alert | `signin.global_budget` audit row (public sign-in is refusing everybody for a while) | 3 days |
| `NOT_ENABLED_TRIED` | warn | right password on an account that is not enabled (`not_allowed_publicly` mark): "*dani* tried to sign in from the internet; turn it on in People?" | 3 days |
| `NOT_FRIEND_CLASS` | warn | a person with `public_signin` who holds a permission outside the friend set (they are cut off from the internet until it is removed) | when it stops being true |
| `LINK_NEAR_BUDGET` | info | an active link with at most 1 use left | when spent, expired or revoked |
| `LINK_NO_LIBRARIES` | warn | an active link whose preset libraries no longer exist (it cannot be used) | when revoked |

The *Sign-in protection* area also shows live counters from the in-memory tracker (failures in the last 10 minutes, names in backoff: counts only, never names). The warnings list gives each item the action that fixes it (open the person, raise their limit, revoke the link).

### 2.7 Hardening

* **Scan triggers (D11).** `NotViaPublicEdgeGuard` is added to every mutation that calls `core.enqueue(…)`: `scanAllLibraries`, `scanLibrary`, `scanSeries`, `analyzeMedia`, `analyzeSeries`, `analyzeLibrary`, `createLibrary`, `updateLibrary`, `patchLibrary` (each can start a scan), `generateLibraryThumbnails`, `processLibraryThumbnails`, `fetchLibraryMetadata`, `runPostProcessing`, and the upload mutations that scan (implementer: `grep -n "core.enqueue" crates/graphql/src/mutation` is the list of record; the ones that need a Manage permission are unreachable for friends anyway, the guard is the second lock). The coverage test reads `crates/graphql/src/mutation/*.rs`, finds every `async fn` whose body contains `.enqueue(`, and fails unless the `#[graphql(guard …)]` above it names `NotViaPublicEdgeGuard` (a short explicit exceptions list for internal helpers, empty today). Approved uploads still rescan their own book internally: no mutation involved.
* **The user editor.** `associatedPermissions[UploadFile]` becomes `[]` (the table's test is updated and a new one pins that ticking Upload files ticks nothing else).
* **Friend set at request time (D12).** `authorize()` gains one check next to the owner's: for a public request whose person has `access_people.public_signin = true`, an **account** whose permissions are not within the friend set is refused with the same non-cookie-deleting 401 as the owner (the account's own row is read when the credential carries a reduced set, as `credentials_are_the_owners` already does for scoped API keys; so a reading-app key cannot smuggle an out-of-class account through). The lookup (`access_people` by primary key) shares the query that already asks about `must_change_password`, so a request makes no extra round trip.
* **Passwords** (D8) also apply to `updateViewer` when the account has `public_signin`.

### 2.8 Configuration, and the numbers in one place

`EdgeConfig` gains `public_signin: bool` (`STUMP_EDGE_PUBLIC_SIGNIN`, default `false`, environment only like the rest). Everything else below is a constant with a test, not a setting.

| Limit | Value |
|---|---|
| Edge: sign-in POST | 6/min per client (burst 4); 30/min overall (burst 10); sign-in page GET 30/min per client (burst 10) |
| Edge: signup POST | 3/min per client (burst 3); 2/min overall (burst 10) |
| Edge: invite GET/POST (existing) | 6/min per client (burst 4) |
| Edge: OPDS | per key 10 r/s (burst 40); 4 concurrent per client, 12 in all; 300 downloads per key per UTC day |
| App: name backoff | free ×4, then 1, 2, 4, 8, 16, 30 min; 60-minute window; 4096 names |
| App: global failure budget | 40 failures per 10 minutes |
| Devices | 3 per person (1 to 10); dormant after 60 days; sessions capped at 2 × devices |
| Signup link | valid 1 hour to 30 days (default 3 days); budget 1 (single) or 2 to 25 (open, default 5) |
| Names / passwords | 3 to 32 characters / 10 to 72 bytes |
| Replace ticket | 5 minutes, single use |

---

## 3. Edge changes (`deploy/public-edge/`; the owner's deploy steps apply them, not the app)

**`nginx.conf` deltas** against the file in the repo (which is what runs live):

1. **The deny map takes the method.** Its key becomes `"$edge_device:$edge_revoked:$request_method:$uri"` (the existing patterns, for a valid pass, `/api/_i/`, the manifest and the favicons, are adjusted to skip the method field) and it gains the line `"~^[a-z0-9-]*:[01]:(GET|HEAD):/$" 0;` (the sign-in front door). A pass-less `POST /` is still denied.
2. **`location = /`**: `if ($edge_device = "") { rewrite ^ /api/_i/signin last; }` then the usual `proxy_pass` for people with a pass.
3. **`location = /api/_i/signin`** (exact, so it beats the `^~ /api/_i/` prefix): its own `add_header` block (HSTS only, like `/api/_i/`, so no pass renewal is added), `limit_req zone=pub_signin_page burst=10 nodelay` (GET), `limit_req zone=pub_signin_post burst=4 nodelay` and `pub_signin_all burst=10 nodelay` (POST only, through a method-keyed map so a GET is never counted against the POST zones), `client_max_body_size 4k`, `limit_req_log_level info; error_log /dev/stderr crit;`.
4. **`location = /api/_i/join`** and **`/api/_i/replace`**: POST only, `pub_signup` (3/min, burst 3) and `pub_signup_all` (2/min, burst 10), body 4k, same logging rule. The `^~ /api/_i/` location (tokens, GET and the paste form) is unchanged.
5. **The OPDS key location, placed before the generic `^/(opds|koreader|kobo)` 403**: `location ~ "^/opds/[a-z0-9-]{1,24}\.[A-Za-z0-9_-]{43}/v1\.2/(catalog|search|keep-reading|smart-lists|reading-lists|authors|genres|libraries|series|books)(/|$)"` with `if ($edge_device = "") { return 404; }`, `if ($request_method !~ ^(GET|HEAD)$) { return 405; }`, `if ($edge_opds_dl = 0) { return 429; }`, `limit_req zone=pub_req burst=300 nodelay` (a location's own `limit_req` replaces the server-level one, so it is repeated) and `limit_req zone=pub_opds_key burst=40 nodelay`, `limit_conn pub_opds_conn 4` and `pub_opds_all 12`, `limit_req_log_level info; error_log /dev/stderr crit;`, `add_header Referrer-Policy no-referrer always; add_header X-Robots-Tag noindex always;`, `proxy_pass http://stump` (no buffering, long reads for downloads as the main location). The `if ($http_authorization ~* "^\s*basic") { return 403; }` stays above it at server level.
6. **Logs**: `$log_uri` gains `"~^/opds/[^/]+/(?<rest>.*)$" "/opds/-key-/$rest"`; the log line keeps `dev=$edge_device` (the key's *id*, useless without the MAC).
7. **New variable and zones**: `js_set $edge_opds_dl gate.opds_dl;` and the zones `pub_signin_page` (30r/m), `pub_signin_post` (6r/m, keyed `$signin_post_key`), `pub_signin_all` (30r/m, keyed `$signin_post_all`), `pub_signup` (3r/m), `pub_signup_all` (2r/m on `$server_port`), `pub_opds_key` (10r/s on `$edge_device`), `pub_opds_conn`, `pub_opds_all`.
8. **The 404 body** for a pass-less request that is not the front door says "Private site. [Sign in]" with a link to `/` (the invite-paste form moves into the sign-in page).

**`stump-gate.js` deltas** (a copy of the reviewed file plus ~25 lines): `opdsKey(r)` (regex above on `r.uri`, MAC check through the second-HMAC trick, `state()` ready and id not revoked); `device(r)` returns the cookie's device, else the OPDS key id for an OPDS URI, else `''`; `opds_dl(r)` (counts only `/books/<id>/file/` URIs in the shared dictionary under `dl:<id>:<utc-day>:<budget epoch>`, `'0'` past 300); `pass_header` and `signin` are untouched (a key never becomes `X-Edge-Pass` and never counts as a sign-in).

**`vectors.json`** gains a `reading_key` (id, value from `openssl dgst -sha256 -hmac`) and a forged one; the Rust test and the edge script both read it.

**`verify-edge.sh`** (new checks, in the same real-nginx-against-a-stub style): pass-less `GET /` returns the stub's page (rewrite works) while `GET /x` and `POST /` are 404; `/api/_i/signin` reachable without a pass, the 7th POST in a minute 429, GET never counted; the signup and replace routes limited; an OPDS URI with a valid key reaches the stub **without a cookie**, a forged or truncated key is 404, a key added to the revoked state is 404 **within 3 s with no reload** and back when removed; before the first state read every key is refused; the same key on `/opds/<key>/v2.0/…`, `/koreader/<key>/…`, `/api/graphql` and a traversal (`/opds/<key>/v1.2/../../api/graphql`, `%2e%2e`) is not served; `POST` on a key URI is 405; `Authorization: Basic` is 403; the 301st download in a day is 429 and a budget reset restores it; no key and no invite token appears in the access or the error log; client-supplied `X-Edge-Pass`/`X-Edge-Public` still never reach the stub.

---

## 4. Data model

One migration, `m20261115_000000_friend_onboarding` (provisional name: the integrator renames it so the order in `Migrator` matches deploy order; once deployed the name is frozen; never two migrations in one deploy).
SQLite, in the style of `m20261113_000000_public_access`; its `down()` and the manual-down SQL are tested, and the test runs **the SQL file itself** so they cannot drift.

```sql
CREATE TABLE IF NOT EXISTS access_signup_links (
    id          TEXT PRIMARY KEY,                       -- uuid
    token_hash  TEXT NOT NULL UNIQUE,                   -- sha256 hex; the token is never stored (creation also checks access_invites.token_hash)
    label       TEXT NOT NULL,                          -- the owner's note ("book club"), <= 60 characters, plain text
    kind        TEXT NOT NULL,                          -- 'single' | 'open'
    created_at  TEXT NOT NULL,
    created_by  TEXT NOT NULL,                          -- the owner's username at the time
    expires_at  TEXT NOT NULL,
    max_uses    INTEGER NOT NULL,                       -- the budget: accounts this link may create (single: 1; open: 2..25)
    uses        INTEGER NOT NULL DEFAULT 0,
    revoked_at  TEXT,
    preset      TEXT NOT NULL                           -- JSON of §2.2, validated at creation and at redemption
);
CREATE TABLE IF NOT EXISTS access_people (
    user_id              TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE ON UPDATE CASCADE,
    public_signin        BOOLEAN NOT NULL DEFAULT 0,    -- may sign in through the public address without a pass (D3)
    max_devices          INTEGER NOT NULL DEFAULT 3,    -- 1..10
    new_libraries_hidden BOOLEAN NOT NULL DEFAULT 1,    -- libraries made later are hidden from this person, decided once each (D7)
    signup_link_id       TEXT REFERENCES access_signup_links(id) ON DELETE SET NULL ON UPDATE CASCADE,
    created_at           TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS access_people_link_idx ON access_people (signup_link_id);
CREATE TABLE IF NOT EXISTS access_library_decisions (
    user_id    TEXT NOT NULL REFERENCES users(id)     ON DELETE CASCADE ON UPDATE CASCADE,
    library_id TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE ON UPDATE CASCADE,
    PRIMARY KEY (user_id, library_id)
);
ALTER TABLE access_devices ADD COLUMN kind TEXT NOT NULL DEFAULT 'browser';    -- 'browser' | 'opds'
```

* **Why a satellite table and not columns on `users`** (the lead's earlier "tables only" rule): a person's public-access settings are not part of Stump's account model, rolling back is `DROP TABLE`, and an old image never reads a column it does not know. A person with **no row** has public sign-in **off** and the defaults above (so Dani, and `book-curator`, are untouched).
* **Existing `access_devices` rows** become `kind = 'browser'` through the column default. Reading-app keys are rows of the same table, so the revoke, restore, budget reset, list, state file and pruning all work for them with no second mechanism. `access_devices.invite_id` is NULL for both new kinds (a device made at sign-in or signup has no device link).
* **Retention** (the housekeeping that exists): signup links 30 days after they expire, are used up or revoked (`access_people.signup_link_id` goes NULL); browser devices not seen for 200 days are deleted (their pass expired at 180); reading-app keys are never pruned unless revoked (then as revoked devices: 200 days).
* **Manual down** (`crates/migrations/manual-down/m20261115_000000_friend_onboarding.sql`, a copy in `research/2026-11-15-friend-onboarding-manual-down.sql`):

  ```sql
  BEGIN;
  DELETE FROM access_devices WHERE kind = 'opds';          -- reading-app keys are meaningless without the column
  ALTER TABLE access_devices DROP COLUMN kind;             -- host sqlite3 is 3.45; DROP COLUMN needs 3.35
  DROP TABLE IF EXISTS access_library_decisions;
  DROP TABLE IF EXISTS access_people;
  DROP TABLE IF EXISTS access_signup_links;
  DELETE FROM seaql_migrations WHERE version = 'm20261115_000000_friend_onboarding';
  COMMIT;
  ```

  What survives a rollback on purpose: the accounts the links made (with their permissions and library exclusions, so they stay Biblioteca-only), and their browser devices. What is lost: the links, the per-person switches and limits, the decision log, the reading-app keys. An older image refuses to start on a database that records a migration it does not know, so rolling the **image** back needs this file (or a snapshot) first. Switching the feature off needs no SQL (`STUMP_EDGE_PUBLIC_SIGNIN=false`).

---

## 5. GraphQL and HTML surface

**HTML routes (all under `/api/_i/`, public marker required, 404 otherwise):** `GET|POST /api/_i/signin`, `POST /api/_i/join`, `POST /api/_i/replace` (D16); the device-link routes as today; `GET /api/_i/<token>` now also answers a signup link. Pages: `Cache-Control: no-store`, `Referrer-Policy: no-referrer`, `X-Robots-Tag: noindex`, `X-Frame-Options: DENY`, CSP `default-src 'none'; style-src 'unsafe-inline'; form-action 'self'`, text HTML-escaped.

**Owner API** (`ServerOwnerGuard.and(NotViaPublicEdgeGuard)`, mutations also `SameOriginGuard`; list fields priced with `pagination_cost`, no unpaginated list added):

```graphql
enum SignupLinkKind { SINGLE OPEN }
enum SignupLinkStatus { ACTIVE USED_UP EXPIRED REVOKED }
enum AccessDeviceKind { BROWSER OPDS }
type SignupPreset { permissions: [UserPermission!]!  libraries: [PresetLibrary!]!  ageRestriction: PresetAge  maxDevices: Int! }
type AccessSignupLink { id: ID!  label: String!  kind: SignupLinkKind!  createdAt: DateTime!  createdBy: String!  expiresAt: DateTime!
                        maxUses: Int!  uses: Int!  status: SignupLinkStatus!  preset: SignupPreset!  people: [AccessPerson!]! }
type CreatedSignupLink { link: AccessSignupLink!  url: String! }               # the url exists only in this response
input SignupPresetInput { permissions: [UserPermission!]!  libraryIds: [ID!]!  ageRestriction: AgeRestrictionInput  maxDevices: Int! = 3 }
input CreateSignupLinkInput { label: String!  kind: SignupLinkKind!  validForHours: Int! = 72  maxUses: Int! = 1  preset: SignupPresetInput! }
input PersonAccessInput { personId: ID!  publicSignin: Boolean  maxDevices: Int  newLibrariesHidden: Boolean  onlyLibraryIds: [ID!] }
enum AccessWarningKind { AT_DEVICE_LIMIT OVER_DEVICE_LIMIT DEVICE_LIMIT_HIT MANY_SESSIONS SIGNIN_FAILURES NAME_BACKOFF GLOBAL_BUDGET
                         NOT_ENABLED_TRIED NOT_FRIEND_CLASS LINK_NEAR_BUDGET LINK_NO_LIBRARIES }
enum AccessWarningSeverity { INFO WARN ALERT }
type AccessWarning { id: ID!  kind: AccessWarningKind!  severity: AccessWarningSeverity!  person: AccessPerson  link: AccessSignupLink
                     count: Int!  since: DateTime!  detail: String }

extend type Query {
  signupPresetDefaults: SignupPreset!                       # what the form pre-fills: the friend preset, the library named Biblioteca, 3 devices
  accessSignupLinks(pagination: Pagination! = …): PaginatedAccessSignupLinkResponse!
  accessWarnings: [AccessWarning!]!
  accessWarningCount: Int!                                  # WARN and ALERT only: the badge
}
extend type Mutation {
  createAccessSignupLink(input: CreateSignupLinkInput!): CreatedSignupLink!
  revokeAccessSignupLink(id: ID!, disableAccounts: Boolean! = false): Int!    # accounts turned off (their devices revoked, sessions ended)
  setPersonAccess(input: PersonAccessInput!): AccessPersonRow!                   # refuses the owner; refuses publicSignin for a person outside the friend set, naming the permissions
}
```

Existing owner types grow: `AccessPersonRow` (the People row; + `publicSignin`, `maxDevices`, `activeDevices`, `sessions`, `newLibrariesHidden`, `signupLink`, `beyondPreset: [UserPermission!]!`, `hiddenLibraries`), `AccessOverview` (+ `publicSignin`, `signinStats { failuresLast10Min namesInBackoff globalBudgetTripped }`), `AccessDevice` (+ `kind`), `AccessSignInReason` (+ `DEVICE_LIMIT`, `NOT_ALLOWED_PUBLICLY`).
Every mutation writes an audit row (`signup.link_created`, `signup.link_revoked`, `signup.created`, `person.access_set`, `signin.device_limit`, `signin.backoff`, `signin.global_budget`, `device.replaced`, `device.removed_by_person`, `opds.key_created`, `opds.key_removed`, `libraries.hidden_new`); the audit never holds a token, a link, a password or typed text.

**Friend API** (any signed-in non-owner user; **not** owner-only and **not** refused through the edge, because people use it from the internet; mutations `SameOriginGuard`; every resolver scopes by `ctx.user.id`, never by an id argument alone):

```graphql
type MyDevice { id: ID!  label: String!  kind: AccessDeviceKind!  current: Boolean!  createdAt: DateTime!  lastSeenAt: DateTime  lastIp: String! }
type MyDevices { devices: [MyDevice!]!  active: Int!  limit: Int! }
type MyReadingApp { id: ID!  label: String!  url: String!  createdAt: DateTime!  lastSeenAt: DateTime }
extend type Query    { myDevices: MyDevices!   myReadingApps: [MyReadingApp!]! }
extend type Mutation {
  renameMyDevice(id: ID!, label: String!): MyDevice!
  removeMyDevice(id: ID!): Boolean!                      # ends the current session too when it is the current device
  createReadingApp(label: String!): MyReadingApp!        # needs Download files; the device limit applies
  replaceReadingApp(id: ID!): MyReadingApp!
  removeReadingApp(id: ID!): Boolean!
}
```

`MyReadingApp.url` is derived for the viewer only (the public base if the request came through the edge, the request's own address otherwise). Without an edge key the mutation says reading apps are not set up on this server.

---

## 6. Screens

**Owner, Settings → Public access** (existing page, same components and mobile cards; new or changed pieces in bold):

1. **Warnings strip** (only when there is something): one card per warning, a severity icon, the sentence, and the action that fixes it (*Open person*, *Raise limit*, *Revoke link*, *Turn on*). Info items are collapsed under "n notes".
2. **Status strip** (existing) + *Public sign-in: on / off* (the switch is the environment's, the strip says where to change it) and the live counters (failed attempts in the last 10 minutes, names waiting, "refusing everybody for a few minutes" if the global budget tripped).
3. **Sign-up links** (new, first because it is the main action). *Create a link*: **Note** · **One person / Open link** · **Valid for** (1 day, **3 days**, 7, 14, 30) · **Accounts allowed** (fixed at 1 for one person; 2 to 25 for open, default 5) · **What they get** (a card that always shows the preset in words: "Can: change their password, change their avatar, download books, add books for your approval. Sees: Biblioteca only; libraries you add later stay hidden. Up to 3 devices."; *Change…* opens the editor: the seven friend-set permissions as checkboxes, the other permissions are not offered; a library checklist with Biblioteca pre-ticked; an age limit; devices 1 to 10) → *Create link*. The link then appears **once** in a read-only box with **Copy**, **Share** (the system share sheet where the browser has one) and a QR code, "shown only now"; an open link with *Add books* ticked shows the warning of §2.2. Table: Note · Type · Expires · Accounts used (2 / 5) · Status · who signed up (usernames) · **Revoke** (a dialog with "also turn off the accounts this link made").
4. **Device links for existing accounts** (the old *Invite links*, renamed, unchanged otherwise).
5. **Devices** (existing): a *Browser* or *Reading app* badge, and per person *devices 2 / 3*.
6. **Sign-in protection** (existing): the new reasons (*device limit*, *right password, not allowed here*), and what each means.
7. **People** (existing): columns **Public sign-in** (a switch), **Devices 2 / 3** (a stepper), **Sessions**, **Libraries** (a summary: "Biblioteca" / "3 hidden"), and **Beyond the preset** chips (the permissions that keep an account out of public sign-in, e.g. *Scan library*, *Upload files* for Dani) with the explanation. Turning the switch on opens a dialog: what the person will be able to do from the internet, the chips to remove first (it refuses until they are gone), **Libraries: leave as they are / only Biblioteca**, and **Set a new password now** (opens the existing generator dialog; recommended for accounts that existed before). *Make a link* for a new person lives in section 3.
8. **Activity** (existing) with the new actions.
9. **Sidebar**: a small amber badge with the count on the Settings button (next to the approvals one; owner only, polled every 60 s), and the same count beside *Public access* in the settings navigation.

**Friends, Settings → Personal** (two new entries, shown to non-owner users only; phone-first cards):

* **My devices**: a card per device: its name (tap to rename), *This device* badge, kind, last seen and last address, **Remove** (the current device says "Remove and sign out"). A footer: "You can use up to 3 devices; reading apps count as devices; installing Stump on an iPhone's home screen counts as another device." At the limit the page says so.
* **Reading apps**: "Connect KOReader, Kobo or another reading app." *Add a reading app* (a name) → a card with the address (Copy), the QR code, the three-line instructions and the *leave username and password empty* note; each existing app: last used, **Replace address**, **Remove**. Disabled with the reason when the person lacks *Download files* or the edge key is not set.

**The public pages** (server-rendered, one column, ≥ 16 px inputs so iOS does not zoom, light and dark, en and es by `Accept-Language`): *Sign in* (username, password, the paste box, a "forgot?" line); *Create your account* (username, password, repeat, the rules in a sentence, "Already have an account? Sign in"); *This link doesn't work*; *You already use 3 devices* (and the list with buttons when D16 is on); *Too many attempts* (429). No JavaScript anywhere, so no show/hide password; the web app's password manager support relies on `autocomplete` attributes.

All new strings are in `en-US.json` and `es-ES.json` (web: `settingsScene.server/access.*` and `settingsScene.app/devices.*`; the HTML pages: the `Words` tables in Rust), with the existing key-parity test extended.

---

## 7. Files to add and change

**Rust**

| Path | Change |
|---|---|
| `crates/migrations/src/m20261115_000000_friend_onboarding.rs` (+ `lib.rs`), `manual-down/…sql` | §4, with up/down and manual-down tests |
| `crates/models/src/entity/{access_signup_link,access_person,access_library_decision}.rs` (+ `mod.rs`), `access_device.rs` (`kind`) | entities |
| `crates/tests/src/db.rs` | the new tables for the test database |
| `crates/models/src/services/signup.rs` | links (create, status, revoke, disable accounts), `redeem_signup_link` (§2.2), presets and validation, `apply_library_preset`, `on_library_created`, `reconcile_libraries` |
| `crates/models/src/services/access.rs` | the device kind, `insert_device_within_limit`, `active_devices`, person settings (`set_person_access`), the warnings query, retention additions, `friend_set` helpers |
| `core/src/access/policy.rs` (new) | username and password rules and the common list, the friend set, device labels from a `User-Agent`, limit constants |
| `core/src/access/signin.rs` (new) | the bounded in-memory tracker (name backoff, global budget, counters) |
| `core/src/access/pass.rs` | reading-app key mint and verify (`opds\|`), the replace ticket (`tkt\|`), vectors |
| `core/src/config/edge_config.rs` | `public_signin` |
| `apps/server/src/routers/public_pages/{mod,words,signin,signup}.rs` (new), `routers/invite.rs`, `routers/mod.rs` | the pages and handlers of §2.1 to §2.4; the device-link pages reuse the shared page chrome |
| `apps/server/src/middleware/{edge.rs,auth.rs,mod.rs}` | `is_open_public_path` (+ the OPDS key shape), `opds_key_middleware`, `authorize()` friend-set check |
| `apps/server/src/routers/opds/v1_2.rs` | the middleware order on the `/{api_key}/v1.2` nest |
| `apps/server/src/routers/api/v2/auth.rs` | `login()` calls the shared `attempt` for non-bound requests (D14); session creation moved into one function used by both |
| `apps/server/src/http_server.rs`, `core/src/access/state.rs` | trace-span redaction of `/opds/<key>/…` (next to the `/api/_i/` one); the 10-minute reconcile task, spawned at start-up independently of the edge settings |
| `crates/graphql/src/{object,query,mutation}/access.rs`, `…/my_access.rs` (new), the `mod.rs` files | §5 |
| `crates/graphql/src/mutation/{library,series,media,post_processing,upload}.rs` | `NotViaPublicEdgeGuard` on every enqueuing mutation; `create_library` calls `on_library_created` |
| `crates/graphql/src/mutation/user.rs` | the password rules in `updateViewer` for `public_signin` accounts |
| `crates/graphql/schema.graphql`, `packages/graphql/src/client/**` | regenerated, never edited by hand |

**Web** (`packages/browser/src/…`): `scenes/settings/server/access/{SignupLinkSection,WarningsStrip}.tsx` and changes to `{PeopleSection,DeviceSection,SignInSection,StatusStrip,AccessScene}.tsx`, `graphql.ts`, `copy.ts`; `scenes/settings/app/devices/{MyDevicesScene,ReadingAppsScene,index}.tsx`; `scenes/settings/{routes.ts,SettingsRouter.tsx}` and `hooks/useRouteGroups.ts` (a `nonOwnerOnly` flag, like `ownerOnly`); `components/navigation/sidebar/{Settings.tsx,AccessWarningsBadge.tsx}`; `scenes/settings/server/users/create-or-update/UserPermissionsTable.tsx` (+ its test); `packages/i18n/src/locales/{en-US,es-ES}.json`; one small lazily-loaded dependency for the QR code (`qrcode.react`, only in the Reading apps and link-created chunks).

**Edge and deploy** (`deploy/public-edge/`): `nginx.conf`, `stump-gate.js`, `vectors.json`, `verify-edge.sh`, `DEPLOY.md` (a new section, §10), `compose-snippet.yaml` and `apply-compose.py` (the new variable). Plus a `FORK.md` row.

---

## 8. Security review of this design

| Threat | Answer |
|---|---|
| **Guessing passwords at a sign-in page anybody can reach** | edge 6/min per client (IPv6 by /64) and 30/min overall; the app's per-name backoff from the 5th failure (up to 30 minutes) and a global 40-per-10-minutes budget; passwords of at least 10 characters, not common, not containing the name; bcrypt at the configured cost; the owner is warned. A slow distributed guess is bounded to about 100 tries a day per name |
| **Locking a friend out on purpose** | pass-less failures never count towards `is_locked` (D4); the backoff only delays *new-device* sign-ins; known devices (a bound pass) are outside the name gate |
| **Username enumeration** | unknown, wrong, ineligible, owner, admin, not enabled, locked and OIDC-only all give the same 401 page after one bcrypt; the backoff exists for every typed name; the device-limit page appears only after a correct password; "name taken" appears only to the holder of a valid signup link |
| **Reaching the owner or an admin through the page** | refused identically (D3) and recorded, never counting towards a lock; `authorize()` refuses owner credentials on every public request and, for `public_signin` accounts, anything outside the friend set (D12) |
| **Escalating through a signup link** | permissions validated against the friend set at creation and again at redemption; the link API is owner-only, not via the edge, and same-origin; the preset's libraries are checked at redemption |
| **Abusing an open link** | budget, expiry, 3/min per client at the edge, the owner's warning near the budget, one-click revoke that can also turn off the accounts it made; each account sees Biblioteca only and uploads wait for approval |
| **Race on the budget or the device limit** | one conditional `UPDATE` / `INSERT … SELECT … WHERE count < limit`, rows checked; tests fire 10 parallel requests at each |
| **A leaked reading-app URL** | read-only (GET/HEAD), only *Download files*, only the person's visible books, only the OPDS 1.2 routes (pinned), 10 r/s and 300 downloads a day per key, instant revocation (app at once, edge in about 2 s), visible to the person with last use and address, replaceable in one step; **a database leak reveals nothing** (derived, not stored); the key is in no log |
| **Using a key or a pass for the wrong thing** | domain-separated MACs (`opds\|` against `dev\|`); the key is not an API key or a JWT; only `opds_key_middleware` understands it; KOReader/Kobo routes, GraphQL, REST and Basic do not |
| **Forged or random keys flooding the app** | the edge rejects a bad MAC in njs with 404 before the app is reached; the app verifies the MAC before it touches the database |
| **Library leakage to new libraries** | the decision log applied in a hook, at start, every 10 minutes and at each public sign-in; the owner's manual choices are never overwritten (tested both ways) |
| **Cross-site posts** | every POST of the pages checks `Sec-Fetch-Site`/`Origin`; owner and friend mutations carry `SameOriginGuard`; GraphQL takes JSON only at the edge |
| **XSS and clickjacking on the pages** | all text escaped, no scripts, CSP with `default-src 'none'`, `X-Frame-Options: DENY`, `no-referrer`, `no-store` |
| **Tokens and secrets in logs** | edge `$log_uri` and `error_log crit` for `/api/_i/` and `/opds/<key>/`; trace spans redact both; the POST carries the token in the body; the audit never holds tokens, links, passwords or typed text; the state file holds ids only |
| **CPU or memory exhaustion on the new paths** | bcrypt only after the cheap gates; fields capped before hashing; hashing outside the transaction; edge limits and 4 KB bodies; the tracker is bounded (4096 names) |
| **Session fixation, login CSRF** | the session is made by the same function and layer as `login()`; same-site checks on the POST |
| **Open redirect, SSRF** | there is no `next` or URL parameter anywhere; every redirect is to `/` |
| **Funnel pointed at the home listener by mistake** | unchanged: the home listener answers 404 to anything carrying `Tailscale-Funnel-Request` |
| **Weak or reused friend passwords** | the policy and common list for invite-made accounts and for later self-changes of public accounts; the 3-device limit caps what a stolen password can do; the owner sees new devices and failures; the friend sees their own device list. Residual: a good-looking password reused from a breach cannot be detected offline |
| **Scans and other jobs from the internet** | every enqueuing mutation refuses public requests, enforced by a coverage test |

---

## 9. Tests

**Rust unit / service tests**

* `core::access::policy`: username rules (length, characters, lowercase, reserved words, a name that differs only in case, a unicode lookalike refused); password rules (9 vs 10 characters, 72 vs 73 bytes, 4 vs 5 distinct characters, the common list in English and Spanish, containing the username); device labels from a table of real `User-Agent` strings; the friend set (each permission in and out).
* `core::access::pass`: reading-key mint/verify against `vectors.json`, tamper cases (id, mac, length, charset), domain separation (a pass is not a key and the reverse), the replace ticket (expiry, tamper, single use).
* `core::access::signin` (tracker): the schedule (4 free, then 1, 2, 4, 8, 16, 30 minutes), a success resets, the window ends, the same behaviour for a name that does not exist, the 4096 bound and idle expiry, the global budget (trips at 40, recovers as the window slides), counters expose no names.
* `models::services::signup`: create/expire/revoke; the atomic spend (**10 parallel redemptions of a 3-use link create exactly 3 accounts**); a name collision rolls back the use; the preset is applied exactly (permissions, exclusions, decisions, age restriction, `max_sessions_allowed`, the access row, the first device); a preset with a Manage permission, an unknown library or `maxDevices = 11` is refused at creation; libraries gone since creation (some: the rest are used; none: invalid, nothing spent); `revoke` with and without `disableAccounts`.
* `models::services::access`: `insert_device_within_limit` (at limit, revoked and dormant devices not counted, 10 parallel inserts at 2 of 3 give one), the OPDS kind counts, `unblock` and the existing tests stay green, retention boundaries for the new rules, the warnings query (each kind appears, fades, and the count excludes info).
* Libraries: signup hides all but the preset; a library made through `createLibrary` is hidden for people with the policy and not for others; a library inserted by SQL is hidden by `reconcile`; a manual un-hide survives a reconcile; a deleted library or user cascades; enabling public sign-in for an existing person decides current libraries without hiding them, unless `onlyLibraryIds`.
* `migrations`: up creates the tables and the column (existing devices read `browser`); down and the manual-down SQL give the same schema, on a database holding old rows.
* Guard coverage: every `async fn` under `crates/graphql/src/mutation` that contains `.enqueue(` carries `NotViaPublicEdgeGuard` (fails with the function's name otherwise).
* Cross-check: the OPDS route prefixes of `routers/opds/v1_2.rs` all appear in the edge regex of `nginx.conf`, and the regex lists nothing else (the file is read with `include_str!`).

**Server integration (`apps/server/tests/access/{signup,public_signin,devices,opds_keys,libraries,warnings,friend_set}.rs`)**

* **Signup page:** GET spends nothing and sets nothing, also with a link-preview agent; valid → form; every invalid state → byte-identical "doesn't work" page; POST needs the marker, the switch, a same-site request; wrong passwords/names re-render with the message; success → account, device, session, both cookies, 303; the token is in no trace output and no response other than the hidden field.
* **Public sign-in:** a friend signs in with no pass → device row, pass cookie `Secure; HttpOnly; SameSite=Lax`, session; the owner, an admin, a person with a Manage permission, a person not enabled, a locked and an OIDC-only account all get the **same status and body** as a wrong password; **100 failures never set `is_locked`**; the 5th failure starts the backoff and an unknown name behaves identically; a success resets; the global budget trips and recovers; at the limit only a correct password gets the limit page and the audit row; the ticket flow (D16): replace works once, a used or expired ticket and another person's device are refused; a bound device failing nine times then once still locks (existing behaviour); another person's bound device is treated as pass-less (D14); a switch off answers 404 everywhere; Spanish pages.
* **Friend set at request time:** a `public_signin` account given `ManageUsers` is refused (401, no cookie deleted) on a public request and still works on a home request; the owner is still refused; accounts with no access row are unchanged.
* **Devices (friend API):** A cannot list, rename or remove B's devices (ids guessed), the owner's legacy devices are invisible, removing the current device ends the session, the owner can still revoke from Public access.
* **OPDS keys:** a valid key gets a feed whose links all stay under `/opds/<key>/v1.2/`; downloads need *Download files*; visibility holds (`VisibilityFixture`: excluded library, age restriction, removed book); `POST` is refused; the same key on KOReader and Kobo routes, `/api/graphql`, REST and as a Bearer is refused; a forged or other-kind (pass) token is refused before the database is touched; revoked → refused at once; a locked, deleted or owner person refused; a person who must change password refused; the daily cap lives at the edge (tested there).
* **Scan hardening and permissions table:** each enqueuing mutation refuses a public request for a person who would otherwise be allowed (table-driven); the web table test: ticking Upload files ticks nothing else.
* All existing access, intake, graphql and opds suites stay green.

**Web (vitest + `tsc -b`):** the link section (the URL appears once and is kept in component state only: not in storage or the address bar; Copy and Share paths, the preset editor offers only friend-set permissions, an open link with *Add books* shows the warning); People (the switch refuses with the chips, the stepper, "only Biblioteca"); the warnings strip and the badge (count excludes info; owner only); My devices (rename, remove, the current device's wording); Reading apps (the QR encodes the URL, Copy, the disabled states); `useRouteGroups` for `nonOwnerOnly`; en/es key parity.

**Edge:** `verify-edge.sh` as listed in §3, run by the implementer and again at deploy.

**Manual, at deploy** (§10, step 7): with a throwaway link and account.

---

## 10. Deploy and rollback (outline; the implementer writes exact commands into `deploy/public-edge/DEPLOY.md`, section "Self-signup links and public sign-in")

Stump's Funnel is **on** now (`:8443`), so every step keeps the edge fail-closed, and the order below keeps each intermediate state safe: **app first with the switch off, then the edge, then the switch**.

1. Build on `l5`, load on `g3`. Pre-flight: no jobs or scans running; **snapshot** `config.backup-<UTC>-db` (the migration adds tables and a column) with the manual-down SQL beside it and `Stump.toml`; compose backup; backup `edge/`.
2. Compose: add `STUMP_EDGE_PUBLIC_SIGNIN=false` to `stump-fork`; set the shared image line (`x-stump-image`); comment with branch, date, migration, rollback image. `docker compose config -q && docker compose up -d --no-build stump-fork stump-intake-probe`; check the migration applied, `docker logs` clean, the checker's selftest lines and `ready`, LAN and tailnet 200, the public listener still 404 without a pass, **and `GET /api/_i/signin` is 404** (the switch is off).
3. Stage the new edge files in `edge-new/`; validate with `nginx -t` in a throwaway container with the same mounts; run `verify-edge.sh` against the staged files; then copy over the live ones and `nginx -s reload` (no new mount, so no recreate). Check: pass-less `GET /` is now 404 from the app (switch off), a pass still works, a request with an old invite link still works, home still 200.
4. **Turn the switch on**: `STUMP_EDGE_PUBLIC_SIGNIN=true`, recreate `stump-fork` and `stump-intake-probe` (the checker must be on the same build). Check `GET /` on `127.0.0.1:10803` returns the sign-in page.
5. Dani: the owner removes *Scan library* (and, if wanted, *Upload files*), then in People turns on *Public sign-in* with *only Biblioteca* and a new password. (An owner step, not part of the deploy.)
6. Walk-through with a **throwaway** link (budget 1, 1 day) and account, through `127.0.0.1:10803` (a browser on the host: loopback counts as a secure origin for the `__Host-` cookie; or `curl` with the cookies passed by hand, as the earlier steps do): create the account and check the preset (permissions, Biblioteca only, 3 devices); sign out; sign in with a second profile (a second device), a third, then a fourth: the limit page and the warning and the badge appear; remove one in *My devices*; create a reading-app key and read the feed with `curl` through the public listener (200), then with the key on `/api/graphql` and `POST` (refused), revoke it (404 within 3 s), `Authorization: Basic` (403); six wrong passwords for the account (the backoff message, the owner's warning), a wrong password for a name that does not exist (the same message); `Origin: https://evil.example` on a POST (refused); then **delete the test account (hard), revoke the link and the devices, and check the library has no new rows**.
7. Tell the owner: how to make a link, what the preset gives, how to read the warnings, how to turn the whole feature off.

**Rollback (in this order, no data to restore for the feature itself):** `STUMP_EDGE_PUBLIC_SIGNIN=false` and recreate `stump-fork` (the sign-in page and signup links stop at once; passes, device links, the owner's tools and everything older keep working); if more is wanted, restore `edge/nginx.conf` and `stump-gate.js` from the backup and reload (the OPDS keys stop at the edge); `tailscale funnel --https=8443 off` while working on it. To return the **image** to `stump:fork-93eda9ca8`: stop, run the manual-down SQL (or restore the snapshot), start the old image. The accounts the links made stay as ordinary Biblioteca-only accounts. Two notes for `DEPLOY.md`: restoring a snapshot forgets revocations made since (re-check Devices), and rotating `STUMP_EDGE_KEY` invalidates every pass **and every reading-app key** at once.

---

## 11. Alternatives considered

| Option | Why not |
|---|---|
| Serve the web app's own login (HTML shell, assets, `claim`, GraphQL) without a pass | opens ~10 more kinds of path to strangers, each needing its own limits and a review; the SPA shows a blank error if any one is missing (D1) |
| Keep device links as the only way in; add an "Add a device" link flow | the owner rejected it as clunky; it also keeps the owner in the loop for every new phone |
| Mint the pass at the edge (nginx parses the sign-in body) | login logic in njs, no unit tests, and the app still has to count devices |
| Per-account permanent lock after N failures from the internet | any stranger can lock any friend out (D4); a progressive per-name delay gives the same slowdown without the weapon |
| Per-IP-only limits | a botnet or an IPv6 prefix rotates around them; the per-name backoff and the global budget do not care where an attempt comes from |
| An allowlist of libraries in the visibility code (`library_hidden_to_user_query`) | one place, but 15+ call sites read `library_exclusions` directly and the owner's existing per-library access screen would stop being able to *grant*; the decision log leaves all of that alone (D7) |
| Reading-app keys as rows of `api_keys` with a scope column | `validate_api_key` is also behind Bearer auth and the KOReader/Kobo routes, so scope would have to be checked at three places; a separate format cannot be accepted by any of them (D9) |
| Store reading-app tokens hashed and show them once | a friend who resets their e-reader has to make a new key; derived keys are re-showable and a database leak still reveals nothing |
| Let a signup link turn on an existing account when its password is typed | any link holder could switch an account on for the internet by knowing its password (D13) |
| Warnings with an "acknowledge" state | a third table and more UI; events fade in 3 days and state warnings clear when fixed. Easy to add later |
| Email or push notifications to the owner | out of scope; the badge and the page are what was asked |
| A captcha on signup | needs a third-party script on a page with a strict CSP and an external dependency; the budget, expiry and approval-gated uploads bound the damage |

---

## 12. Effort, and decisions for the lead

| Part | Estimate |
|---|---|
| Migration, entities, signup and access services (limit, decisions, warnings, retention) | 1.5 days |
| `core::access` policy, tracker, reading keys, tickets, config | 0.75 day |
| Public pages (sign-in, signup, limit, replace, 429), `attempt`, `login()` unification, i18n | 2 days |
| Middleware: OPDS keys, `authorize()` friend set, scan guards and the coverage test | 1 day |
| GraphQL owner and friend modules | 1.25 days |
| Edge: nginx, gate, `verify-edge.sh`, vectors, `DEPLOY.md` | 1 day |
| Web: links, people, warnings, badge, My devices, Reading apps, QR, i18n, permissions table | 2.5 days |
| Tests across Rust, web and edge, `FORK.md`, docs | 1 day |
| **Total** | **about 11 days** (±30%); builds and tests on `l5` |

**Cuttable without changing the rest:** D16 (the replace step, 0.5 day), D17 (the download cap, 0.25 day), the QR code and Share button (0.25 day), renaming devices (0.25 day), the live counters in the status strip (0.25 day).

**Decisions taken here (the lead may override), with the cost of the alternative**

* **D1, a server-rendered sign-in page.** The alternative (the web app's login, opened to strangers) is in §11: more surface, +1 day of edge and review.
* **D12, the friend set excludes Scan library (and everything with Manage/Create/Delete/Edit).** Dani must lose Scan library (and Upload files is allowed) before her public sign-in can be turned on; the alternative is a deny-list of Manage permissions, which leaves Scan library, Edit metadata and Remove books open to the internet for any enabled account.
* **D12, the request-time check for `public_signin` accounts.** It adds one primary-key lookup per request (merged with the existing password-change lookup). Without it a Manage permission granted later would only block *new* devices.
* **D14, a bound device used for another person is treated as pass-less.** Keeping `wrong_person` would strand two people sharing a computer.
* **D4, no permanent lock from the internet, memory-only backoff.** A restart clears it; persisting it needs a table and a write per failure (0.5 day) and buys nothing against an attacker who cannot restart the server.
* **D5/D16, the dormancy period (60 days) and the replace step.** The period is a constant; the replace step is optional.
* **D8, password policy also for self-changes of public accounts.** It changes `updateViewer` for those accounts only.
* **D9, derived reading-app keys.** The cost: rotating `STUMP_EDGE_KEY` changes every key (the Reading apps page then shows the new address; e-readers need it pasted again). It was already true for passes.
* **The preset does not include Smart lists** (`AccessSmartList`), as the owner listed. Shared smart lists are shown to "everyone who may use smart lists", so a friend without it will not see them: one tick on the link, or a decision for the owner.
* **QR in the browser with `qrcode.react`** (lazy chunk) rather than a Rust crate: no change to `Cargo.lock`.
* **No acknowledge state for warnings** (§11).

**Findings for the parent, outside this package:** (1) `dani` sees all four libraries and holds Scan library and Upload files today (F3). (2) A web app tab whose pass was revoked mid-session shows errors until reloaded (the edge answers 404 HTML); a one-line handling in the web client's API layer could send it to `/`. (3) In-app browsers of chat apps and an iPhone's home-screen app have their own cookie jars and each counts as a device; the *My devices* page says so. (4) bcrypt's 72-byte limit also affects the owner's generated passwords (20 characters: fine) and anything pasted longer. (5) The device-binding of the earlier design protected links, not accounts; the gates here are what protect the page.
