# Public SPA login: Stump's own login screen on the public address

Branch `feat/public-login`, from `43fa3fe50` (= `feat/friend-onboarding` + the restyled server pages; live is `71310fb5a`,
`43fa3fe50` is being deployed). **Design only: no code in this commit.** No migration, no new setting (the existing
`STUMP_EDGE_PUBLIC_SIGNIN` stays the master switch).

## 0. What this is

Today a stranger (or a friend on a new device) who opens the public address gets a small server-made HTML page
(`apps/server/src/routers/public_pages/`), and a signup link opens a second one. The owner finds them inconsistent with the
app. This design serves **the app's real login screen** (`packages/browser/src/scenes/auth/LoginOrClaimScene.tsx`, improved
for friends) at `/`, adds a **join screen** for signup links built from the app's own form components, and keeps the
server-made pages as the fallback and the rollback. For the web app to run without a pass the edge lets a small, exact set of
requests through (§2); everything else stays 404 until a pass exists, as now.

**Surface accounting** (what a stranger can reach after this): static, public, secret-free files (the shell and its flat hashed
bundle files); three answers the **edge itself** gives (constants); and **two** JSON endpoints that are thin wrappers over the
two cores the form endpoints already use (`sign_in_from_internet`, `join_from_internet`). No new capability, no new data, no new
way to be signed in.

## 1. Decisions

**D1. Owner override: no invite-link box on the login screen.** (Owner decision, supersedes item 3 of the request.)
The public login screen has **no** "Have an invite link?" / paste-a-link option, and no input other than username and password.
People who hold a link just open it (`/api/_i/<token>`). The only extra line under the form is **"Forgot your password? Ask the
owner."** (es: "¿Olvidaste tu contraseña? Pregunta al administrador."). Binding on the implementer and the reviewer:
- SPA login screen: no link box, no text about invite links, no link to one. Tests: vitest asserts exactly two inputs and no
  /invite/i text in public mode; the Playwright spec (§7) asserts the same on the real page.
- Server-made sign-in page (`/api/_i/signin`, kept as the no-JavaScript fallback and the rollback page, D7): the `<details>`
  paste box is **removed** from `signin_page()`; `Words.have_link` is deleted in both languages and from the "both languages have
  every string" test; the page keeps the form and the one forgot line. Test: the page has no `name=t` input and no form to
  `/api/_i/`.
- The pass-less front door never shows a link box (it is the SPA).
- **Not covered, left as they are, flagged for the lead:** the "This link doesn't work" page (`invalid_page`) and the "Private
  site" page (`private_page`, shown only while `STUMP_EDGE_PUBLIC_SIGNIN` is off). Both exist to take a link from somebody who
  already holds one, so `GET /api/_i/?t=` (the paste endpoint) stays. If the owner wants those two stripped as well, it is one
  line each.
- The signup-link flow is unchanged for its holder: open the link, get the join screen.

**D2. The app's own front end is served without a pass, proxied from the app, by exact shape.** Navigation: GET/HEAD of exactly
`/`, `/auth`, `/join`. Files: GET/HEAD of flat `/assets/<name>.<ext>` with `ext` in `js css woff2 png webp svg ico` (the built
`assets/` folder holds 1317 top-level entries, among them 339 `.map`, 270 `.gz` and 270 `.br` files and 6 subfolders: maps and compressed twins are **not**
exposed; the shell is `index.html` + 60 modulepreloaded flat `.js` chunks + 2 `.css` + one `.woff2` + the logo). The files come
from the app, not from the edge container: the app always serves the build that matches itself, so there is no extra deploy
step that could go stale. The capture test (D11) is the authority on the final shape; a subfolder file the login screen turns
out to need gets its own exact rule, never a wildcard.

**D3. The edge answers the login screen's three pre-auth questions itself** (only when there is no valid pass; with a pass the
request goes to the app as always): `GET /api/v2/auth/viewer` → 401, `GET /api/v2/claim` →
`{"isClaimed":true,"isPublic":true}`, `GET /api/v2/auth/oidc/config` →
`{"enabled":false,"allowRegistration":false,"disableLocalAuth":false}` (what the app says for a server without OIDC). Reason: the
app then sees no pass-less API request at all except the two POSTs. The claim stub also means a public visitor can never be
shown the "initialize your server" form, whatever the server's state. Cost: three constants in nginx that must match the SDK's
types; the capture test fails if they drift (a failing `oidc/config` is an error screen: it is a `useSuspenseQuery`).

