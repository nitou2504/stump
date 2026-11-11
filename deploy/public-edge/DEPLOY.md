# Public access for Stump: deploy, check, roll back

Branch `feat/public-access-admin` (design: `research/2026-11-11-stump-public-access-design.md`, with the
squad lead's review applied). The owner manages invites, devices, sign-in blocks and passwords in
**Settings → Public access** (the server owner, from the LAN or the tailnet: the page and its API refuse
requests that came through the public edge, and the edge answers 404 for `/settings/access`). This file is the
ordered deploy of the files in `deploy/public-edge/`. Nothing here touches the live system until you run it.

| piece | change |
|---|---|
| app (`stump-fork` image) | the admin page and its GraphQL API, the invite pages `/api/_i/…`, the edge middleware (who reached the app, with which pass), the owner/registration refusals through the edge, "must choose your own password", **one migration** (`m20261113_000000_public_access`: 5 new tables) |
| `stump-fork` compose | `STUMP_EDGE_KEY` (new to the app; from `./.env`), `STUMP_EDGE_PUBLIC_URL`, `STUMP_EDGE_STATE_DIR=/edge-state`, `STUMP_EDGE_REQUIRE_PASS`, and the `./edge-state` volume |
| public edge | `edge/{nginx.conf,stump-gate.js}`: follows the app's state file once a second; sends `X-Edge-Public` on every public request and `X-Edge-Pass` for a valid, unrevoked pass; invite location with its own headers; no tokens in logs |
| what stays | passes already handed out keep working (same format); `edge-common/invite.sh stump …` and `edge/revoked.map` stay as terminal break-glass; the old `edge-common/gate.js` stays on disk, unused, and `stump-edge` keeps its `../edge-common` mount until you choose to drop it (the old edge needs it until step 7) |

**Order matters.** The new edge refuses every pass until the app has written the state file, and the app must not
demand a pass until the new edge sends it. So: app first (`STUMP_EDGE_REQUIRE_PASS=false`), then the edge, then the
strict switch, then (owner's decision) Funnel.

Run everything as `sthefano` from `/mnt/data2/containers/stump-fork` unless a path says otherwise. Build on `l5`
(`AGENTS.md`); one build at a time. Stump's Funnel is **off** today: this deploy has no public traffic to protect, but
it follows the order that keeps it safe with Funnel on.

**`S` is the checkout of the revision you are deploying**: the folder that holds `deploy/`, `crates/` and `docker/`.
Every command below that copies a file out of the repository starts from `$S`. It is **not**
`/mnt/data2/Code/stump`: that checkout sits on another branch and has no `deploy/` folder, so a copy from there
fails, and an old file stays live. Set it once per shell, and check it before you start:

```sh
S=/mnt/data2/Code/worktrees/stump/golive      # or wherever the revision of the image you deploy is checked out
test -f $S/deploy/public-edge/nginx.conf && git -C $S log --oneline -1     # its HEAD is the revision of the image
```

## 0. Pre-flight

```sh
cd /mnt/data2/containers/stump-fork
tailscale funnel status | grep -c 8443 || true       # 0: Funnel is off for Stump (expected)
docker port stump-fork; echo "(nothing above = no host port: only the edge reaches the app)"
sqlite3 -readonly config/stump.db "select count(*) from jobs where status in ('RUNNING','QUEUED','PAUSED')"   # 0
grep -c '^STUMP_EDGE_KEY=' .env                       # 1: the key exists (the brute-force design made it)
ls -l .env | awk '{print $1}'                         # -rw------- (mode 600)
```

## 1. Backups (for the rollback)

```sh
cd /mnt/data2/containers/stump-fork
TS=$(date -u +%Y%m%dT%H%M%SZ); B=config.backup-$TS-db; mkdir -p $B
sqlite3 config/stump.db ".backup $B/stump.db"; sqlite3 -readonly $B/stump.db 'pragma quick_check'     # ok
cp -a config/Stump.toml $B/Stump.toml                 # the image rewrites it at every start: restore it on a rollback
# the two migrations of this build, each with its own down (run in the reverse order: intake first, then access)
M=$S/crates/migrations/manual-down
cp $M/m20261114_000000_intake_approval.sql $B/manual-down-1-intake.sql
cp $M/m20261113_000000_public_access.sql   $B/manual-down-2-access.sql
cp -a docker-compose.yml docker-compose.yml.bak-$(date +%Y%m%d)-public-access
cp -a edge $B/edge && cp -a .env $B/env.pre-public-access && chmod 600 $B/env.pre-public-access
echo $B
```

## 2. Image

Build on `l5` and load it on `g3` as `AGENTS.md` says (`stump:fork-<rev>`), then put it in the compose file with the
usual comment (what changed, **migrations m20261113_000000_public_access and m20261114_000000_intake_approval**, rollback image and the manual-down SQL).

## 3. The state folder (owned by uid 1000, the `PUID` the app runs as)

The app writes `state.json` here; the edge reads it (uid 101, read-only). A folder, not a file: an atomic replace is
followed through a directory mount, a single-file mount would keep the old inode.

```sh
mkdir -p ./edge-state && chmod 755 ./edge-state && ls -ld ./edge-state     # drwxr-xr-x sthefano sthefano
```

## 4. Compose

```sh
python3 $S/deploy/public-edge/apply-compose.py docker-compose.yml --dry-run | diff docker-compose.yml - | head -40
python3 $S/deploy/public-edge/apply-compose.py docker-compose.yml      # backup: docker-compose.yml.bak-<ts>
docker compose config -q && echo compose-ok            # prints nothing else (no secrets)
```

This sets `STUMP_EDGE_REQUIRE_PASS=false` on purpose (step 8 turns it on).

It **adds** the read-only state mount to `stump-edge` and **keeps** the `../edge-common` mount: the old edge's
`nginx.conf` imports `gate.js` from it until step 7, so an unscoped `docker compose up -d` or a
`--force-recreate` before then still starts the old edge instead of failing on the missing file. Nothing loads that
mount after step 7; it is harmless, and can be deleted from the compose file the next time the edge is recreated.

## 5. Start the app (the old edge keeps serving)

```sh
docker compose up -d --no-build stump-fork
for i in $(seq 1 30); do [ "$(curl -s -o /dev/null -w %{http_code} http://127.0.0.1:10801/api/v2/ping)" = 200 ] && break; sleep 3; done
docker ps --filter name=stump-fork --format '{{.Image}} {{.Status}}'
sqlite3 -readonly config/stump.db "select max(version) from seaql_migrations"     # m20261114_000000_intake_approval (the latest; public_access is the one before it)
python3 -c "import json;d=json.load(open('edge-state/state.json'));print(sorted(d), d['revoked'])"   # ['budget','revoked','v','written'] []
docker logs --since 2m stump-fork 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E 'Public access|panic| ERROR ' | head       # "Public access is configured", clean
grep -c 'edge' config/Stump.toml || echo "no [edge] in Stump.toml (the key and the section are never written)"   # 0
echo "LAN $(curl -s -o /dev/null -w %{http_code} http://192.168.0.17:10801/)  tailnet $(curl -s -o /dev/null -w %{http_code} https://g3.tail3e401.ts.net:10802/api/v2/ping)"
```

Then sign in as the owner on the tailnet (`https://g3.tail3e401.ts.net:10802`) and open **Settings → Public access**:
the status strip says it is ready (key, address, state folder set), and the old passes appear under *Devices* as
*Legacy* once they have been used. The old edge is still in front, so existing passes keep working.

## 6. Stage and validate the new edge files

Stage them in their own folder, **not** over the live ones: if the edge container restarted between this step and the
next it would otherwise load a new `nginx.conf` that nobody has validated yet (the live edge keeps serving the files in
`edge/` until step 7).

```sh
N=$PWD/edge-new; mkdir -p $N && chmod 755 $N
install -m 644 $S/deploy/public-edge/{nginx.conf,stump-gate.js} $N/
cp edge/revoked.map $N/revoked.map && chmod 644 $N/revoked.map          # keep the break-glass revocations
docker run --rm --network stump-fork_default -e EDGE_KEY=$(openssl rand -hex 32) -e EDGE_COOKIE=__Host-g3-stump \
  -v $N:/etc/nginx/edge:ro -v $PWD/edge-state:/etc/nginx/state:ro --entrypoint nginx nginx:alpine -t -c /etc/nginx/edge/nginx.conf
# and the real config against a stub (85 checks, ~1 min, nothing live is touched):
bash $S/deploy/public-edge/verify-edge.sh
```

## 7. Switch the edge

```sh
# copy and recreate back to back (one command line): the recreate is what loads the new nginx.conf and stump-gate.js
D=$PWD/edge; N=$PWD/edge-new
install -m 644 $N/nginx.conf $N/stump-gate.js $D/ && docker compose up -d --force-recreate stump-edge     # a changed volume needs a recreate, not a reload
sleep 5; docker logs --since 20s stump-edge 2>&1 | grep -iE 'emerg|error|state file' || echo "edge log clean"
docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf 2>&1 | tail -1
```

Checks against the live loopback port of the public listener (`127.0.0.1:10803`):

```sh
E=http://127.0.0.1:10803
echo "no pass: $(curl -s -o /dev/null -w %{http_code} $E/api/v2/ping)"                 # 404
echo "invite page: $(curl -s -o /dev/null -w %{http_code} $E/api/_i/AAAAAAAAAAAAAAAAAAAAAA)"   # 200 (the app's confirmation page)
echo "admin page: $(curl -s -o /dev/null -w %{http_code} $E/settings/access)"          # 404
echo "home still fine: LAN $(curl -s -o /dev/null -w %{http_code} http://192.168.0.17:10801/)  tailnet $(curl -s -o /dev/null -w %{http_code} https://g3.tail3e401.ts.net:10802/api/v2/ping)"   # 200 200
echo "Funnel guard: $(curl -s -o /dev/null -w %{http_code} -H 'Tailscale-Funnel-Request: ?1' http://127.0.0.1:10801/api/v2/ping)"        # 404 (the tailnet check above must stay 200)
```

**An old pass still works** (made by `invite.sh`, or the old edge): it shows up in *Devices* as *Legacy*:

```sh
L=$(/mnt/data2/containers/edge-common/invite.sh stump deploy-old 1 | sed 's|https://g3.tail3e401.ts.net:8443|'$E'|')
C=$(curl -s -D- -o /dev/null -X POST -H 'Sec-Fetch-Site: same-origin' --data-urlencode "t=$L" $E/api/_i/ | grep -i '^set-cookie' | sed -E 's/^[Ss]et-[Cc]ookie: ([^;]+);.*/\1/')
echo "${C%%=*}"; curl -s -o /dev/null -w "with pass: %{http_code}\n" -b "$C" $E/api/v2/ping                  # __Host-g3-stump ; 200
```

(The loopback listener is plain HTTP: a `Secure` cookie is kept by curl here only because `-b "$C"` sets it by hand.)

## 8. Make the app strict

Only now (the edge sends `X-Edge-Pass`, and step 7 passed):

```sh
python3 $S/deploy/public-edge/apply-compose.py docker-compose.yml --require-pass
docker compose up -d --no-build stump-fork                 # when no job is running
# a public request with no pass is refused by the app itself too (the edge is not the only gate)
docker exec stump-edge wget -qO- --header 'X-Edge-Public: 1' http://stump-fork:10801/api/v2/ping; echo   # empty / 404
```

Repeat the checks of step 7 and the walk-through below.

## 9. Walk-through (once, before telling anybody)

1. **Settings → Public access → Invite links:** person (a friend's account), device name, 3 days, 3 uses → **Create
   link** → **Copy**. The link is shown only now.
2. Open it on a phone **off the tailnet** (mobile data): "Open Stump on this device?" with one button → press it →
   the sign-in page. Sign in as the friend. (The owner never signs in publicly: try it, it is refused.)
3. Back on the page: **Devices with a pass** lists the phone with its address and *last seen*.
4. **Revoke** it: the phone gets "Access turned off" within a couple of seconds, no reload, no terminal. **Restore**.
5. **Sign-in protection:** three wrong passwords for the friend show up under *Recent sign-ins*; lock the account by
   hand and **Unblock** it: a typo afterwards does not lock it again.
6. **People → Set a new password** for the friend: the page shows a generated password once; the friend signs in
   and is asked for a new one before anything else.

Logs prove no link or token was logged and the pass is attributed:

```sh
docker logs --since 10m stump-edge 2>&1 | grep -c '/api/_i/-hidden-'                                        # >= 1
docker logs --since 10m stump-edge 2>&1 | grep -E '/api/_i/[A-Za-z0-9_-]{10,}' || echo "no invite link in the edge log"
docker logs --since 10m stump-fork 2>&1 | grep -E '/api/_i/[A-Za-z0-9_-]{10,}' || echo "no invite link in the app log"
```

## 10. Funnel (the owner's step)

```sh
tailscale funnel --bg --https=8443 http://127.0.0.1:10803
tailscale funnel status          # must show ONLY :8443 -> 127.0.0.1:10803 for Stump (never :10801 or :10802)
```

## Enable friend uploads

### Before you start

1. **`stump-fork` must run this branch's build.** The Stump that is live today (friends-7, `stump:fork-cab3a8d87`)
   already carries the upload pipeline, switched off, but **not** what this section promises: approvals that are only
   the owner's, the refusals through the edge, `uploadBooks` needing *Manage library*, and the edge route with its
   limits. Deploy this branch first with the routine of `AGENTS.md` (it has **no migration**), then check:

   ```sh
   cd /mnt/data2/containers/stump-fork
   docker ps --filter name=stump-fork --format '{{.Image}}'          # stump:fork-<the revision below>
   git -C $S rev-parse --short=9 HEAD                                # the same revision
   ```
2. **The app must ask for a pass** (`STUMP_EDGE_REQUIRE_PASS=true`, step 8 above). Without it the edge is the only
   gate. Check: `docker logs --since 30m stump-fork 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -o 'require_pass: [a-z]*' | tail -1`
   prints `require_pass: true` (restart `stump-fork` first if the log has rotated).
3. **Nothing is running**: `sqlite3 -readonly config/stump.db "select count(*) from jobs where status in ('RUNNING','QUEUED','PAUSED')"` prints 0.

### What you are switching on

A build of this branch carries friend uploads, **switched off**: nobody can upload from the internet until all four
things below are true. Do them in this order, the permission last: the edge limits, the checker, the setting, the
permission.

What Stump itself guarantees (tests: `apps/server/tests/access/uploads.rs`, `graphql/intake_approval.rs`,
`graphql/upload.rs`):

* the one way a book comes in from the internet is **Add books**, `POST /api/v2/intake/upload`, within the person's
  limits; it waits for the owner's approval, and the book that is filed is one the isolated checker wrote anew;
* **approving, sending back, the approval queue and its count are the server owner's, from home**: the app refuses
  them for every request that came through the edge (nginx only forwards the GraphQL call), whatever permissions a
  friend holds (*Manage library* and *Delete library* included);
* filing straight into a library (`intakeAccept`, `intakeUpload`, `uploadBooks`, `uploadSeries`) never works through
  the edge; `uploadBooks` and `uploadSeries` need *Manage library*, so *Upload files* alone no longer skips the inbox;
* somebody who files books themselves (*Manage library*) uploads from home only; the owner's credentials are refused
  through the edge like everywhere.

### 1. The edge: a route for books

`nginx.conf` now has a location for `/api/v2/intake/upload`: a body up to `intake_max_request_mb` + 1 MB (**301 MB**;
raise both together), 12 requests a minute per client (burst 6), 2 at a time per client and 8 in all, nothing spooled
(`proxy_request_buffering off`), Stump's own origin only, and the pass every request needs. The public GraphQL body cap
is **256 KB** now (a 20,000-character description in 4-byte characters fits); everything else stays as it was: 25 MB by
default, 4–8 KB on the sign-in, refresh and invite routes. `stump-gate.js` does not change (it is identical to the live one).

Run it as **one chain**: each command runs only if the one before it worked, so a failed copy can never be followed by
a reload that re-applies the old file and still prints the expected answers.

```sh
cd /mnt/data2/containers/stump-fork && : "${S:?set S first, see the top of this file}" \
&& N=$PWD/edge-new && rm -rf $N && mkdir -p $N && chmod 755 $N \
&& install -m 644 $S/deploy/public-edge/{nginx.conf,stump-gate.js} $N/ \
&& cp edge/revoked.map $N/revoked.map && chmod 644 $N/revoked.map \
&& docker run --rm --network stump-fork_default -e EDGE_KEY=$(openssl rand -hex 32) -e EDGE_COOKIE=__Host-g3-stump \
     -v $N:/etc/nginx/edge:ro -v $PWD/edge-state:/etc/nginx/state:ro --entrypoint nginx nginx:alpine -t -c /etc/nginx/edge/nginx.conf \
&& bash $S/deploy/public-edge/verify-edge.sh \
&& cp -a edge/nginx.conf edge/nginx.conf.bak-$(date +%Y%m%d)-before-uploads \
&& install -m 644 $N/nginx.conf edge/nginx.conf \
&& docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf \
&& docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf \
&& echo "edge switched"
```

`verify-edge.sh` ends with `85 checks, 0 failed` (needs docker; about a minute). If the chain stops, read its last
line: before `install -m 644 $N/nginx.conf edge/nginx.conf` nothing live has changed; after it, put the old file back
(`cp -a edge/nginx.conf.bak-<date>-before-uploads edge/nginx.conf`, see *Back out*).

Then the two checks that tell the new edge from the old one (the old one answers the first line exactly the same way,
so only the second proves the new file is the one in the container):

```sh
E=http://127.0.0.1:10803
echo "no pass:    $(curl -s -o /dev/null -w %{http_code} -X POST "$E/api/v2/intake/upload?libraryId=x")"                  # 404
echo "new config: $(docker exec stump-edge grep -c 'location = /api/v2/intake/upload' /etc/nginx/edge/nginx.conf)"   # 1 (0 = the old config is still there)
```

### 2. The checker, and the setting

Friends' books are read by a second container of the same image (`stump-intake-probe`: no network, read-only root, no
capabilities, the inbox read-only, one process with a memory ceiling and a deadline for every file); the server reads
none of their bytes. The compose service is in `docker/compose.fork-intake-probe.example.yml`, with its own steps:

```sh
cd /mnt/data2/containers/stump-fork
mkdir -p ./config/workers/intake ./config/intake && ls -ld ./config/workers/intake ./config/intake     # owned by sthefano (uid 1000), not root
cp -a docker-compose.yml docker-compose.yml.bak-$(date +%Y%m%d)-intake-probe       # the backup first, then the merge:
# merge the example's `x-stump-image` anchor and its `stump-intake-probe` service into docker-compose.yml, and
# make stump-fork use the same anchor (`image: *stump_image`), with the anchor's default set to the tag in use today
# (the one that stump-fork's own `image: ${STUMP_FORK_IMAGE:-stump:fork-<rev>}` line names now)
```

Then the setting, in the compose file's `environment:` for `stump-fork` (not in `Stump.toml`: Stump rewrites that file
at every start) :

```yaml
      - STUMP_INTAKE_PROBE_MODE=queue        # off (default) | queue | subprocess | inprocess
```

`queue` is the one to use: the isolated container above. `subprocess` is the same checker as a child process of the
server (same user, folders and network): only for a server without the second container. `inprocess` reads friends'
books inside the server: never for the internet. `off` closes uploads for everyone who needs approval.

```sh
docker compose config -q && docker compose up -d --no-build stump-fork stump-intake-probe     # BOTH, together
sleep 15; docker logs --since 2m stump-intake-probe 2>&1 | grep -E 'selftest|ready'
#   one `selftest <check>: ok …` line per check, then `stump-intake-probe <rev> ready`
docker logs --since 2m stump-intake-probe 2>&1 | grep -cE 'selftest .*: (WARN|FAIL)'      # 0
#   a WARN (no network? a read-only inbox?) means it is NOT isolated, a FAIL that an essential check failed: the checker
#   still says `ready` after either, so look at this count and not only at `ready`; fix it first
hb=config/workers/intake/worker.json; [ -f $hb ] && { a=$(( $(date +%s) - $(stat -c %Y $hb) )); [ $a -lt 25 ] && echo "checker heartbeat fresh (${a}s old)" || echo "NOT fresh (${a}s old): the server sees the checker as offline"; } || echo "no worker.json yet: the checker has not started"
```

**From now on every Stump image change recreates both services**: `docker compose up -d --no-build stump-fork
stump-intake-probe`. The checker and the server must be the same build; with another version the checker is "out of
date", checking is offline, and friends' uploads close until both are recreated (the owner's own uploads are not
affected). The deploy routine in `AGENTS.md` says the same, and keeps the image tag in the one `x-stump-image` line
that pins both.

