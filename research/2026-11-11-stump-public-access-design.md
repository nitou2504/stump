# Admin "Public access" for Stump: invites, devices and sign-in protection without a terminal

*Design only, 2026-11-11. Branch `feat/public-access-admin` from `20cd74b41` (the live `stump:fork-20cd74b41`). No
code was written and no live file was touched. What was run: read-only inspections, and one throwaway `nginx:alpine`
container pair (removed), described in §1. The reference is iaboy's owner-approved design
(`iaboy/docs/research/2026-11-10-admin-public-access-design.md`) and its as-built result (`iaboy/deploy/public-edge/`,
live since 2026-11-10); where Stump differs from iaboy, the difference is called out.*

**Owner request.** Routine access admin must not need a terminal. In Stump's server settings, owner only and never
through the public edge, a **Public access** section where the owner can (1) create invite links per person and
device, (2) see devices with a pass and revoke or restore them within seconds, (3) see failed sign-ins, unlock an
account and reset a device's daily budget, (4) set a person's password, and (5) read an audit log of all of it.

---

## 0. Decisions

| # | Decision | Why |
|---|---|---|
| D1 | **The app is the source of truth.** Invites, devices and the audit log are tables in Stump's database. The edge only *verifies* passes and *follows* the app's state. | The owner administers from Stump; one place holds the facts; the app can also enforce them itself. |
| D2 | **The app mints passes.** `stump-fork` gets the edge key (`STUMP_EDGE_KEY`, which it does not have today). The pass format is unchanged: `<id>.<issued>.<mac>`, `mac = base64url(HMAC-SHA256(key, "dev\|id\|issued"))`. The key lives in its own config struct that never prints or serialises it (§2.7). | Same trade as iaboy's D2 (reviewed, accepted): no subrequests, no second secret, all logic in Rust where it is unit-tested. A pass only opens the *public sign-in page*; a process that already runs inside `stump-fork` gains nothing it does not have. |
| D3 | **App → edge: a small state file in a shared *directory***, polled by njs once a second into a shared dictionary. No network control endpoint, no reload. | Nothing new for a client to reach. Verified approach (iaboy's PoC and its as-built `iaboy-gate.js`, reviewed GO). |
| D4 | **Edge → app: two headers on every public request.** `X-Edge-Public: 1` ("this came through the public listener") and `X-Edge-Pass: <cookie value>` (only for a valid, unrevoked pass). The app **re-verifies the MAC** with the key and also requires the marker. Both headers are stripped on the home listener and replaced on the public one (verified, §1). | The app never trusts a bare id. The marker is trustworthy for the reason `X-Real-IP` already is: `stump-fork` publishes no host port, so only the containers on its compose network can reach it (`stump-edge`, and the PDF worker, which never calls Stump), and `stump-edge` overwrites both headers. |
| D5 | **Revocation is enforced twice.** The edge drops a revoked device within about 2 s of the app's write; the app refuses it **at once** (it reads its own database). | "Effective immediately" holds for anything that reaches the app; the edge follows with no reload. |
| D6 | **An invite is for one person.** The device it creates is bound to that Stump user (by user id, not name). Through the public edge, that device can only sign in as that person: any other name is answered like a wrong password, and that attempt is **recorded but does not count towards the other account's lock**. | A leaked link or stolen phone exposes one account's password to guessing, never all of them; and it cannot be used to lock out somebody else (§2.3). |
| D7 | **Tokens are random 128-bit values, stored only as a SHA-256 hash, shown once** with a Copy button. `GET /api/_i/…` never spends or sets anything; only the confirmation `POST` does. Revoking a device also revokes the invite it came from; "Revoke all of <person>" revokes all of that person's invites. | A database read does not leak working links; link previews in chat apps cannot burn uses; a stolen phone cannot return through its own link. |
| D8 | **The sign-in log is Stump's own `user_login_activity`** with three added columns, not a parallel table. | The owner's wording is "from Stump's login activity". Stump records a sign-in only for an account that exists, so a password typed into the username box is **never stored** (the typed text is not kept for unknown names at all). |
| D9 | **"Clear lockout" really clears.** Today `updateUserLockStatus(lock: false)` and `stump account unlock` only flip `is_locked`; the 24-hour failure count stays, so the **next** typo locks the account again (§1, F1). The new unblock also stops the old failures counting. The Users page's unlock uses the same function. | Without this, an unblock button would not work. |
| D10 | **No forced "change at next sign-in" in v1.** Stump has no such state (no column, no guard, no screen). The People section reuses the existing `updateUser` password set and shows a one-time generated password. Options and cost if the owner wants it: §12, D-C. | Building a restricted-session state inside the core auth path is a feature of its own; the friends are trusted adults and can change their own password. |
| D11 | **Everything new is owner-only *and* refused on public requests**: a new `NotViaPublicEdgeGuard` next to `ServerOwnerGuard` on every field; the edge also answers 404 for `/settings/access`. | The owner's session cookie is not port-scoped, so an owner who tests an invite link in their own browser would otherwise reach the page through Funnel. |
| D12 | **Stump gets its own njs file** `stump-gate.js` (a copy of the reviewed `iaboy-gate.js`, comments and names aside). The shared `edge-common/gate.js` is not edited; after the switch `stump-edge` stops importing it. | Lowest-risk reuse of reviewed code; nothing shared is touched. |
| D13 | **Terminal tools stay as break-glass:** `edge-common/invite.sh stump …` (its links are accepted by the app, §2.2) and `edge/revoked.map` (+ reload). | If the admin page cannot be reached, the owner is not locked out of managing access. |

---

## 1. What was verified, read, and found

**Read (Stump):** `apps/server/src/http_server.rs` (layers, `TraceLayer`), `routers/mod.rs` and `routers/spa.rs`
(the SPA fallback answers any unknown path, even under `/api`), `routers/api/graphql.rs` (what the handlers put in the
request data), `routers/api/v2/auth.rs` (`login`, `track_login_attempt`, the `X-Edge-Public` owner refusal),
`middleware/{auth,host}.rs` (`auth_middleware`, `ClientIp`), `config/session/utils.rs`, `core/src/config/{stump_config,
oidc_config}.rs`, `crates/graphql/src/{guard,data,schema,complexity,pagination}.rs`, the `reading_history` module
(current conventions), `crates/graphql/src/{query,mutation}/user.rs`, `crates/models/src/entity/{user,
user_login_activity}.rs`, the migrations (`m20261110…user_sharing_settings` as the "new table" template), the web
settings (`routes.ts`, `SettingsRouter.tsx`, `useRouteGroups.ts`, `UsersScene.tsx`, the login-activity section), the test
harness (`apps/server/tests/common/app.rs`, `tests/auth/*`), and the live edge (`stump-fork/edge/nginx.conf`,
`proxy_common.conf`, `edge-common/gate.js`, the compose service).
**Read (iaboy, as built):** `deploy/public-edge/{iaboy-gate.js,nginx.conf,DEPLOY.md}`, `domain/public_access.py`, and the
lead's review of the design (its eight required changes are already folded into what is reused here).

**Throwaway proofs of concept** (two short runs of `nginx:alpine` with njs plus a tiny echo upstream; everything removed).
They reproduce Stump's include pattern as this design proposes it: `proxy_common.conf` strips `X-Edge-Public` /
`X-Edge-Pass`, and a new `proxy_public.conf` **that itself includes `proxy_common.conf`** sets them again for public
locations. Result, with a client that sends **forged** `X-Edge-Pass` and `X-Edge-Public` and a pass cookie:

| listener | headers the app received |
|---|---|
| home (`proxy_common.conf` only) | neither header; the cookie reduced to `stump_session` only (the pass cookie is **not** forwarded) |
| public, valid pass (nested include) | `X-Edge-Public: 1` and `X-Edge-Pass: <the cookie's value>` (the forged copies replaced), `Host` and `X-Forwarded-Proto` from the include |
| public, no valid pass (the invite path) | `X-Edge-Public: 1` only; the forged `X-Edge-Pass` is **not** passed (an empty value omits the header) |
| home, request carrying `Tailscale-Funnel-Request` | 404 with the guard of §3 item 8; an ordinary request without it is 200 (the header was stood in by hand: the live edge log shows tailscaled sets it on Funnel traffic) |

So "include, then set", also through a nested include, is safe, and the marker/pass cannot be planted from outside. The `js_periodic` + shared-dictionary
mechanism (a state file replaced atomically is followed within about 2.5 s through a directory mount, and fails closed
until the first good read) is the one iaboy's design proved and its real-nginx tests (13 cases) exercise; Stump's edge runs
the same image, so the same `stump-gate.js` logic applies unchanged.

**Findings in today's Stump that shape the design**

| # | Finding | Consequence |
|---|---|---|
| F1 | `track_login_attempt` locks on the 10th failure in 24 h (`FAILED_LOGINS_BEFORE_LOCK = 9`). `updateUserLockStatus(false)` and the CLI's `account unlock` only set `is_locked = false`; the failures stay in the window. | An unblock must also stop those failures counting (D9). Also a real bug for the Users page today. |
| F2 | `X-Edge-Public` exists, presence-only, and **only the login location** sets it; nothing else in the server knows about the edge. | Generalised to every public request (D4). |
| F3 | The session cookie `stump_session` is not `__Host-` and cookies ignore ports, so an owner session from `:10802` is sent to `:8443` as well. | The owner-only guard must also refuse public requests (D11). |
| F4 | The existing `loginActivity` query is owner-only but **unpaginated** and unbounded. | The page gets its own paginated, priced query. |
| F5 | There is no "must change password" state. | D10. |
| F6 | `TestApp::new()` mounts `routers::mount(...)` plus the session layer only, not the layers of `http_server.rs`. | The new middleware must be added inside `routers::mount` so tests exercise it. |
| F7 | `StumpConfig` derives `Debug`, `Serialize` and GraphQL `SimpleObject`; the OIDC secret avoids this with a nested struct and a hand-written redacting `Debug`. | The edge key follows the OIDC pattern (§2.7). |
| F8 | `TraceLayer::new_for_http()` records the request URI (at debug level and below). | `/api/_i/<token>` must be redacted in the span (§5). |
| F9 | Native Stump apps and OPDS cannot hold the pass cookie (OPDS is blocked publicly anyway). | Friends use the web app or the PWA; an iOS Home Screen app has its own cookie jar, so the link is pasted once inside it (the "Private site" page already has the paste form). |

---

## 2. Architecture

```
                       Funnel :8443 ─▶ stump-edge public :8081 (nginx + stump-gate.js) ─▶ stump-fork (app)
 owner, LAN / tailnet ─▶ Settings → Public access                                              │
   (home listener :8080; never public)                                                        │
                                                                                              ▼
   Stump database ──┬─ access_invites   (hashed tokens, expiry, uses, person)
                    ├─ access_devices   (id, person, first/last seen, last IP, revoked, budget epoch)
                    ├─ access_audit     (every admin action; no tokens, no passwords)
                    └─ user_login_activity  (+ device_id, failure_reason, counts_toward_lock)
        │
        │ after every change the edge cares about, at start-up, and every 6 hours
        ▼
   /edge-state/state.json ── written by stump-fork (atomic), read-only in stump-edge ──▶ njs poll, 1 s
        {"v": 1794423791150, "written": "…Z", "revoked": ["ana-phone-k3f9x"], "budget": {"ana-laptop-2m8qd": 1794420000}}
```

**Request path through the edge (unchanged in spirit):** no valid pass → 404 (except the invite pages, the manifest and
the fixed icons). A valid pass is the signed cookie `__Host-g3-stump`. New: the edge also refuses a pass whose id is in
the revoked set, and refuses *all* passes until it has read the state file once (fail closed).

### 2.1 Pass and invite formats

* **Pass cookie (unchanged):** `__Host-g3-stump = <id>.<issued>.<mac>`, 180 days, renewed daily by the edge. `<id>` matches
  `[a-z0-9-]{1,24}`; `issued` is 10 digits of epoch seconds; `mac = base64url-no-padding(HMAC-SHA256(key, "dev|" + id + "|" +
  issued))` (43 characters). **The key is the 64-character hex string used as is**: its UTF-8 bytes are the HMAC key, exactly as
  in `gate.js` and `invite.sh`; it is **not** hex-decoded. Existing passes keep working, which the shared vectors (§9) pin.
* **Device id (new, server generated):** `<slug of the label, up to 17 chars>-<5 random base32 chars>`, e.g.
  `ana-phone-k3f9x`; unique in `access_devices`.
* **Invite token (new):** 22 characters of base64url from 16 random bytes. The link is
  `<STUMP_EDGE_PUBLIC_URL>/api/_i/<token>`. Only `sha256(token)` is stored.
* **Legacy invite (still accepted):** `name.expiry.mac` from `edge-common/invite.sh stump …` (`mac` over
  `"inv|name|expiry"`). It makes a device with **no person** and no use limit, valid until its expiry. It is the
  break-glass path (D13). A device that was revoked is **never brought back** by its old link.
* Invites are refused for the server owner at creation *and* at redemption (the owner never signs in publicly).

### 2.2 Redeeming an invite (served by the app, public only, before any pass)

```
GET  /api/_i/<token>      → confirmation page "Open Stump on this device?" with one button. Spends nothing.
GET  /api/_i/?t=<pasted>  → the same page (the edge's "Private site" form posts here; a whole pasted link works)
POST /api/_i/             → redeem: one atomic UPDATE → new device row → Set-Cookie pass → 303 /
```

* Answered only when the request carries the public marker (`X-Edge-Public`); from the LAN or tailnet the routes are 404.
* The pages are small server-rendered HTML (no SPA assets are reachable without a pass), English and Spanish by
  `Accept-Language`, with `Cache-Control: no-store`, `Referrer-Policy: no-referrer`, `X-Robots-Tag: noindex`, a strict
  CSP (`default-src 'none'; style-src 'unsafe-inline'; form-action 'self'`), and the token echoed only after it matched
  the token shape (HTML-escaped).
* **The spend** is one conditional statement: `UPDATE access_invites SET uses = uses + 1 WHERE token_hash = ? AND uses <
  max_uses AND expires_at > ? AND revoked_at IS NULL`; zero rows means "not valid". Unknown, expired, used up, revoked and
  "person gone or locked" all give **one generic page**, so nothing can be learnt from it.
* **A browser that already holds a pass from this same invite** (`X-Edge-Pass` verifies and the device row belongs to the
  invite) gets the same device back and spends no use: a repeated tap is harmless.
* The edge limits the path (`pub_invite`, 6 a minute per client). A 128-bit token cannot be guessed anyway.
* Every redemption writes an audit row (`invite.redeemed`: device, address, never the token) and one info log line.

### 2.3 Sign-in through the public edge (additions to `login()` in `routers/api/v2/auth.rs`)

Order of the checks after the user lookup (the first two exist today):

1. **Unknown name** → dummy password work, 401 (unchanged).
2. **Owner through the edge** → dummy work, recorded with reason `owner_via_edge`, **not counting towards any lock**, 401
   (unchanged behaviour; now read from the request's edge context instead of re-reading the header).
3. **Wrong person (new, D6):** the request carries a verified device bound to person *P* and `P ≠ this user` → the same
   dummy work, recorded with reason `wrong_person` and `counts_toward_lock = false`, 401. It is the same answer as a wrong
   password, after the same work, so it reveals nothing about which names exist or who the device belongs to. It must not
   count: otherwise a leaked invite for Ana's phone could be used to lock Ben out by "guessing" his name through it.
4. The existing locked / password / session flow, now recording the device id and a failure reason
   (`wrong_password`, `locked`) on every attempt.

Binding applies at **sign-in** (as in iaboy). Binding every later request to the device's person is possible (§12, D-B).

### 2.4 Lockouts, budget and the unblock fix

* **Edge budget** (8 sign-in POSTs per device per UTC day, held in the edge's memory) stays. **Reset** uses a
  *generation*: the state file carries `budget: {device: reset_epoch}` and the counter key becomes
  `<id>:<day>:<epoch>`, so a new epoch starts a fresh count at once without touching the edge's counters. The cap (8) stays
  below Stump's own lock (the 10th failure).
* **Stump lockouts are in the database** (`users.is_locked` plus the failure count), so "Blocked right now" is simply the
  locked accounts with their failures in the last 24 h.
* **Unblock** (`models::services::access::unblock_user`): `is_locked = false` **and** `counts_toward_lock = false` on the
  user's failed rows, in one transaction, then an audit row. `updateUserLockStatus(lock: false)` (the Users page) and the
  CLI's `account unlock` call the same function, so all three agree. The rows stay visible in the sign-in log (marked
  "cleared"); only the count ignores them. The lock query gains `AND counts_toward_lock = true`.
* nginx's own per-client limit (6 sign-ins a minute) has no state to clear; it ends within a minute. The page says so.

### 2.5 The request flow in the app, and what is trusted

A new middleware, `apps/server/src/middleware/edge.rs`, added as the **last layer inside `routers::mount`** (F6), runs for
every request before any handler or `auth_middleware`:

1. `via_public = headers.contains_key("x-edge-public")`. Not present → the request is "home": insert
   `EdgeRequest { via_public: false, device: None }` and go on. Nothing else about the request changes.
2. Present → verify `X-Edge-Pass` with the key (shape, MAC through a constant-time compare, age ≤ 180 days, issued not
   more than a day in the future). The header is believed **only because** the marker is present, and the MAC is checked
   regardless, so planting a bare id is impossible (the lead's blocker 1 for iaboy).
3. A verified id → one indexed lookup in `access_devices`:
   * **unknown id** (a pass the old edge minted before the switch, so the app has never seen it) → registered on the spot as a *legacy* device
     (`INSERT OR IGNORE`, person NULL, label = id, audit `device.registered_legacy`), so it appears in the list and can be
     revoked from the page;
   * **revoked** → 403 with a plain "access turned off" page, immediately;
   * otherwise noted as seen: `last_seen_at`, `last_ip` (from `ClientIp`), `last_agent` (120 chars, control characters
     stripped), written **at most once a minute per device** (a process-wide throttle map; the write is spawned off the
     request).
4. **Belt and braces (`STUMP_EDGE_REQUIRE_PASS`, default `false` until the new edge is live, then `true`):** a public request
   with no verified device is answered 404 unless its path is `/api/_i/…`, `/assets/manifest.webmanifest` or the fixed
   favicons. If the edge's pass check were ever misconfigured, the app would not become public by accident.
5. The result is inserted as a request extension `EdgeRequest { via_public, device }`. `graphql_handler` and
   `graphql_subscription_handler` copy it into the GraphQL request data, where the guard reads it.

The trust chain, in one place: the marker is believed because only `stump-edge` can reach `stump-fork` (no host port,
checked at deploy with `docker port stump-fork`) and the edge overwrites it; the pass is believed because its MAC verifies
under a secret only the edge and the app hold. A client can forge neither.

### 2.6 The state file

`/edge-state/state.json`, mode 0644, in a directory owned by uid 1000 (the `PUID` that `stump-fork` drops to; the edge's
uid 101 reads it):

```json
{"v": 1794423791150, "written": "2026-11-11T14:03:11Z", "revoked": ["ana-phone-k3f9x"], "budget": {"ana-laptop-2m8qd": 1794420000}}
```

* Device ids and epochs only. **No tokens, keys, usernames or addresses.**
* **Written** after every revoke, restore, revoke-all and budget reset, **at application start** (before the listener
  accepts requests) and every 6 hours with the housekeeping pass (a harmless heartbeat).
* **`v` is the write time in milliseconds**, forced to grow (`max(now, last + 1)`). The edge applies a file whenever its `v`
  *differs* from the one it holds (so a restart that resets a counter can never freeze it); it parses first and replaces the
  whole state with **one** `set()` (no moment with no revocations loaded).
* **`publish()` runs under a process-wide lock and reads the database inside it**, so two concurrent revokes cannot leave
  the older write last. Write = temporary file in the same directory, `fsync`, `rename`, then `chmod 0644` (never trusting
  the umask).
* An unreadable or invalid file keeps the edge's last good state (one warning, not one a second). Before the first good read
  the edge is "not ready" and refuses every pass.

### 2.7 Configuration (`core/src/config/edge_config.rs`, nested like OIDC)

```rust
#[derive(Config, Serialize, Clone, PartialEq)]          // not SimpleObject: nothing of it is in GraphQL
#[config(env_prefix = "STUMP_EDGE_")]
pub struct EdgeConfig {
    #[serde(skip_serializing)] pub key: Option<String>,            // STUMP_EDGE_KEY, at least 64 chars
    #[setting(default = "__Host-g3-stump")] pub cookie: String,    // STUMP_EDGE_COOKIE
    pub public_url: Option<String>,        // STUMP_EDGE_PUBLIC_URL  https://g3.tail3e401.ts.net:8443
    pub state_dir: Option<String>,         // STUMP_EDGE_STATE_DIR   /edge-state
    #[setting(default = false)] pub require_pass: bool,            // STUMP_EDGE_REQUIRE_PASS
    #[setting(default = true)]  pub binds_person: bool,            // STUMP_EDGE_BINDS_PERSON
}
// hand-written Debug prints key as "[REDACTED]", as OidcConfig does
```

In `StumpConfig`: `#[setting(nested)] #[graphql(skip)] pub edge: Option<EdgeConfig>`. With no `STUMP_EDGE_*` variables the
feature is inert (`edge = None`): the middleware treats every request as home, the invite routes answer 404, and everything
is as today. A key shorter than 64 characters counts as "no key" (as `gate.js` does): passes cannot verify, and the page
says so. Unit tests: the key is absent from `format!("{:?}", config)` and from `serde_json::to_string(&config)`; the nested
env prefix works (as OIDC's).

---

## 3. Edge changes (`deploy/public-edge/`; applied by the owner's deploy steps, not by the app)

**New `stump-gate.js`** (mounted from `edge/`, imported as `gate`): the reviewed `iaboy-gate.js`, copied, with the header
comment and file names changed and nothing else (`device`, `pass_header`, `renew`, `signin`, `poll`; `SIGNIN_CAP = 8`; the
state file at `/etc/nginx/state/state.json`; **no `invite`**, invite pages are the app's). Keeping the logic identical is the
point: it has been through an Opus review and a real-nginx test run.

**`nginx.conf` deltas** against the live `stump-fork/edge/nginx.conf`:

1. `js_import gate from /etc/nginx/edge/stump-gate.js;` (was `/etc/nginx/njs/gate.js`); keep `env EDGE_KEY; env EDGE_COOKIE;`.
2. `js_set $edge_device gate.device; js_set $edge_pass gate.pass_header; js_set $edge_renew gate.renew; js_set $edge_signin
   gate.signin;` and a second dictionary: `js_shared_dict_zone zone=edge_state:256k type=string;` next to `edge_signin`.
3. Inside the public `server`: `location @periodic { js_periodic gate.poll interval=1s; }` (one worker polling is enough: the
   dictionary is shared memory).
4. **Two include files instead of per-location header lines.** `proxy_common.conf` (home and public) gains
   `proxy_set_header X-Edge-Pass "";`. A new **`proxy_public.conf`** is `include`d by **every public forwarding location** and
   contains `include proxy_common.conf;` plus `Host g3.tail3e401.ts.net:8443`, `X-Forwarded-Proto https`, `X-Edge-Public 1`
   and `X-Edge-Pass $edge_pass`. This replaces the three repeated lines per public location and removes the chance of one
   location forgetting the marker. (The login location's own `X-Edge-Public 1` moves into it.)
5. `location ^~ /api/_i/ { limit_req_log_level info; error_log /dev/stderr crit; limit_req zone=pub_invite burst=4 nodelay;
   client_max_body_size 8k; proxy_pass http://stump; include …/proxy_public.conf; }` (was `js_content gate.invite`). The
   `$edge_deny` map already lets `/api/_i/` through without a pass. New zone `limit_req_zone $pub_client zone=pub_invite:1m
   rate=6r/m;`. The two log lines keep the token out of the **error** log (a rate-limit rejection logs the request line).
6. **Access log without tokens:** `map $uri $log_uri { default $uri; "~^/api/_i/" "/api/_i/-hidden-"; }` and the `edge`
   `log_format` uses `$log_uri`.
7. `location ~* ^/settings/access { return 404; }` in the public server (an SPA route: harmless to the data, but "admin
   routes 404 publicly" costs one line).
8. **Home listener refuses Funnel-originated requests:** `if ($http_tailscale_funnel_request) { return 404; }`. tailscaled
   sets `Tailscale-Funnel-Request: ?1` on Funnel traffic only (the edge log shows `funnel="?1"`), so if Funnel were ever
   pointed at `:10801` by mistake, the no-pass home listener still would not serve the internet. (Confirmed at deploy by
   the normal check that a request through `:10802` still returns 200: it would be 404 if tailscaled set the header on
   `serve` traffic too.)
9. `revoked.map` and its `map` stay as the terminal break-glass.
10. The state directory is mounted read-only at `/etc/nginx/state`. The `../edge-common:/etc/nginx/njs` mount **stays**
    through the switch (the lead's review, item 3: the old live `nginx.conf` imports `gate.js` from it until the new
    edge is installed, so dropping it at the compose step would make any recreate in between fail); nothing loads it
    afterwards, and `invite.sh` is a host script that does not need it.

**Compose** (applied at deploy; `apply-compose.py` is an idempotent patcher with `--dry-run` and a backup, like iaboy's):

```yaml
  stump-fork:
    environment:
      - STUMP_EDGE_KEY=${STUMP_EDGE_KEY:?set STUMP_EDGE_KEY in .env}     # the key already in ./.env (mode 600)
      - STUMP_EDGE_PUBLIC_URL=https://g3.tail3e401.ts.net:8443
      - STUMP_EDGE_STATE_DIR=/edge-state
      - STUMP_EDGE_REQUIRE_PASS=false                                    # "true" after the new edge is live
    volumes:
      - ./edge-state:/edge-state
  stump-edge:
    volumes:
      - ./edge:/etc/nginx/edge:ro
      - ./edge-state:/etc/nginx/state:ro
```

The current comment says the key never reaches the app; that property is deliberately given up (D2). The edge keeps
`EDGE_KEY`/`EDGE_COOKIE` as it has today.

---

## 4. Data model

One migration, `m20261113_000000_public_access` (provisional: the integrator renames it so the order in `Migrator`
matches deploy order; once deployed the name is frozen; never two migrations in one deploy). SQLite only, in the style of
`m20261110_000000_user_sharing_settings`; its `down()` and the manual-down SQL below are tested.

```sql
CREATE TABLE IF NOT EXISTS access_invites (
    id          TEXT PRIMARY KEY,                       -- uuid
    token_hash  TEXT NOT NULL UNIQUE,                   -- sha256 hex; the token itself is never stored
    person_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE ON UPDATE CASCADE,
    label       TEXT NOT NULL,                          -- device name typed by the owner ("Ana's phone")
    created_at  TEXT NOT NULL,
    created_by  TEXT NOT NULL,                          -- the admin's username at the time
    expires_at  TEXT NOT NULL,
    max_uses    INTEGER NOT NULL DEFAULT 3,
    uses        INTEGER NOT NULL DEFAULT 0,
    revoked_at  TEXT
);
CREATE TABLE IF NOT EXISTS access_devices (
    id              TEXT PRIMARY KEY,                   -- the pass name, [a-z0-9-]{1,24}
    invite_id       TEXT REFERENCES access_invites(id) ON DELETE SET NULL,
    person_id       TEXT,                               -- NULL = legacy (terminal) pass: no binding. NO foreign key (below)
    label           TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    first_seen_at   TEXT,
    last_seen_at    TEXT,
    last_ip         TEXT NOT NULL DEFAULT '',
    last_agent      TEXT NOT NULL DEFAULT '',
    revoked_at      TEXT,
    revoked_by      TEXT,
    budget_reset_at INTEGER NOT NULL DEFAULT 0          -- epoch of the last manual budget reset
);
CREATE INDEX IF NOT EXISTS access_devices_person_idx ON access_devices (person_id);
CREATE TABLE IF NOT EXISTS access_audit (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    at      TEXT NOT NULL,
    actor   TEXT NOT NULL,                              -- the admin's username, or 'public' for redemptions
    action  TEXT NOT NULL,                              -- invite.create | invite.revoke | invite.redeemed | device.revoke | …
    target  TEXT NOT NULL DEFAULT '',                   -- an invite id, device id or user id
    detail  TEXT NOT NULL DEFAULT ''                    -- short; built from ids and labels, never a token, link or password
);
CREATE INDEX IF NOT EXISTS access_audit_at_idx ON access_audit (at);
ALTER TABLE user_login_activity ADD COLUMN device_id TEXT;                                   -- the pass id of a public sign-in
ALTER TABLE user_login_activity ADD COLUMN failure_reason TEXT;                              -- wrong_password | locked | owner_via_edge | wrong_person
ALTER TABLE user_login_activity ADD COLUMN counts_toward_lock BOOLEAN NOT NULL DEFAULT 1;   -- 0 = ignored by the lock rule
```

* **No foreign key on `access_devices.person_id`, on purpose.** A cascade would delete the device rows of a hard-deleted
  user, and the next request with that still-valid pass would **register it again as an active legacy device**. Rows are kept
  (they show as "person removed"); a person who no longer exists cannot sign in anyway.
* **Retention** (the daily/6-hourly housekeeping, `models::services::access::prune`): `access_audit` 400 days; expired or
  revoked invites 30 days after, unless a live device came from them; **revoked devices 200 days after revoking**. That is
  longer than a pass can live (180 days) plus slack, so a revocation is never forgotten while its pass could still verify
  (iaboy's code already uses 200 for the same reason; a 90-day figure in its design text would have been wrong).
  `user_login_activity` keeps its existing behaviour (kept until the owner clears it).
* **Statuses are computed**, not stored: an invite is *unused* (`uses = 0`), *used*, *used up* (`uses = max_uses`), *expired*
  or *revoked*; a device is *active* or *revoked*; a sign-in row is *ok*, *failed* or *cleared*.
* **Manual down** (`research/2026-11-13-public-access-manual-down.sql`, written with the implementation; host `sqlite3` is
  3.45, `DROP COLUMN` needs 3.35):
  ```sql
  BEGIN;
  DROP TABLE IF EXISTS access_audit;
  DROP TABLE IF EXISTS access_devices;
  DROP TABLE IF EXISTS access_invites;
  ALTER TABLE user_login_activity DROP COLUMN counts_toward_lock;
  ALTER TABLE user_login_activity DROP COLUMN failure_reason;
  ALTER TABLE user_login_activity DROP COLUMN device_id;
  DELETE FROM seaql_migrations WHERE version = 'm20261113_000000_public_access';
  COMMIT;
  ```
  An older image refuses to start on a database that records a migration it does not know, so rolling the **image** back
  needs this (or a snapshot restore) first.

---

## 5. GraphQL API (`crates/graphql/src/{object,query,mutation}/access.rs`)

Every field below has `guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)"`. `NotViaPublicEdgeGuard` (in `guard.rs`) reads
`EdgeRequest` from the request data and refuses when `via_public` is true; no data means "not public". People appear as the
existing slim `ReaderProfile { id, username, avatarUrl }` (batched by `ReaderProfileLoader`, no query per row).

```graphql
type AccessOverview { configured: Boolean!  requirePass: Boolean!  publicUrl: String  stateWrittenAt: DateTime
                      activeDevices: Int!  openInvites: Int! }
enum AccessInviteStatus { UNUSED USED USED_UP EXPIRED REVOKED }
type AccessInvite { id: ID!  person: ReaderProfile  label: String!  createdAt: DateTime!  createdBy: String!
                    expiresAt: DateTime!  maxUses: Int!  uses: Int!  status: AccessInviteStatus! }
type CreatedAccessInvite { invite: AccessInvite!  link: String! }      # the link exists only in this response
enum AccessDeviceStatus { ACTIVE REVOKED }
type AccessDevice { id: ID!  person: ReaderProfile  label: String!  legacy: Boolean!  createdAt: DateTime!
                    firstSeenAt: DateTime  lastSeenAt: DateTime  lastIp: String!  lastAgent: String!
                    status: AccessDeviceStatus!  revokedAt: DateTime  revokedBy: String }
enum AccessSignInReason { WRONG_PASSWORD LOCKED OWNER_VIA_EDGE WRONG_PERSON }
type AccessSignIn { at: DateTime!  person: ReaderProfile  ip: String!  userAgent: String!  deviceId: String
                    deviceLabel: String  successful: Boolean!  reason: AccessSignInReason  cleared: Boolean! }
type AccessBlockedUser { person: ReaderProfile!  failuresLast24h: Int!  lastFailureAt: DateTime }
type AccessAuditEntry { id: ID!  at: DateTime!  actor: String!  action: String!  target: String!  detail: String! }
# page types are defined in access.rs (XPage { nodes, pageInfo }); nothing is added to pagination.rs

extend type Query {
  accessOverview: AccessOverview!
  accessInvites(pagination: Pagination! = {offset: {page: 1, pageSize: 50}}): PaginatedAccessInviteResponse!
  accessDevices(pagination: Pagination! = …): PaginatedAccessDeviceResponse!
  accessSignIns(failuresOnly: Boolean! = false, pagination: Pagination! = …): PaginatedAccessSignInResponse!
  accessBlockedUsers: [AccessBlockedUser!]!
  accessAudit(pagination: Pagination! = …): PaginatedAccessAuditResponse!
}
extend type Mutation {
  createAccessInvite(input: CreateAccessInviteInput!): CreatedAccessInvite!   # personId, label, validForHours (1|24|72|168|720), maxUses (1..10)
  revokeAccessInvite(id: ID!): Boolean!
  revokeAccessDevice(id: ID!): Boolean!               # also revokes the invite it came from while that is active
  restoreAccessDevice(id: ID!): Boolean!
  revokeAccessDevicesOfPerson(personId: ID!): Int!    # all devices, all open invites, and the person's sessions and refresh tokens
  resetAccessDeviceBudget(id: ID!): Boolean!
  unblockUser(id: ID!): Boolean!                      # unlock + forgive the failures in the window (§2.4)
}
```

* **List fields** use `pagination_cost` (`#[graphql(complexity = …)]`), page size at most 200 (default 50); `accessBlockedUsers`
  is priced `RELATED_LIST * child_complexity`. No unpaginated list is added (the complexity rules of the security branch apply).
* **Revoke one device** does *not* end the person's sessions (sessions are not tied to a device; their other phones stay
  signed in); **revoke all of a person** does. For a stolen phone the owner revokes the device (the thief then cannot reach
  the app at all) and sets a new password, which already ends sessions.
* **Existing mutations get one audit write each** (the owner acting on someone else): `updateUser` with a password
  (`user.password_set`), `updateUserLockStatus` (`user.lock`, `user.unlock`), `deleteUserSessions`
  (`user.sessions_deleted`). `updateUserLockStatus(false)` now calls the unblock function.
* **The link** (`CreatedAccessInvite.link`) is built from `STUMP_EDGE_PUBLIC_URL` and returned once; nothing stores or logs it.
  The page keeps it in component state only (not in the query cache's persisted data, not in the URL).
* **Existing `loginActivity` / `UserLoginActivityModel`** gains `deviceId` and `failureReason` through the entity
  (`counts_toward_lock` is `#[graphql(skip)]`); the old Users → Login activity table is unchanged.
* **Server side of the logs:** `TraceLayer` gets a `make_span_with` that records `/api/_i/-hidden-` instead of the URI of any
  `/api/_i/…` request (token and `t=` value included), and nothing logs the form body (F8); a unit test covers both.

---

## 6. The admin screens (`/settings/access`, owner only)

Listed under **Management** in Settings as **Public access** / **Acceso público**, after Users. It is owner-only: the
route is rendered only for `isServerOwner`, and `useRouteGroups` learns an `ownerOnly` flag. (Its existing
`customPermission` field is set by the smart-list settings routes but **ignored by the filter**; honouring it would change
what that unrelated screen shows, so a new flag is added and `customPermission` is left alone.) Keys in `settingsScene.server/access.*`
in `en-US.json` and `es-ES.json` (the fork's two maintained catalogs; other locales fall back to `en-US`).
The edge answers 404 for the path, and every query refuses public requests.

1. **Status strip.** "Public access: ready" / what is missing (key, public address, state folder); whether devices are
   required yet (`requirePass`); when the app last wrote the edge's state; counts of active devices and open invites.
2. **Invite links.**
   * Form: Person (a drop-down of accounts other than the owner, not locked or deleted) · Device name (≤ 40 chars) ·
     Valid for (1 hour, 24 hours, **3 days**, 7 days, 30 days) · Uses (1, **3**, 5, 10) → **Create link**.
   * Right after creation the page shows the full link once in a read-only box with **Copy** ("This link is shown only now.
     Create another if you lose it."). Copy uses the Clipboard API where it exists and a select-and-`execCommand` fallback
     on plain-HTTP LAN addresses.
   * Table: Person · Device name · Created · Expires · Uses (2 / 3) · Status · **Revoke**.
3. **Devices with a pass.** Grouped by person; columns Device · First seen · Last seen · Last address · Status. Buttons:
   **Revoke** (with confirmation), **Restore**, **Reset sign-in budget**, and per person **Revoke all of <person>**. A
   *Legacy* badge marks passes made in the terminal; *person removed* marks orphans. Help text: "A revoked device stops
   working within a couple of seconds."
4. **Sign-in protection.**
   * *Blocked right now*: locked accounts with their recent failures and **Unblock** (and a note when the edge's own 6-a-minute
     limit is in play: it clears by itself).
   * *Recent sign-ins* (50 a page, "failures only" filter): time · person · address · device · result · reason (a cleared
     failure is dimmed).
5. **People.** Rows for the friends: name · last sign-in · state, with **Set a new password** (a dialog with a generated
   password, 20 characters from an unambiguous alphabet through `crypto.getRandomValues`, a Copy button and "type your own";
   it posts the **existing** `updateUser` and the server ends their sessions as it does today; shown once), **Unblock** when
   locked, and **Manage** (the existing user page). If the person lacks *Change password*, a hint says so (they could not
   replace the temporary one).
6. **Activity.** The last 50 audit rows (who, what, when, target).

Mobile: tables become stacked cards (the Users page's existing pattern); dialogs use `ConfirmationModal`. The page's queries
are `useSuspenseGraphQL` documents with codegen like the rest of the settings.

---

## 7. Files to add and change

**Rust**

| Path | Change |
|---|---|
| `crates/migrations/src/m20261113_000000_public_access.rs` (+ `lib.rs`) | the migration of §4, with up/down and manual-down tests |
| `crates/models/src/entity/{access_invite,access_device,access_audit}.rs` (+ `mod.rs`), `user_login_activity.rs` | entities; three new fields on the login activity (`counts_toward_lock` skipped in GraphQL) |
| `crates/tests/src/db.rs` | create the new tables for the test database |
| `crates/models/src/services/access.rs` (+ `mod.rs`) | invites, devices, sign-in log queries, audit, `unblock_user`, `prune`, `edge_state` (revoked ids and budget epochs) |
| `core/Cargo.toml` | **no new dependency**: `core` already depends on `ring` 0.17 (`ring::hmac` gives HMAC-SHA256 with a constant-time `verify`, `ring::digest` gives SHA-256), `base64` and `rand` |
| `core/src/config/edge_config.rs`, `stump_config.rs` | §2.7 |
| `core/src/access/{mod,pass,state}.rs` | pass/legacy-invite mint and verify, token hashing, `publish()`, the housekeeping loop |
| `apps/server/src/middleware/{edge.rs,mod.rs}` | §2.5 |
| `apps/server/src/routers/invite.rs`, `routers/api/mod.rs`, `routers/mod.rs` | `/api/_i/` routes and pages; mounted next to `/graphql` and `/v2`; the middleware layer |
| `apps/server/src/routers/api/graphql.rs` | copy `EdgeRequest` into the request and websocket data |
| `apps/server/src/routers/api/v2/auth.rs` | §2.3 (binding, device and reason on every attempt, `counts_toward_lock` in the lock query, edge context instead of the raw header) |
| `apps/server/src/http_server.rs` | `TraceLayer` URI redaction; first `publish()` before serving; spawn the housekeeping loop (like the smart-list warmer) |
| `crates/graphql/src/{object,query,mutation}/access.rs` (+ the three `mod.rs`) | §5 |
| `crates/graphql/src/guard.rs`, `data.rs` | `NotViaPublicEdgeGuard`; the `EdgeRequest` type |
| `crates/graphql/src/mutation/user.rs` | audit writes; `updateUserLockStatus(false)` unblocks |
| `crates/cli/src/commands/account.rs` | `account unlock` calls the same unblock (one line) |
| `crates/graphql/schema.graphql`, `packages/graphql/src/client/**` | regenerated, never merged by hand |

**Web** (`packages/browser/src/…`)

| Path | Change |
|---|---|
| `scenes/settings/server/access/{index,AccessScene,StatusStrip,InviteSection,DeviceSection,SignInSection,PeopleSection,ActivitySection}.tsx` + `graphql.ts` | the page |
| `scenes/settings/{routes.ts,SettingsRouter.tsx}`, `hooks/useRouteGroups.ts` | the owner-only item and route; the `ownerOnly` filter |
| `packages/i18n/src/locales/{en-US,es-ES}.json` | `settingsScene.server/access.*` |

**Edge and deploy (`deploy/public-edge/`, new in the repo; nothing live is touched by the implementer)**

`stump-gate.js`, `nginx.conf` (the complete file), `proxy_common.conf`, `proxy_public.conf`, `revoked.map` (empty),
`compose-snippet.yaml`, `apply-compose.py` (idempotent, `--dry-run`, backup), `vectors.json` (fixed pass / legacy-invite /
forged vectors from `openssl dgst -sha256 -hmac`, read by the Rust test and the edge script), `verify-edge.sh` (§9),
`DEPLOY.md` (exact ordered commands, checks and rollback). Plus a `FORK.md` row and
`research/2026-11-13-public-access-manual-down.sql`.

---

## 8. Security review of this design

| Threat | Answer |
|---|---|
| Forging a pass | needs the 256-bit key; the MAC compare is constant time (the edge compares through a second HMAC, the app through `ring::hmac::verify`) |
| Planting `X-Edge-Pass` / `X-Edge-Public` | stripped on both listeners and replaced on the public one (§1 proof); the app also re-verifies the MAC; `stump-fork` has no host port |
| Funnel pointed at the home listener by mistake | the home listener answers 404 to any request carrying `Tailscale-Funnel-Request` (§3, item 8); the deploy checks `tailscale funnel status` shows only `:8443 → 127.0.0.1:10803` |
| Reaching the admin page or its API publicly | edge 404 for `/settings/access`; `NotViaPublicEdgeGuard` + `ServerOwnerGuard` on every field (so an owner session plus a pass in one browser still cannot) |
| Guessing an invite | 128-bit random token, hashed at rest, 6 a minute per client at the edge, one generic answer for every failure |
| Link previews spending uses | `GET` is free; only the confirmation `POST` spends |
| Race on a multi-use invite | one conditional `UPDATE`, rows checked (a test fires 10 parallel redemptions at a 3-use invite and expects exactly 3) |
| A leaked link or stolen device | bound to one person (D6): at most 8 guesses a day per device against that one account and none against the owner; expiry and use limit; the owner revokes the invite or the device |
| A stolen device returning through its own link | revoking the device revokes its invite; a legacy link never revives a revoked device |
| Using a leaked device to lock someone else out | `wrong_person` attempts are recorded but do not count towards the lock (D6) |
| Secrets in the UI, logs or database | the token is shown once and stored hashed; the key is in a struct that redacts and skips serialising; the state file holds ids and numbers only; the sign-in log never stores typed text for unknown names; audit `detail` is built from ids and labels; tokens are redacted in the app's trace spans and in the edge's access and error logs |
| Tampering with the state file | only `stump-fork` writes it; the edge mounts it read-only; no other container mounts it |
| Stale or missing state | not ready = refuse all passes (fail closed); a bad file keeps the last good state; the app enforces revocations itself |
| CSRF on the admin API | GraphQL takes JSON only (the edge rejects `text/plain` posts) and the session cookie is `SameSite=Lax`; every action is a mutation |
| XSS through labels | the web app escapes; labels are plain text (≤ 40 chars, control characters stripped); device ids are `[a-z0-9-]` |
| Write load from "last seen" | at most one write a minute per device, off the request path |
| Enumerating accounts | unknown name, wrong password, wrong person and owner-through-edge all give the same 401 after the same work |
| Deleting a person leaving a live pass | device rows are never cascade-deleted (§4); the account cannot sign in; the owner can revoke the row |

---

## 9. Tests

**Rust unit / service tests**

* `core::access::pass`: fixed vectors from `vectors.json` (the same file the edge script reads); tamper cases (id, issued, mac,
  length), expiry (180 days), a future `issued`, a short key, legacy `name.expiry.mac` accepted until expiry; cookie
  attributes of the minted `Set-Cookie`.
* `core::access::state`: contents, atomic replace, mode 0644, `v` grows even within one millisecond, no secret in the file,
  a missing directory logs and does not crash, **two concurrent publishes never leave the older one last**.
* `core::config`: `Debug` and `serde_json` of the config never contain the key; the nested env prefix loads.
* `models::services::access`: invite create / expire / revoke; the atomic spend (10 parallel redemptions of a 3-use invite
  give exactly 3); only the hash is stored; same-invite pass returns the same device; legacy register; device revoke also
  revokes the invite; revoke-all; **a revoked legacy id is never revived**; prune keeps revoked devices 200 days; the sign-in
  query and its `failures_only`; **`unblock_user` clears `is_locked` and the window** (nine failures, locked, unblock, one
  more failure does not lock, nine more do); audit rows for each action.
* `migrations`: up creates the tables and columns with their defaults on a database holding old `user_login_activity` rows
  (their `counts_toward_lock` is true), down restores the schema, and the manual-down SQL gives the same result.

**Server integration (`apps/server/tests/auth/edge.rs`, `invites.rs`, `graphql/access.rs`; the harness gains a
`TestApp::with_config(...)` for an edge key and state directory)**

* **Middleware:** headers are ignored without the marker; marker + valid pass notes the device (last seen throttled); marker
  with a forged or expired pass → no device; an unknown valid id registers as legacy and shows in `accessDevices`; a revoked
  device → 403 **at once** (no state file involved); with `require_pass`, a public request without a device is 404 except the
  invite pages, the manifest and the favicons; a home request is never touched.
* **Invites:** `GET` (also with a link-preview user agent) spends nothing and sets no cookie; `POST` redeems and sets
  `__Host-g3-stump` with `Secure; HttpOnly; SameSite=Lax`; every failure renders the same page; not available without the
  marker (404); a second tap with the pass from the same invite spends nothing; an invite for the owner is refused at
  creation and at redemption; the token never appears in the trace output.
* **Sign-in:** the bound person signs in; another user through that device gets the wrong-password answer, is recorded as
  `wrong_person`, and **ten such attempts do not lock that user**; the owner through the edge is still refused and recorded;
  every public attempt records `device_id`; existing login tests stay green.
* **GraphQL access:** every field refuses a non-owner (including one holding `ManageServer` and `ManageUsers`), refuses the
  owner on a public request, and works for the owner at home; `createAccessInvite` returns the link once and nothing later
  returns it; revoke → the state file lists the device before the response ends; the complexity of each list stays in the
  limits and the biggest page costs what `pagination_cost` says; `updateUser` by the owner writes an audit row.

**Web (vitest + `tsc -b`):** the page renders each section from mocked queries; Copy (both paths) and "shown once"; the
password dialog's generator length and alphabet; the owner-only filter in `useRouteGroups` and the updated `routes.test.ts`;
no `link` ever reaches a persisted cache.

**Edge (`deploy/public-edge/verify-edge.sh`, Docker, run by the implementer and again at deploy):** a throwaway `nginx:alpine`
with the real `nginx.conf`, a stub upstream that echoes headers, and a state file: no pass → 404 everywhere except the invite
path, manifest and icons; a pass from `vectors.json` is accepted and the stub sees `X-Edge-Public: 1` and `X-Edge-Pass`;
forged and expired passes → 404; **before the state file exists every pass is refused**; after it exists passes work; adding
a device to `revoked` makes it 404 **within 3 s with no reload**, removing it brings it back; a budget epoch bump gives a
fresh count and the 9th sign-in in a day is 429; client-supplied `X-Edge-Pass` / `X-Edge-Public` never reach the stub; a
corrupt state file keeps the last good state; a request with `Tailscale-Funnel-Request` to the home listener is 404; no
token appears in the access or error log.

**Manual, at deploy:** the owner creates an invite on the page, opens it on a phone off the tailnet, signs in, revokes the
device from the page and watches the phone lose access in seconds; unblocks a deliberately locked test account.

---

## 10. Deploy and rollback (outline; the implementer writes the exact commands into `deploy/public-edge/DEPLOY.md`)

Stump's Funnel is **off** today, so this deploy has no live public traffic to protect; it still follows the order that keeps
it safe when Funnel is on (the edge fails closed until the app has written the state file; the app must not demand devices
until the new edge sends them).

1. Merge the branch; build the image on `l5` and load it on `g3`. Before restarting: no jobs or scans running; a database
   snapshot (`config.backup-<UTC>-db`) with the manual-down SQL beside it; compose backup; backup `edge/` and `.env`.
2. `mkdir -p /mnt/data2/containers/stump-fork/edge-state` (owner `sthefano` = uid 1000, mode 755).
3. Compose: the `stump-fork` environment and volume of §3 with `STUMP_EDGE_REQUIRE_PASS=false`; `docker compose up -d
   --no-build stump-fork`. Check: migration applied, `state.json` exists with `"v"`, logs clean, LAN and tailnet fine,
   `docker port stump-fork` prints nothing.
4. Stage the new edge files in `edge-new/` (not over the live ones), validate with `nginx -t` in a throwaway container with
   the **same mounts**, then copy them over and `docker compose up -d --force-recreate stump-edge` (a new volume needs a
   recreate) in one command line, so a restart in between cannot load the new config without the state mount.
5. Local checks against `127.0.0.1:10803` (`verify-edge.sh` against the live edge): no pass 404; a link from `invite.sh`
   works (the app verifies it and registers a *Legacy* device, which then shows in **Public access → Devices**; its later
   requests updating *last seen* prove the app received and verified `X-Edge-Pass`); a new invite from the page works end
   to end; `/settings/access` is 404; a request through `:10802` still returns 200 (the Funnel guard of §3 item 8 does not
   touch tailnet traffic), while the same request with a hand-added `Tailscale-Funnel-Request` header is 404.
6. Revocation test: revoke a throwaway device from the page; 404 at the edge within about 2 s, 403 at the app at once; reset
   the budget; unblock a locked test account.
7. Set `STUMP_EDGE_REQUIRE_PASS=true`, `docker compose up -d --no-build stump-fork`; repeat 5.
8. Turning Funnel on stays the owner's step: `tailscale funnel --bg --https=8443 http://127.0.0.1:10803`, and
   `tailscale funnel status` must show only that target.

**Rollback (in this order; no data to restore):** `tailscale funnel --https=8443 off` while working; restore the saved `edge/`
files and the compose file; run the manual-down SQL with Stump stopped; start the previous image (`docker compose up -d
--force-recreate stump-fork stump-edge`). Existing passes keep working throughout (the format did not change);
`invite.sh stump …` and `revoked.map` work before, during and after. Two operational notes for `DEPLOY.md`: **restoring an
older database snapshot forgets the revocations made since**, so re-check *Devices* after any restore; and **rotating
`STUMP_EDGE_KEY`** (edit `.env`, recreate `stump-fork` and `stump-edge`) invalidates every pass at once, after which each
person needs a new invite. A reload of the edge can refuse passes for a second or two while it re-reads the state file.

---

## 11. Alternatives considered

| Option | Why not |
|---|---|
| Edge mints passes; the app asks the edge through an internal `subrequest` | the key would stay out of `stump-fork`, but it adds an internal location, a second secret and header juggling, and none of it can be unit-tested without Docker (iaboy's D2 reasoning, same here) |
| A control endpoint on the edge for revokes | a new network surface reachable from every container on the network, a secret to protect, state lost on restart; the file needs none of that |
| The app writes `revoked.map` and calls `nginx -s reload` | it would need to run commands in another container (the Docker socket: no) |
| Revoke by changing the key | revokes everyone at once; not a per-device control |
| Only the app checks revocation | a revoked device would still reach the edge's static files and invite pages, and "no pass, no page" would depend on the app |
| A separate `access_signins` table (as iaboy has) | Stump already keeps a sign-in log, owner-visible, with lock logic built on it; a second log would disagree with the lock count |
| Store tokens in plain text so a link can be shown again | a database read would leak working links; creating another takes one click |
| Bind every request, not just sign-in, to the device's person | needs a check inside `auth_middleware`'s three branches for little extra protection (§12, D-B) |
| A forced password change | see D10 and §12, D-C |

---

## 12. Effort, and decisions for the lead

| Part | Estimate |
|---|---|
| Migration, entities, `models::services::access`, unblock fix | 0.75 day |
| `core::access` (pass, state, housekeeping) and the nested edge config | 0.75 day |
| Middleware, invite routes and pages (en/es), `login()` changes, trace redaction, guard | 1.25 days |
| GraphQL access module (objects, queries, mutations, complexity) and audit hooks | 1 day |
| Web page, five sections and dialogs, i18n (en, es), owner-only route filter | 1.75 days |
| Edge files, compose patcher, `DEPLOY.md`, `verify-edge.sh`, vectors | 0.75 day |
| Tests across Rust, web and the edge script, docs and `FORK.md` | 1 day |
| **Total** | **about 7 days** (±30%); the builds and tests run on `l5` |

Larger than iaboy's 3.75 days because Stump needs a migration with a core-table change, a Rust service layer, GraphQL with codegen,
a React page in two locales, and the extra login-path tests; the reviewed edge code is reused rather than rewritten.

**Decisions taken here (override if you disagree), with the cost of the alternative**

* **D-A, the key lives in `stump-fork` (D2).** Fallback: the subrequest design in §11 (+1 day, no key in the app).
* **D-B, person binding at sign-in only.** Binding every authenticated request to the device's person adds a check in
  `auth_middleware` (session, bearer and Basic branches) for a small gain (a replayed session of *another* person through this
  device). About 0.5 day if wanted.
* **D-C, no forced "change at next sign-in" (D10).** If wanted: a `users.must_change_password` column set by the owner's
  temporary password and cleared when the person changes their own; the login response and `AuthUser` carry it; the web app
  routes to *Account* and blocks the rest; and a server-side restriction in `auth_middleware` (everything except the viewer,
  `updateViewer` with a password and logout is refused) so it is not only a screen. About 1.5 days and a change in the core
  auth path; I recommend deferring it.
* **D-D, only the new operations refuse public requests (D11).** Optional, small and consistent: also treat the owner as a
  plain user on public requests everywhere (clear `is_server_owner` in `auth_middleware` when the marker is present), so an
  owner session reaching Funnel can do nothing owner-like at all. About 0.25 day, with a visible behaviour change to confirm.
* **D-E, reuse `user_login_activity` (D8).** Alternative: a parallel `access_signins` table as in iaboy (+0.25 day, a second log).
* **D-F, the existing Users page gets the unblock fix (D9).** It changes `updateUserLockStatus(false)` and `account unlock`
  for everyone, which is the intended repair of F1.
* **D-G, `edge-common/gate.js` stays on disk** (nothing imports it after the switch; `invite.sh` is independent of it).

**Findings for the parent, outside this package:** (1) F1 above is a live bug today (an unlocked account re-locks on its next
typo). (2) The session cookie is shared across ports (F3); a general fix is D-D. (3) iaboy's as-built `REVOKED_KEEP_DAYS = 200` is
right; only its design text said 90 days. (4) Native Stump apps cannot use the public edge (F9).

## 13. As built: the lead's review applied (checked against the code, 2026-11-11)

This section supersedes the parts of the design it contradicts. In particular §4's three `ALTER TABLE user_login_activity`
lines are **not** in the migration (R6): the sign-in marks are the new table `access_signin_marks`.

| Lead's item | As built | Where it is pinned |
|---|---|---|
| R1 `Stump.toml` must not get an `[edge]` table | `edge` and `key` are `skip_serializing`; the settings come from the environment only; `DEPLOY.md` backs `config/Stump.toml` up with the snapshot and restores it on a rollback | `test_the_edge_key_and_section_are_never_written` (core), `DEPLOY.md` §1 and Rollback |
| R2 "public" never depends on the config | `via_public` is the marker header, always; only a device check and minting a pass need a usable key; `require_pass` without a key closes the public side | `middleware/edge.rs`, `tests/access/edge.rs` |
| R3 the internet is never the owner | one `authorize()` for the session, token, Basic and API-key paths refuses the owner on a public request with a `401` that does not delete the owner's cookie; `register` refuses public requests; `NotViaPublicEdgeGuard` is the second check | `tests/access/edge.rs` (REST, GraphQL, websocket) |
| R4 CSRF | `POST /api/_i/` refuses `Sec-Fetch-Site` other than `same-origin`/`none`, or a foreign `Origin`; every access mutation also needs `SameOriginGuard` | `tests/access/invites.rs`, `tests/access/graphql.rs` |
| R5 set a password and require a change | `setUserPassword(id, password, requireChange = true)` (owner only, not via the edge, same origin), table `access_password_resets`, REST and websocket refused and GraphQL limited to `me`/`updateViewer` while it is set, `updateViewer` clears it, the web app shows only the new-password screen (`PasswordGate`) | `tests/access/password.rs`, `PasswordGate.test.tsx`, `AppRouter.test.tsx` |
| R6 tables only | `access_signin_marks(login_activity_id PK, device_id, failure_reason, counts_toward_lock)`; the lock rule ignores marks with `counts_toward_lock = 0`; the unblock forgives the person's failures in one transaction; `UserLoginActivityModel` is unchanged | `tests/access/signin.rs`, `crates/migrations` (manual down) |
| R7 guards refuse when they cannot tell | every guard refuses when there is no `EdgeRequest`; the subscription handler passes it | `guard.rs` tests, `the_websocket_is_guarded_like_the_rest_of_graphql` |
| R8 state file | a failed write is reported to the owner; the temporary file is made `0644` before the rename; the version only grows, under one lock | `a_state_file_that_cannot_be_written_is_reported`, `the_version_of_the_state_file_always_grows`, `concurrent_changes_never_leave_an_older_state_in_the_file` |
| N1 headers once | the public forwarding headers are set at server level, no location sets its own; one map for `Upgrade`/`Connection` | `verify-edge.sh` (63 checks) |
| N2 `/api/_i/` | its own header set (`Referrer-Policy: no-referrer`), and no pass renewal | `verify-edge.sh` |
| N3 redemption | the person is checked before a use is spent; the spend and the device are one transaction; a revoked device counts as no device on the invite pages | `tests/access/invites.rs` |
| N4 timestamps | one second either side of an invite's expiry | `an_invite_works_until_its_expiry_and_not_after` |
| N5 audit | password sets by any actor, self-service password changes, locks and unlocks, and `deleteLoginActivity` | `a_password_set_by_a_user_manager_and_a_cleared_sign_in_log_are_audited` (a *Manage users* actor's password set, and `login_activity.cleared`), `a_password_the_owner_sets_must_be_replaced_before_anything_else_works` (`user.password_set`, `user.password_changed`), `a_lock_by_hand_says_so_and_the_lock_and_the_unblock_are_audited` (`user.lock`, `user.unblock`) |
| N6 two kinds of lock | `lockedBy` is `FAILURES`, `OWNER` or `UNKNOWN` | `an_account_locked_by_failures_says_so`, `a_lock_by_hand_says_so_and_the_lock_and_the_unblock_are_audited` |
| N7 no secrets in spans | no `instrument` span captures a password or token; the invite path is `/api/_i/-hidden-` in the trace; the key is `[REDACTED]` in `Debug`; a start-up line says whether public access is configured | `stump_config.rs` tests, `verify-edge.sh` |

Retention (§4): `prune` forgets audit rows after 400 days, revoked devices after 200, and invites 30 days after they expired or
were revoked unless a device that is still alive came from them; it never touches the sign-in log. The housekeeping runs
every 6 hours (the first pass after the first interval; the state file is written at start-up). Pinned by
`tests/access/retention.rs`, which checks each boundary a day (or a minute) either side.

Not done, on purpose: the extra database query that `authorize()` makes per request to ask whether a password must be
chosen (the lead suggested a `LEFT JOIN` in `LoginUser`). It is one primary-key lookup on a table that is empty for everybody but
the people the owner gave a password to; the join can replace it if a profile ever shows it.