**D4. One sign-in core for the page and the JSON login.** `submit()` in `public_pages/signin.rs` is split: everything after
parsing the form becomes `sign_in_from_internet(state, session, ip, agent, passed_device, username, password) -> PublicSignIn`
(`Refused`, `Failed`, `LimitReached{max}`, `SignedIn{account, pass_cookie: Option<String>}`): `authenticate_public` (guard,
one bcrypt, eligibility), the device made inside the person's limit (`insert_device_within_limit`) or the limit refusal
(`device_limit_reached`: audit + owner warning), `start_session`, the recorded attempt, hiding libraries that appeared since,
and the pass cookie (`pass::mint_pass` + `pass::pass_cookie_header`). The form maps the outcome to HTML, the JSON login to
status codes (§4.8). **Nothing is re-implemented**: the guard, the owner/locked/not-enabled rules, the audit and the warnings
are the same code, so the two doors can't drift and failures through one door count towards the other's budget.

**D5. Signup in the SPA; the link format does not change.** The owner's links stay `…/api/_i/<token>`. `GET /api/_i/<token>` for
a **usable signup link** answers `303 Location: /join#<token>` (when the edge says the SPA login is on, D6); an unusable one
keeps the generic invalid page; device links keep their confirm page. The token travels in the **URL fragment** (never sent to
any server, never in a `Referer`, never in an access log) and the SPA scrubs it with `history.replaceState` before anything else
runs. It is then sent once, in the body of `POST /api/v2/auth/join` with `{token, username, password}`
(`join_from_internet`, the steps of `signup::submit` after parsing: the link is looked up **before** the name, rules, hash outside
the transaction, one transaction spends the use and makes account + first device, session, pass). The SPA checks "repeat the
password" itself.

**D6. A capability header ties the edge and the app together.** The new `nginx.conf` sets `X-Edge-Login: spa` at server level
(`proxy_set_header` replaces any client value, like `X-Edge-Public`). The app turns the SPA behaviours on only when it sees that header **and**
`public_signin` is on: the open list of §2.2, the pass-less JSON login/join, the `/join` redirect of D5. With the **old** nginx.conf
(edge-only rollback, D7) the header is absent and the app behaves exactly as before (server-made pages, 303 never sent), so
neither order of deploy can leave the pair half-working.

**D7. The server-made pages stay (fallback and rollback), unlinked.** Reasons: they are the instant rollback (§8), the device-link
confirm page and the invalid page are part of the pass flow and are not SPA-able without more work, and they are tested. The
front door and the SDK redirect no longer point at them; server-made pages and the edge's 404 body link to `/` (which is the SPA
with the new edge and the server-made sign-in with the old one, so the link is right in both worlds). The only change to
them is D1 and the shorter wording already committed. Rollback ladder in §8.

**D8. A CSP for the pass-less documents only; a hard navigation after sign-in.** The shell served without a pass carries a
hash-based CSP (§3.5). A CSP for the whole signed-in app is out of scope and risky (reader iframes, PDF workers, blobs). So after
a successful sign-in or signup in the SPA the page does `window.location.assign(<safe redirect>)`: the app then reloads **with the
new pass**, gets the shell without the pass-less CSP, and no signed-in code ever runs under it. Only on the public address
(`isPublic`); at home the login behaves as today (client-side navigation).

**D9. One list, two copies, one fixture.** The edge's `$edge_open` map (primary gate) and the app's `is_open_public_request`
(second gate) must agree. Both are tested against `deploy/public-edge/open-paths.json` (`{method, path, edge, app}` rows incl.
the 404 lookalikes): `verify-edge.sh` checks the `edge` column by curl, a Rust test checks the `app` column
(`include_str!`). Adding a path means adding a row; a path in one list but not the fixture fails a test.

**D10. Open redirect closed.** The login screen's `?redirect=` is used today as `window.location.href = redirect` when it contains
`/api`; on the public address a crafted link could bounce a freshly signed-in friend to another site. `safeRedirect()`: only
`^/(?![/\\])` (a same-origin path), else `/`. Applied on the public screens and, being harmless, at home too.