Limits (Stump.toml or `STUMP_<KEY>` in the environment; the server sends the checker its limits with every request,
and the checker keeps them under ceilings of its own): `intake_max_request_mb` 300, `intake_max_file_mb` 300, `intake_max_files_per_request`
50, `intake_user_quota_mb` 2048 (what a person can have waiting), `intake_user_max_items` 100, `intake_min_free_gb`
20 (uploads stop below this much free disk), `intake_total_max_gb` 50, `intake_upload_idle_secs` 60;
for the checker `intake_probe_timeout_secs` 120, `intake_probe_mem_mb` 768, `intake_probe_max_entries` 20000,
`intake_probe_max_entry_mb` 64, `intake_probe_max_unpacked_mb` 2048, `intake_probe_max_pages` 10000,
`intake_probe_max_image_mp` 50, `intake_active_content` `strip`, `intake_cbr_to_cbz` true. One person can have limits of
their own in **Settings → Book approvals**. `enable_upload = false` closes uploads for everybody ("Uploads are turned
off on this server").

### 3. The permission

In **Settings → Users**, give the friend **Add books** and nothing else of this family. Never *Upload files*,
*Manage library*, *Delete library* or *Manage users*: a friend needs none of them. *Manage library* (which *Delete
library* includes) files books without any approval, from home only. *Upload files* alone no longer files books (the
server asks for *Manage library* too), but it is the permission of the library upload paths, which friends have no use
for. *Manage users* hands out permissions. Restrict what they may see with library exclusions and an age restriction
as usual: what an uploader is shown (similar books, folders, tags) is limited to the books they can read.

### 4. Check it (loopback port of the public listener; use a test person and a test device)

The test person is an ordinary friend account with **Add books** and nothing else (step 3). Run the blocks below in
one shell, in this order.

**a. A pass, a sign-in and an upload:**

```sh
cd /mnt/data2/containers/stump-fork; E=http://127.0.0.1:10803
# a pass for a test device (Settings → Public access → Invite links; the break-glass script does the same) as in step 7
L=$(/mnt/data2/containers/edge-common/invite.sh stump uptest 1 | sed 's|https://g3.tail3e401.ts.net:8443|'$E'|')
C=$(curl -s -D- -o /dev/null -X POST -H 'Sec-Fetch-Site: same-origin' --data-urlencode "t=$L" $E/api/_i/ | grep -i '^set-cookie' | sed -E 's/^[Ss]et-[Cc]ookie: ([^;]+);.*/\1/')
# the friend signs in through the edge (counts as one of the device's 8 sign-ins a day)
TOK=$(curl -s -b "$C" -H 'Content-Type: application/json' -d '{"username":"<friend>","password":"<password>"}' "$E/api/v2/auth/login?generate_token=true" | python3 -c 'import json,sys;print(json.load(sys.stdin)["accessToken"])')
LIB='<a library id the friend can use>'      # replace the placeholder with a library id
python3 $S/deploy/public-edge/make-test-epub.py /tmp/hostile.epub
echo "upload:     $(curl -s -o /tmp/up.json -w %{http_code} -b "$C" -H "Authorization: Bearer $TOK" -F 'file=@/tmp/hostile.epub;filename=hostile.epub' "$E/api/v2/intake/upload?libraryId=$LIB")"   # 200 and {"items":[…]}
ID=$(python3 -c 'import json;print(json.load(open("/tmp/up.json"))["items"][0]["id"])'); echo "item:       $ID"
```

**b. What the edge refuses:**

```sh
echo "no pass:    $(curl -s -o /dev/null -w %{http_code} -X POST "$E/api/v2/intake/upload?libraryId=$LIB")"                                  # 404
echo "bad origin: $(curl -s -o /dev/null -w %{http_code} -X POST -b "$C" -H "Authorization: Bearer $TOK" -H 'Origin: https://evil.example' -F 'file=@/tmp/hostile.epub;filename=hostile.epub' "$E/api/v2/intake/upload?libraryId=$LIB")"   # 403: the new location's own answer
echo "too big:    $(curl -s -o /dev/null -w %{http_code} -X POST -b "$C" -H "Authorization: Bearer $TOK" -H 'Content-Type: multipart/form-data; boundary=x' -H 'Content-Length: 400000000' --data-binary x "$E/api/v2/intake/upload?libraryId=$LIB")"   # 413: before a byte is read (the old config says 413 too)
```

A `bad origin` that is not 403 means the old edge file is still the one running: go back to step 1 and look at its
`new config` line.

**c. The book reaches the approval queue.** A friend's book waits for approval only once it is **sent**: after the
checker has read it, it is `READY` or `NEEDS_REVIEW` in the friend's own inbox and the owner sees nothing. The web's
*Send for approval* button makes the call below:

```sh
Q='{"query":"{ intakeItems { id state } }"}'
for i in $(seq 1 30); do
  ST=$(curl -s -b "$C" -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' -d "$Q" $E/api/graphql \
       | python3 -c 'import json,sys;print(next((x["state"] for x in json.load(sys.stdin)["data"]["intakeItems"] if x["id"]=="'$ID'"),"?"))')
  case $ST in READY|NEEDS_REVIEW|FAILED) break;; esac; sleep 2
done; echo "analysed:   $ST"          # READY or NEEDS_REVIEW (FAILED: the checker refused the book, its reason is in the inbox: stop here)
curl -s -b "$C" -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' \
  -d '{"query":"mutation { intakeSubmit(ids:[\"'$ID'\"]) { id state } }"}' $E/api/graphql | head -c 200; echo     # "state":"PENDING_APPROVAL"
```

If `intakeSubmit` answers with a message instead of `PENDING_APPROVAL` (it names what the uploader still has to fill
in, for example a title the checker could not read), open **Add books** as the test person, fix what the card asks and
press *Send for approval*: it is the same call.

**d. The friend cannot approve, whatever the permissions** (the item must still be `PENDING_APPROVAL` afterwards):

```sh
echo "count:      $(curl -s -b "$C" -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' -d '{"query":"{ pendingIntakeCount }"}' $E/api/graphql | head -c 160)"   # an error, never a number
echo "approve:    $(curl -s -b "$C" -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' -d '{"query":"mutation { intakeApprove(ids:[\"'$ID'\"]) { id state } }"}' $E/api/graphql | head -c 160)"   # an error, never a state
```

**e. The owner approves, from home.** At home as the owner (the LAN or the tailnet, never through the edge), open
*Settings → Book approvals*: the test book waits there, with what the checker found (a script, a handler, a frame, a
`javascript:` link, a form). Approve it, wait for *Filed*, and look at what reached the library:

```sh
F=$(find /mnt/data2/Books -iname '*hostile*' -name '*.epub' -newer /tmp/hostile.epub | head -1); echo "$F"
unzip -p "$F" | grep -aciE '<script|<[a-z0-9]+[^>]*[[:space:]]on[a-z]+[[:space:]]*=|<iframe|javascript:|<form'   # 0: the filed copy was written anew, none of it is left
unzip -l "$F" | head                                                          # mimetype first, content.opf, nav.xhtml, toc.ncx, the page
```

The test book carries `onload` and `onclick` handlers on purpose: a filed copy that still has one of them prints a
number other than 0.

**f. Clean up.** *Remove from library* (on the book's page) only hides the test book and keeps its file for 14 days:
delete it for good in **Settings → Deleted books → Delete permanently**. Revoke the test device in **Settings → Public
access**, remove the test person if you made one, and `rm -f /tmp/hostile.epub /tmp/up.json`.

### Back out

```sh
# friends can't upload any more; what is in their inboxes stays. In the compose file, in the `environment:` of
# stump-fork, SET `STUMP_INTAKE_PROBE_MODE=off`: do not just delete the line, because Stump writes the value in effect
# back into Stump.toml at every start, so `queue` would stay in force
docker compose up -d --no-build stump-fork && docker compose stop stump-intake-probe
# the edge's upload location (optional: without the setting nothing can be uploaded anyway)
cp -a edge/nginx.conf.bak-<date>-before-uploads edge/nginx.conf
docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf && docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf
```

## Rollback (in this order)

```sh
cd /mnt/data2/containers/stump-fork; B=config.backup-<ts>-db
tailscale funnel --https=8443 off                        # 1. close the public address while you work
docker compose stop stump-edge stump-fork                #    (Stump stopped: the SQL below runs with nothing writing)
cp -a docker-compose.yml.bak-<date>-public-access docker-compose.yml          # 2. compose, edge files, key file
rm -rf edge && cp -a $B/edge edge
sqlite3 config/stump.db < $B/manual-down-1-intake.sql    # 3. the tables of the migrations, the last one first (each SQL's own checks print 0)
sqlite3 config/stump.db < $B/manual-down-2-access.sql
cp -a $B/Stump.toml config/Stump.toml                    #    the previous image rewrites it at start: put it back
docker compose up -d --force-recreate stump-fork stump-edge                    # 4. the previous image (set in the compose file) and the old edge
```

Passes already handed out keep working through the old edge (their format did not change); `invite.sh stump …` and
`revoked.map` work before, during and after. Two notes: **restoring an older database snapshot forgets the revocations
made since**, so re-check *Devices* after any restore; and **rotating `STUMP_EDGE_KEY`** (edit `.env`, recreate
`stump-fork` and `stump-edge`) invalidates every pass at once, after which each person needs a new invite. After the
manual down, the invites and the device list of the page are gone: re-add the ids you want dead to `edge/revoked.map`
and reload the edge (`docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf`).

## Operations

* Everything routine is in **Settings → Public access**. The terminal is only for emergencies:
  `edge-common/invite.sh stump <name> [hours]` (the app accepts its links), `edge/revoked.map` (+ the reload above).
* Edit the edge config in place, then:
  `docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf && docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf`.
* Funnel off, any time: `tailscale funnel --https=8443 off` (the tailnet `:10802` and the LAN `:10801` are unaffected).
* The edge follows `edge-state/state.json` (device ids and numbers only). If the file is missing the edge refuses every
  pass (fail closed) until the app writes it again; the app writes it at every start, after every change and every 6 hours.
* A rate-limit rejection (`429`) of the edge's own per-client limit (6 sign-ins a minute) has no state to clear: it ends
  within a minute. The daily budget (8 sign-ins a device) is reset from the page.