**D11. Verify the allowlist with a real browser, don't guess it.** A Playwright spec (`apps/web/tests/specs/public-login.spec.ts`,
the repo already has `playwright.config.ts`, base URL from `STUMP_BASE_URL`) drives the pass-less flow against a stack with the new
edge, records every request and every CSP violation, and asserts: each request is in the fixture's edge-open/stub rows, nothing
goes to another origin, zero violations, then the app works with the pass it got. It also tells us whether the shell's
core-event websocket (`useCoreEvent` is called in `AppLayout` before anyone is signed in) retries on `/api/graphql/ws`: it
answers 404 (not allowlisted) and gives up after its retries; harmless, but the implementer should gate the hook on a signed-in
user so the page doesn't spend requests on it (small change, test).

**D12. No migration, no new setting.** `claim` gains a JSON field; nothing else is stored.

## 2. The exact surface without a valid pass

"No valid pass" = no pass, a forged/expired one, or one the app revoked (`$edge_has_pass = 0`).
Decided on the **raw request line** (`$request_uri`, with the method), case-sensitive, never on nginx's cleaned `$uri` (the R3
rule): dot segments, `%2e %2f %5c %00`, `//`, trailing slashes and case variants are 404.

### 2.1 The edge (primary gate)

| # | Method | Raw path (exact) | Answer | Why | Limit |
|---|---|---|---|---|---|
| 1 | GET, HEAD | `/` (+ optional `?query`) | the app's `index.html`, no rewrite | the front door | `pub_signin_page` 30/min/client, burst 10 |
| 2 | GET, HEAD | `/auth` (+ `?query`) | the same shell | the login route: SDK redirect, reload, back | same |
| 3 | GET, HEAD | `/join` (+ `?query`) | the same shell | the signup route (the token is in the fragment) | same |
| 4 | GET, HEAD | `/assets/<name>.<ext>`, `<name>` = `[A-Za-z0-9][A-Za-z0-9._-]{0,119}`, ext = `js css woff2 png webp svg ico`, **no query** | the file, from the app | the shell's chunks, CSS, font, logo, favicons | `pub_req` 20/s burst 300, `pub_conn` 30 (the existing safety net) |
| 5 | GET | `/api/v2/auth/viewer` (+ `?query`) | **edge stub** `401 {"status":401,"message":"Unauthorized"}` | boot: "who am I" → 401 sends the app to `/auth` | `pub_req` |
| 6 | GET | `/api/v2/claim` | **edge stub** `200 {"isClaimed":true,"isPublic":true}` | the login screen's claim check | `pub_req` |
| 7 | GET | `/api/v2/auth/oidc/config` | **edge stub** `200 {"enabled":false,"allowRegistration":false,"disableLocalAuth":false}` | the login screen's OIDC check (suspense) | `pub_req` |
| 8 | POST | `/api/v2/auth/login` (+ `?query`, ignored) | the app: JSON sign-in | the sign-in | `pub_signin_post` 6/min/client burst 4 + `pub_signin_all` 30/min burst 10, **shared with the form**; 4 KB |
| 9 | POST | `/api/v2/auth/join` | the app: JSON signup | signup | `pub_signup` 3/min burst 3 + `pub_signup_all` 2/min burst 10, **shared with the form**; 4 KB, 405 for other methods |

Unchanged and still open: `/api/_i/…` invite pages (GET/POST), `GET/POST /api/_i/signin`, `POST /api/_i/join`,
`/assets/manifest.webmanifest`, `/assets/favicon-<w>x<h>.png`, reading-app keys (`/opds/<key>/v1.2/…`).
**Stubs 5–7 are GET only** (HEAD, POST are 404). With a valid pass, 5–7 go to the app (real answers; `claim` then says
`isPublic: true` too, §4.6) and 1–4 are served exactly as today (no CSP).

Deliberately **404 without a pass** (named because the web app asks for some of them): `/sw.js` (the service worker is
registered only after the page loaded; the failed registration is a silent console error and a signed-in friend gets it with the
pass), `/api/graphql`, `/api/graphql/ws`, `*.map`, `*.gz`, `*.br`, any `/assets/<subfolder>/…`, `/favicon.ico`, `/index.html`,
`/api/v2/auth/{logout,refresh-token,register}`, `/api/v2/auth/oidc/{authorize,callback}`, every other SPA route (a pass-less `/books/…` shows the 404 page whose link goes to `/`;
serving the shell for deep links is a possible later step, not needed now).

### 2.2 The app (second gate)

`is_open_public_request(method, path, public_signin, spa_login)` in `middleware/edge.rs`, extending `is_open_public_path` (invites,
manifest, favicons, reading keys: unchanged, any method). When **both** flags are true it also opens: GET/HEAD `/`, `/auth`,
`/join`; GET/HEAD `/assets/<same shape>`; POST `/api/v2/auth/login`, POST `/api/v2/auth/join`. The three stubs are **not** in
the app's list: they never reach it without a pass, and if the stubs were ever missing the shell would fail closed (an error,
not a hole). `public_signin` off or no capability header → none of the new rows; and for GET/HEAD `/` without a device the
app then answers the old `private_page` instead of a bare 404 (what the old front-door rewrite produced), so the documented
back-out ("set `STUMP_EDGE_PUBLIC_SIGNIN=false`") still ends at a page that says what to do. A pass the app has **revoked**
but the edge still honours (the edge follows the app's state file within a second) is treated as no pass on POST login/join,
as the middleware already does for `/api/_i/` (a person who just removed a device can sign in again without a 403 in between).

## 3. The edge: `nginx.conf` and `stump-gate.js`

### 3.1 Maps
- `$edge_open` = `map "$request_method:$request_uri"` with the four regexes of rows 1–4, 5–7, 8–9 (anchored, no `.*`,
  `[^#\s]*` for queries as the existing `$edge_exempt`). It **replaces `$edge_front`**; `$edge_deny` keeps its shape with
  `$edge_open` in the last slot (valid pass → 0; exempt → 0; open → 0; else 1). `$edge_signin_ok` and the front-door `rewrite`
  go away (the sign-in page is reached by its own exempt path only).
- `$shell_csp`: `map $edge_has_pass` → the policy of §3.5 when 0, `""` when 1 (`add_header` omits an empty value, so a request
  with a pass gets no CSP, as today).
- Zone keys that apply only to pass-less requests: `$shell_get_key` (`GET|HEAD` and no pass → `$pub_client`, else `""`),
  `$login_nopass_key` / `$login_nopass_all` (no pass → `$pub_client` / `$server_port`, else `""`). The sign-in page's GET zone
  (`pub_signin_page`) is reused for rows 1–3 so a signed-in friend's page loads are never counted.

### 3.2 Locations
- `location = /`: the front-door rewrite is removed; `proxy_pass http://stump;` with the CSP, the limits above.
- `location = /auth` and `location = /join`: same block (exact matches; with a pass they would fall to `location /` anyway, the exact
  blocks only add the pass-less limits and the CSP).
- `location ~ ^/assets/[A-Za-z0-9][A-Za-z0-9._-]{0,119}\.(?:js|css|woff2|png|webp|svg|ico)$` below the existing manifest/favicon block
  (regex locations match in file order, so the old exact block still wins for those); the server-level `$edge_deny` check has already
  passed; `proxy_pass http://stump;` (a location's own `add_header` replaces the server-level set: repeat HSTS, nosniff,
  `Referrer-Policy`, `X-Frame-Options` as the other locations do).
- The three stubs (`location = /api/v2/auth/viewer` etc.): `default_type application/json;` `if ($edge_has_pass = 0) { return 401|200 '<exact body>'; }`
  else `proxy_pass http://stump;`; `add_header Cache-Control "no-store" always;` plus the repeated security headers. They must
  never carry `X-Edge-Sign-In`: a `401` on `viewer` is the app's own signal to go to `/auth`, and the SDK's 404 interceptor
  would otherwise fight it.
- `location ~* ^/api/v2/auth/login$`: for no pass, `limit_req` through the `$login_nopass_*` keys using the **sign-in form's
  zones** (`pub_signin_post`, `pub_signin_all`), so alternating form and JSON can't double the budget; with a pass, the existing
  `pub_login`, `pub_login_all` and the 8-a-day device cap. `client_max_body_size 4k`. The error log is `crit`-only as in the other
  sign-in locations; nothing of a request body is ever logged.
- `location = /api/v2/auth/join`: new, `405` unless POST, `pub_signup` + `pub_signup_all`, `4k`, same logging rule as `/api/_i/join`.
- The server-level 404 for a request without a pass keeps its text; its link goes to `/` (`<a href="/">Sign in</a>`), and
  `X-Edge-Sign-In: 1` stays on those 404s only.

### 3.3 `stump-gate.js`
`signin(r)`: `POST` without a valid pass is no longer a refusal: after the existing "state not ready → `0`" (fail closed), no valid
pass returns `'1'`; the app's guard and the zones are the budget for it. With a pass: the per-device daily counter, unchanged.
`vectors.json` is untouched (the pass format does not change).

### 3.4 Headers added
`proxy_set_header X-Edge-Login spa;` at server level (D6). Responses of rows 1–3: `Cache-Control` as the app sends it
(`no-cache`), `X-Frame-Options DENY`, `Referrer-Policy same-origin`, HSTS, nosniff, `Permissions-Policy`, and the CSP.

### 3.5 CSP of the pass-less documents (rows 1–3 only)
```
default-src 'none';
script-src 'self' 'sha256-CmpOl6kcMHciMT/+yWjgPwf73VaMVddSqn6/Wlw6/lk=' 'sha256-ckz9D9aoRbeD8jMHl1wcCyDBnLLE2ew4WOs0tSdoMWU=';
style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self';
manifest-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; object-src 'none'
```
The two hashes are those of the two inline bootstrap scripts of the built `index.html` of `stump:fork-71310fb5a` (theme and font
from `localStorage`); the module script, chunks and CSS are `'self'`. `'unsafe-inline'` for **styles** only (the shell's inline `<style>`,
React style attributes, framer-motion). **Hash drift** (a later SPA build changes an inline script): `verify-edge.sh --live` fetches
the pass-less shell, hashes every inline `<script>` and fails unless each hash is in the header. The failure mode of drift is
mild (the early theme/font bootstrap is blocked: a flash of the default theme), never a failed sign-in. The Playwright run
adds any further directive the login/join screens turn out to need (a `securitypolicyviolation` is a failure). A browser whose
service worker answers the shell from its cache has no CSP on that page: the same as the signed-in app today.

## 4. The app

### 4.1 `EdgeRequest` and the middleware
`graphql::data::EdgeRequest` gains `spa_login: bool` (true when `X-Edge-Login: spa` came with the marker header; `Default` false, so
every construction site and test keeps compiling). `edge_middleware` fills it, applies `is_open_public_request`, answers the
private page for GET/HEAD `/` when `public_signin` is off, and keeps `Seen::Revoked` as "no device" for POST login/join.

### 4.2 `sign_in_from_internet` (D4)
Signature in §D4. The existing `submit()` shrinks to: switch/key checks, same-origin and size caps, call the core, map the outcome to
`signin_page/limit_page/too_many_page/303`. The core keeps every property the module doc promises (same answer for everything that is
not a sign-in, never a lock, cheap gates before the password, the failure written after the answer, a pass already in the browser
is never replaced).

### 4.3 `login()` (JSON)
New branch at the very top (after the OIDC-only refusal, **before the account lookup**, so an unknown name and a known one do the
same work): `via_public && edge.spa_login && public_signin && edge.device.is_none()` → same-origin check (`post_is_same_origin`;
a miss is the generic 401 with no work), username ≤ 64 chars and password ≤ 128 bytes (as the form) → `sign_in_from_internet`.
`generate_token` / `create_session` query parameters are **ignored** on this path: a browser behind the edge gets a session
and a pass, never a JWT. The handler returns `Response` (not `Json<…>`) so it can append `Set-Cookie`. Every other case (a
pass present, home, tailnet) runs the existing code untouched.

### 4.4 `POST /api/v2/auth/join`
New route beside `login` in `routers/api/v2/auth.rs`; body `{token, username, password}`; `join_from_internet` is `signup::submit`
minus the HTML: same gates (`public_signin_on`, same-origin, caps), link before name, rules, bcrypt outside the transaction,
`redeem_signup_link` (one transaction, preset, first device when no pass is in the browser), `start_session`, the recorded
attempt, pass cookie. The form handler and this one both call it.

### 4.5 `GET /api/_i/<token>`
`invite.rs::show`: a usable signup link and `edge.spa_login` → `303 /join#<token>` (no body that repeats the token); the old
form page when the header is absent. Unusable links, device links, `?t=` pastes: unchanged.

### 4.6 `claim`
`ClaimResponse` gains `is_public: bool` (`isPublic`) = `edge.via_public`. At home it is `false`; the SDK type makes it optional.

### 4.7 The server-made pages
D1 removal in `signin.rs`/`mod.rs`; `invalid_page`'s "Sign in" link and the 404 body link go to `/`. Their tests move with them (the
both-languages test loses `have_link`; the sign-in page test asserts no paste box).

### 4.8 Answers of the JSON endpoints (all JSON, `Cache-Control: no-store`)
| Outcome | `POST /api/v2/auth/login` | `POST /api/v2/auth/join` |
|---|---|---|
| unknown name, wrong password, the owner, not enabled for the internet, locked, no password, cross-site, oversized | **401** `{"status":401,"message":"Unauthorized"}`, byte-identical, same work | – |
| the guard refuses (client, name or everybody) | **429**, `Retry-After: 300`, "Too many attempts. Wait a few minutes." (localised by `Accept-Language`) | – (the edge's signup zones) |
| right password, all devices used | **403** "You already use 3 devices. Remove one in Settings → My devices, or ask the owner." (localised; audit row + owner warning as today) | – |
| link unusable | – | **404** "This link doesn't work. Ask for a new one." |
| name or password rule | – | **422** the rule's text (the same localised strings the form shows) |
| name taken | – | **422** "That username is taken" |
| ok | **200** the user JSON (`LoginResponse::User`, with the must-change-password flag and avatar as `login()` builds them) + `Set-Cookie` pass (when a device was made here) + the session cookie | same |

The bodies of 403/422/429 are the API's `{status, message}` object; the SPA reads `message`.

## 5. The web app

- **`LoginOrClaimScene`** (public mode = the claim says `isPublic`): the same logo, "Stump" gradient heading, `Form`, `Input`,
  `PasswordInput`, `Button` as at home. Hidden: the claim alert, OIDC buttons, the server picker. Added: under the form the muted line
  `authScene.public.forgot` (**D1: nothing else, no link box**). Errors inline (`Alert variant="destructive"`, not a toast): 401 →
  `authScene.public.wrongCredentials` "Wrong username or password."; 429 → `tooMany` "Too many attempts. Wait a few minutes.";
  403/422 → the server's `message`; anything else → `generic` "Couldn't sign in. Try again.". Success → `window.location.assign(safeRedirect(redirect))` (D8).
  At home nothing changes. `renderError` is also fixed to read `data.message` (the API's error body is an object; the existing cast
  to string would crash React for a locked account at home too).
- **`JoinScene`** (route `/join` in `AppRouter.tsx`): reads the token from `location.hash`, scrubs it with `history.replaceState`
  first, shows "Create your account", username, password, "Repeat the password", the hint "Username: 3 to 32 characters. Password: at least 10."
  and "Create my account"; client checks (match, minimum lengths) and the server's `message` for the rest; no token or a 404 →
  "This link doesn't work. Ask for a new one." (no paste box, D1); success → hard navigation to `/`; "Have an account? Sign in" → `/auth`.
  Not on the public address (`isPublic` false) → redirect to `/auth`.
- **SDK/client**: `ClaimResponse.isPublic?`, `AuthAPI.join()`, `useLoginOrRegister` returns `isPublic`; `EDGE_SIGN_IN_PAGE` becomes
  `/auth`, and the interceptor also stays quiet on `/join` (and `/api/_i/` as now), so there is no redirect loop.
- **`useCoreEvent`** does not subscribe until a user is signed in (D11).
- **i18n** (`en-US.json`, `es-ES.json`, under `webApp.authScene.public.*` and `webApp.authScene.join.*`): `forgot`, `wrongCredentials`, `tooMany`,
  `generic`, `deviceLimit` (fallback when a 403 has no message), and the join title, labels, rules hint, button, mismatch, bad link,
  "Have an account? Sign in". Spanish mirrors the wording already in `public_pages/mod.rs`. Other locales fall back to en-US.

## 6. Security review

| Concern | How it is handled |
|---|---|
| Password guessing | the same zones and the same `SigninGuard` as the form (per client, per name with growing waits and no permanent lock, global budget), one bcrypt per request, caps before hashing; no per-device budget exists without a pass, so none is pretended |
| Account enumeration | one 401 body for every non-sign-in, same work and timing (the core), stubs constant; join looks the link up before the name |
| The owner / admins / not-enabled accounts | refused inside `authenticate_public` as today; and on every later request by the friend rule of the middleware |
| Login CSRF | `post_is_same_origin` first, `application/json` required (a cross-site form can't send it without a CORS preflight, which the app's origin list refuses) |
| XSS in the pass-less screens | React escaping, no HTML injection on those scenes (the implementer greps for `dangerouslySetInnerHTML` in the auth scenes), the hash CSP, `frame-ancestors 'none'` |
| Open redirect | `safeRedirect` (D10) |
| Link token | fragment only, scrubbed at once, one POST body, never in a log (`/join` has none; `/api/_i/<token>` is still masked in the edge log) |
| Information from the open files | the web app's JS is public; no secrets live in it (checked in the capture); no source maps; the claim state is hidden by the stub; the bundle hashes fingerprint the build: accepted |
| Resource use | rows 1–4 are cheap, cached (`immutable`) and under the existing safety net; the two POSTs under the form's zones |
| WebSocket and GraphQL | 404 without a pass, as now |
| Old edge + new app, new edge + old app | the capability header (D6) makes the old pair behave as before; a new edge with an old app serves a shell whose files the old app also serves, but the old app has no pass-less JSON login: sign-in fails closed (**deploy the app first**, §8) |
| CSP hash drift | `verify-edge.sh --live` check; mild failure mode |
| Residual | the global failure budget can still be used by a botnet to slow *new-device* sign-ins for a few minutes (accepted in the onboarding design; known devices use passes) |

## 7. Tests

- **Rust unit** (`middleware/edge.rs`): `is_open_public_request` against the `app` column of `open-paths.json` for all four flag
  combinations (new rows only with both flags); `loggable`/`is_cross_site` untouched.
- **Rust integration** (`apps/server/tests/access/`, new `public_login.rs`): JSON login without a pass: right password → 200, a pass cookie
  that verifies and belongs to a **new device of that person**, a session, the attempt recorded; wrong password, unknown name, the
  owner, a not-enabled account, a locked one, an OIDC-only one → the **same bytes**; the guard counts a JSON failure and a form
  failure in one budget (alternate them: refused at the same total); 429 shape; the fourth device → 403 message + audit +
  `DEVICE_LIMIT_HIT`; a request that already carries a pass signs in without making a device; cross-site and oversized → generic 401
  with no bcrypt; no `X-Edge-Login` header → the old behaviour (404 for the pass-less POST); `generate_token=true` pass-less still a
  session. JSON join: valid → 200 + device + cookie + one use spent; used up / expired / revoked / unknown token → the same 404; rule
  failures → 422 with the localised text; name taken; the budget race test of the form, run against both doors. `GET /api/_i/<signup>`
  → 303 `/join#<token>` with the header, the old page without it, invalid page for unusable links, device links unchanged. `claim` →
  `isPublic` true via the edge, false at home. Middleware: switch off → `GET /` shows `private_page`, everything new 404; revoked
  pass + POST login → treated as no pass. D1: the server sign-in page has no `name=t` input and no `/api/_i/` form.
- **Edge** (`verify-edge.sh`, static mode; the stub upstream shows what reached it): rows 1–9 pass-less 200/401/stub bodies exact
  and **not forwarded** (stubs); `X-Edge-Login: spa` reaches the app and a client's own value does not; CSP present pass-less and
  absent with a pass; each lookalike of §2.1 is 404 (`/auth/`, `//auth`, `/%61uth`, `/Auth`, `/auth/../x`, `/join/x`, `/index.html`,
  `/assets/x.js.map|.gz|.br|.json|.html`, `/assets/fonts/x.woff2`, `/assets/../x.js`, `/assets/%2e%2e/x.js`, `/assets//x.js`,
  `/assets/x.js%00`, a 121-character name, a name starting with `.` or `-`, `POST /assets/x.js`, `HEAD`/`POST` of a stub, `GET /api/v2/auth/login`,
  `GET /api/v2/auth/join`); the old 404 list stays (graphql, ws, upload, `/api/v2/ping`, `/sw.js`, `/favicon.ico`, `/settings`, opds without a key,
  `/api/v2/auth/{logout,refresh-token,register}`); zones: 7th pass-less POST in a minute 429 with form and JSON interleaved, 4 KB cap
  413, signup zone; a request with a pass still gets the daily cap; the revoked-device and "front door" checks are rewritten for the new
  front door (no `uri=/api/_i/signin` rewrite any more); every entry of `open-paths.json` agrees. `--live`: the CSP hash-drift check.
  The count of checks in `DEPLOY.md` is updated.
- **Web** (vitest): public login scene (exactly two inputs, no /invite/i text, forgot line, hidden claim/OIDC/servers, error mapping 401/429/403
  with `data.message`, success → `location.assign` with a sanitised redirect, home mode unchanged); `safeRedirect` table (`/x`, `//evil`,
  `/\evil`, `https://evil/api`, `javascript:`, empty); `JoinScene` (hash read then scrubbed before the first request, validation, 404,
  422, success, not-public redirect); SDK interceptor target and loop guard; `useCoreEvent` gating; i18n parity en/es for the new keys.
- **Browser** (Playwright, D11): against the throwaway stack in §8: friend credentials in env; records requests and violations; asserts
  the subset rule, zero violations, two inputs and no invite text, the forgot line, login → home with the pass; then a second context with the pass
  cookie only: `viewer` 200.

## 8. Deploy and rollback

No migration; image + edge files + one runbook section in `DEPLOY.md` ("Public login").
1. Pre-flight as in "Friend onboarding": `trust_proxy_headers = true`, 0 jobs, backups (`config.backup-…-db`, `edge/`, compose).
2. **App first** (it is backward compatible with the old edge): new image on the shared `x-stump-image` line, recreate **both**
   `stump-fork` and `stump-intake-probe`, probe's ready line, home/tailnet 200, pass-less `GET /` still the server-made sign-in (the old
   edge rewrites it).
3. **Edge**: stage `edge-new/{nginx.conf,stump-gate.js}`, `nginx -t` in a throwaway container, `verify-edge.sh` (static), install with
   dated `.bak-…-before-publogin` copies, `nginx -s reload` (no recreate), `verify-edge.sh --live` incl. the hash-drift check.
4. **Walk-through** with a throwaway link and account, through the real browser flow and the Playwright spec (signup link → join →
   signed in; sign out; sign in on another browser; the 4th device message; wrong password message; clean-up as in the onboarding runbook).
5. Tell the owner.

**Rollback ladder** (fastest first):
1. *Edge only*, one second, no restart: restore `nginx.conf` and `stump-gate.js` from the `.bak` files and reload. The capability header
   disappears, so the app returns to the server-made pages by itself; the front door is the server-made sign-in page again.
2. *App switch*: `STUMP_EDGE_PUBLIC_SIGNIN=false` (set it, do not delete the line), recreate both containers: the new open list closes and `/` shows the
   "Private site" page.
3. *Image*: back to `stump:fork-43fa3fe50` (or `71310fb5a`) on the shared line, recreate both. No data to restore.
4. `tailscale funnel --https=8443 off` while working.

## 9. Order of work for the implementer (a commit each, tests with it)
1. `refactor(access)`: `sign_in_from_internet` extracted from `submit()`, no behaviour change (existing 108 tests stay green); D1 removal of the
   link box from the server sign-in page with its tests.
2. `feat(access)`: `EdgeRequest.spa_login`, `is_open_public_request` + fixture + middleware (private page, revoked-as-none); `claim.isPublic`.
3. `feat(access)`: pass-less JSON login branch + §4.8 answers; `join_from_internet`, `POST /api/v2/auth/join`, the `303 /join#<token>`.
4. `feat(edge)`: `nginx.conf` (maps, stubs, shell locations, assets, zones, CSP, header, `X-Edge-Login`), `stump-gate.js`, `verify-edge.sh`.
5. `feat(web)`: SDK/client/i18n; public login mode; `safeRedirect`; `useCoreEvent` gating.
6. `feat(web)`: `JoinScene` and its route.
7. `test(web)`: the Playwright spec; the allowlist/CSP findings recorded in this file's appendix; `DEPLOY.md` section; FORK.md row.
Builds and tests on g3 only, one package at a time (`g3-cargo stump-check-target-publogin …`).

## 10. For the reviewer to confirm or change
1. Stubs at the edge (D3) instead of letting the three GETs reach the app.
2. Token in the URL fragment with a 303 from the old link (D5) instead of a `/join/<token>` path.
3. The capability header (D6) and its extra state; the alternative is an edge-only rollback that breaks signup links until the app is also rolled back.
4. Scope of D1: `invalid_page` and `private_page` keep their paste boxes.
5. `.map`/`.gz`/`.br` and subfolders closed; the capture decides what else, with exact rules.
6. `generate_token` ignored on the pass-less path; deep links not served pass-less.
7. Hard navigation after sign-in (D8) costs one more load of the shell, only on the public address.
