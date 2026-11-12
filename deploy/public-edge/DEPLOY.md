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
# the two migrations of this build, each with its own down (run in the reverse order: intake first, then access):
# copies for the record, next to the snapshot; the rollback below runs the same files from $S
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
# and the real config against a stub (321 checks, ~9 min, nothing live is touched):
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

1. **Settings → Public access → People:** click the friend → **Add a device with a link**: device name, 3 days, can be
   opened 3 times → **Create link** → **Copy**. The link is shown only now. (A link for somebody who has no account yet is
   **Invite someone**, on the same page.)
2. Open it on a phone **off the tailnet** (mobile data): "Open Stump on this device?" with one button → press it →
   the sign-in page. Sign in as the friend. (The owner never signs in publicly: try it, it is refused.)
3. Back on the page: the friend's sheet lists the phone under **Devices (1 of 3)**, with when it was last used.
4. **Remove** it (confirm): the phone gets "Access turned off" within a couple of seconds, no reload, no terminal.
   **Show removed devices → Restore**.
5. **Sign-in protection:** three wrong passwords for the friend show up under **History → Sign-ins** (*Show history*); lock
   the account by hand and **Unblock** it (in the friend's sheet): a typo afterwards does not lock it again.
6. The friend's sheet → **Set a new password**: the dialog shows a generated password once (*They choose their own at their
   next sign-in* is ticked); the friend signs in and is asked for a new one before anything else.

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

`verify-edge.sh` ends with `321 checks, 0 failed` (needs docker; about nine minutes). If the chain stops, read its last
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

The test person is an ordinary friend account with **Add books** and nothing else (step 3). **With public sign-in on
(`STUMP_EDGE_PUBLIC_SIGNIN=true`, see "Friend onboarding" below) the account must also be let in from the internet**:
*Settings → Public access → People → Public sign-in* on for it (the switch refuses an account that holds more than a
friend may, and names what to remove). An account the owner never let in gets `401` on every request through the edge,
whatever pass the browser holds. Run the blocks below in one shell, in this order.

**a. A pass, a sign-in and an upload:**

```sh
cd /mnt/data2/containers/stump-fork; E=http://127.0.0.1:10803
# a pass for a test device (Settings → Public access → a person's sheet → Add a device with a link; the break-glass script does the same) as in step 7
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
sqlite3 config/stump.db < $S/crates/migrations/manual-down/m20261114_000000_intake_approval.sql   # 3. the tables of the migrations that were applied, the last one first (each SQL's own checks print 0)
sqlite3 config/stump.db < $S/crates/migrations/manual-down/m20261113_000000_public_access.sql
cp -a $B/Stump.toml config/Stump.toml                    #    the previous image rewrites it at start: put it back
docker compose up -d --force-recreate stump-fork stump-edge                    # 4. the previous image (set in the compose file) and the old edge
```

Run only the downs of migrations that are in `seaql_migrations` (`select version from seaql_migrations order by version
desc limit 3`), and in that order. The two files are the ones the backups hold as `manual-down.sql`: the backup made
at the Public access deploy holds the access one, the backup made at the uploads deploy the intake one; either
backup folder is enough to find them again (`cmp` them with the files above).

Passes already handed out keep working through the old edge (their format did not change); `invite.sh stump …` and
`revoked.map` work before, during and after. Two notes: **restoring an older database snapshot forgets the revocations
made since**, so re-check *Devices* after any restore; and **rotating `STUMP_EDGE_KEY`** (edit `.env`, recreate
`stump-fork` and `stump-edge`) invalidates every pass at once, after which each person needs a new invite. After the
manual down, the invites and the device list of the page are gone: re-add the ids you want dead to `edge/revoked.map`
and reload the edge (`docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf`).

## Friend onboarding: signup links, sign-in from any device, reading apps

Branch `feat/friend-onboarding` (design: `research/2026-11-12-friend-onboarding-design.md`, with the squad lead's
review applied; it also contains `fix/edge-audit`). What the owner gets, all in **Settings → Public access**:

* **Signup links** (after the page was redone in `feat/access-ux`: the button **Invite someone** on *Public access*, one person or
  several people; a device link for somebody who already has an account is *Add a device with a link* in their sheet, under *People*): a link for one person or an *open* one (a budget of 2 to 25 accounts), valid 1 hour to 30 days (3
  by default). The visitor opens it, chooses a username and a password, and is in: the account is made from the link's
  **preset** (default: Change password, Change avatar, Download files, Add books; **only the library named "Biblioteca"**;
  libraries made later stay hidden; 3 devices). A link can never give *Upload files*, *Scan library* or anything that
  manages, edits, creates or deletes: the server refuses it when the link is made and again when it is used.
* **Sign-in from any device**: after the first visit, a friend opens the address on any phone or computer and signs in with
  the username and password; that device gets its pass (up to 3 devices a person, then a clear refusal and a warning
  for the owner). The owner and every account that holds more than a friend may are refused through the edge, as before.
* **Reading apps** (KOReader, a Kobo…): *Settings → Reading apps* gives a personal address with a key in it
  (`/opds/<key>/v1.2/…`): no pass, no cookie, GET/HEAD only, the OPDS 1.2 routes only, 10 requests a second, 300 files a day
  per key; it counts as one of the person's devices and is revocable at once.
* **Warnings** (people at or over their device limit, four or more sessions, bursts of failed sign-ins, links near their
  budget) at the top of the page and as a badge on the Settings button.
* Small hardening: the user editor no longer ticks *Scan library* for *Upload files*; every mutation that queues a job
  (scans, analyses, thumbnails, metadata fetches) is refused through the edge (a test fails the build for one that is not).

| piece | change |
|---|---|
| app (`stump-fork` image) | the sign-in page and signup form (`/api/_i/signin`, `/api/_i/join`), reading-app keys in OPDS, the friend-set rule on **every** public request, the friend and owner GraphQL APIs, **one migration** (`m20261115_000000_friend_onboarding`: 3 new tables and `access_devices.kind`) |
| `stump-fork` compose | `STUMP_EDGE_PUBLIC_SIGNIN` (`false` first, `true` at step 4) |
| public edge | `edge/{nginx.conf,stump-gate.js}`: pass-less `GET /` shows the sign-in page, limits for it and for signup, the reading-key location, a 404 that says where to sign in, keys out of every log |
| what stays | passes, device links for existing people, the owner's terminal tools (`invite.sh`, `revoked.map`), approvals owner-only, the owner never through the edge |

**The rule behind all of it (with `STUMP_EDGE_PUBLIC_SIGNIN=true`).** On every request through the edge, whatever the credential
(a session, a token, an API key, a reading-app key) and whatever pass the browser holds, the account must be: not the owner,
not locked, *Public sign-in* on for it, and holding only friend-set permissions (Change password / username / avatar,
Download files, Add books, Access smart lists). Anything else gets the owner's `401` (without deleting any cookie). A
permission the owner gives a friend later cuts them off the internet at once and still works at home. Accounts made by
signup links are enabled; an existing account is enabled by the switch in People, or by making a device link for it (friend-class
accounts only). **Dani today** holds *Upload files* and *Scan library*: remove both (*Settings → Users*), then turn
*Public sign-in* on for her in People ("Only Biblioteca", and set a new password).

**What the numbers are** (constants with tests, not settings):

| limit | value |
|---|---|
| edge, sign-in POST | 6 a minute per client (burst 4), 30 a minute for everybody (burst 10); page GET 30 a minute |
| edge, signup POST | 3 a minute per client (burst 3), 2 a minute for everybody (burst 10) |
| app, one client | at most 10 counted failures per 10 minutes, then `429` with no password work |
| app, one name from one client | 4 free tries, then waits of 1, 2, 4, 8, 16, 30 minutes (window of 1 hour): about 10 guesses an hour per client and name |
| app, one name from everywhere | past 20 failures in an hour, waits of 1 to 30 minutes for everybody, a friend's *new* device included: about 25 guesses an hour (~600 a day) however many clients |
| app, everybody | 40 failures from clients under their cap in 10 minutes refuse everybody for a few minutes (it takes at least 4 clients) |
| never | a lock: a failure through the page never counts towards `users.is_locked`, and a friend's known devices are never asked |
| passwords | 10 to 72 bytes, 5 different characters, not a common one, not containing the username |
| devices | 3 a person (1 to 10, owner's choice); a device unseen for 60 days no longer counts; reading apps count |

### 0. Pre-flight and backups

```sh
cd /mnt/data2/containers/stump-fork; S=/mnt/data2/Code/worktrees/stump/onboard     # the checkout of the revision you deploy
test -f $S/deploy/public-edge/nginx.conf && git -C $S log --oneline -1
sqlite3 -readonly config/stump.db "select count(*) from jobs where status in ('RUNNING','QUEUED','PAUSED')"     # 0
grep -E '^trust_proxy_headers *= *true' config/Stump.toml || echo 'STOP: trust_proxy_headers is not true'      # must print the line (see below)
T=$(date -u +%Y%m%dT%H%M%SZ); B=config.backup-$T-db; mkdir $B && sqlite3 config/stump.db ".backup $B/stump.db"
cp config/Stump.toml $B/; cp -a edge $B/edge; cp $S/crates/migrations/manual-down/m20261115_000000_friend_onboarding.sql $B/manual-down.sql
cp docker-compose.yml docker-compose.yml.bak-$(date +%Y%m%d)-friend-onboarding
docker image inspect stump:fork-<rev> >/dev/null && echo image-ok       # built on l5 and loaded (AGENTS.md); remember the rollback tag
```

**`trust_proxy_headers = true` must be in `config/Stump.toml`** (it is on the live file). The sign-in limits are per client,
and the client is the address the edge forwards in `X-Real-IP`. With the setting off every client from the internet would
look like the edge itself, and 10 failed sign-ins from anybody would make every friend wait to sign in on a new device.
If the line is missing or `false`, stop and fix that first (it needs a restart of `stump-fork`, which step 1 does anyway).

*Test the migration first* on a copy of the live database in an isolated container (never the live one):
`cp $B/stump.db /tmp/migrate-test.db`, run the image against it with the app's migration command, and check
`sqlite3 /tmp/migrate-test.db "select version from seaql_migrations order by version desc limit 2"` and the four
checks printed at the end of `manual-down.sql`.

### 1. The app first, with the switch off

In `docker-compose.yml`: set the shared image line (`x-stump-image: &stump_image ${STUMP_FORK_IMAGE:-stump:fork-<rev>}`),
add a comment (branch, date, migration, rollback image `stump:fork-<previous>`), and add `STUMP_EDGE_PUBLIC_SIGNIN=false`
next to `STUMP_EDGE_REQUIRE_PASS=true` in the environment of `stump-fork`.

```sh
docker compose config -q && docker compose up -d --no-build stump-fork stump-intake-probe
docker logs --since 2m stump-fork 2>&1 | grep -iE 'migrat|error|panic' | head      # the migration applied, nothing else
docker logs --since 2m stump-intake-probe 2>&1 | grep -cE 'WARN|FAIL'               # 0, then `stump-intake-probe <rev> ready`
echo "home $(curl -s -o /dev/null -w %{http_code} http://127.0.0.1:10801/api/v2/ping) tailnet $(curl -s -o /dev/null -w %{http_code} https://g3.tail3e401.ts.net:10802/api/v2/ping) public $(curl -s -o /dev/null -w %{http_code} http://127.0.0.1:10803/api/v2/ping)"   # 200 200 404
curl -s -o /dev/null -w '%{http_code}\n' -X POST http://127.0.0.1:10803/api/_i/signin -d x=1     # 404: the switch is off
```

With the switch off the app behaves as before (a pass-less `GET /api/_i/signin` shows the old "Private site, paste your invite
link" page); it also refuses reading-app keys that arrive through the edge.

### 2. The edge: stage, validate, install (no recreate: no new mount)

```sh
mkdir -p edge-new && cp $S/deploy/public-edge/nginx.conf $S/deploy/public-edge/stump-gate.js $S/deploy/public-edge/revoked.map edge-new/
# nginx -t with the same mounts as the live edge, in a throwaway container
docker run --rm -v "$PWD/edge-new":/etc/nginx/edge:ro -v "$PWD/edge-state":/etc/nginx/state:ro -e EDGE_KEY=x$(printf 'x%.0s' $(seq 63)) -e EDGE_COOKIE=__Host-g3-stump \
  --add-host stump-fork:127.0.0.1 --entrypoint nginx nginx:alpine -t -c /etc/nginx/edge/nginx.conf
bash $S/deploy/public-edge/verify-edge.sh        # 321 checks, ~9 min, nothing live is touched (it runs the staged copy of this folder)
cp edge/nginx.conf edge/nginx.conf.bak-$(date +%Y%m%d)-before-onboarding && cp edge/stump-gate.js edge/stump-gate.js.bak-$(date +%Y%m%d)-before-onboarding
cp edge-new/nginx.conf edge/nginx.conf && cp edge-new/stump-gate.js edge/stump-gate.js
docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf && docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf
sleep 3; docker exec stump-edge grep -c 'location = /api/_i/signin' /etc/nginx/edge/nginx.conf         # 1
bash $S/deploy/public-edge/verify-edge.sh --live                                                       # the live listener: no pass is 404
```

Check: a request with a pass still works, an old invite link still opens, `home 200`, `tailnet 200`, and a pass-less
`GET http://127.0.0.1:10803/` shows the old private page (the switch is off).

### 3. Turn it on

Set `STUMP_EDGE_PUBLIC_SIGNIN=true` in `stump-fork`'s environment and recreate **both** containers (the checker must run the same build):

```sh
docker compose config -q && docker compose up -d --no-build stump-fork stump-intake-probe
curl -s http://127.0.0.1:10803/ | grep -c 'action="/api/_i/signin"'              # 1: the front door shows the sign-in page
curl -s -D- -o /dev/null http://127.0.0.1:10803/api/v2/ping | grep -ci 'x-edge-sign-in: 1'    # 1
```

### 4. Walk through it once, with a throwaway link and account (before telling anybody)

As the owner at home (`K` is `book-curator`'s owner key, `H=http://127.0.0.1:10801/api/graphql`):

```sh
cd /mnt/data2/containers/stump-fork; K=$(cat ../stump/book-curator.key); H=http://127.0.0.1:10801/api/graphql; E=http://127.0.0.1:10803
LIB=$(curl -s -H "Authorization: Bearer $K" -H 'Content-Type: application/json' -d '{"query":"{ libraries(pagination:{none:{unpaginated:true}}) { nodes { id name } } }"}' $H | python3 -c 'import json,sys;print(next(x["id"] for x in json.load(sys.stdin)["data"]["libraries"]["nodes"] if x["name"]=="Biblioteca"))')
URL=$(curl -s -H "Authorization: Bearer $K" -H 'Content-Type: application/json' -d '{"query":"mutation($i: CreateSignupLinkInput!){ createAccessSignupLink(input:$i){ url link { id } } }","variables":{"i":{"label":"throwaway","kind":"SINGLE","validForHours":24,"maxUses":1,"preset":{"permissions":["CHANGE_PASSWORD","CHANGE_AVATAR","DOWNLOAD_FILE","ADD_BOOKS"],"libraryIds":["'$LIB'"],"maxDevices":3}}}}' $H | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"]["createAccessSignupLink"]["url"])')
T=${URL##*/}; echo "link token is ${#T} characters"; PW='walk-through-Harbor-47'
# a GET shows the form and spends nothing; the POST makes the account, the device, the session and the pass
curl -s -o /dev/null -w 'form %{http_code}\n' $E/api/_i/$T
curl -s -D /tmp/join.h -o /dev/null -X POST -H 'Sec-Fetch-Site: same-origin' -d "t=$T&username=walkthrough&password=$PW&password2=$PW" $E/api/_i/join; head -1 /tmp/join.h     # 303
PASS=$(grep -i '^set-cookie: __Host-g3-stump=' /tmp/join.h | sed -E 's/^[^=]+=([^;]+);.*/\1/'); SESS=$(grep -i '^set-cookie: stump_session=' /tmp/join.h | sed -E 's/^[^=]+=([^;]+);.*/\1/')
curl -s -H "Cookie: __Host-g3-stump=$PASS; stump_session=$SESS" -H 'Content-Type: application/json' -d '{"query":"{ me { username } myDevices { active limit } }"}' $E/api/graphql    # walkthrough, 1 of 3
# two more devices by signing in, then the fourth is refused with the clear message
for n in 1 2 3; do curl -s -o /tmp/in$n.html -w "device $n: %{http_code}\n" -X POST -H 'Sec-Fetch-Site: same-origin' -H "X-Forwarded-For: 198.51.100.$n" -d "username=walkthrough&password=$PW" $E/api/_i/signin; done   # 303 303 403
grep -o 'already use [0-9]* devices' /tmp/in3.html
# a reading app: the address works with no pass and no cookie, only for OPDS, and stops when removed
C="Cookie: __Host-g3-stump=$PASS; stump_session=$SESS"
DEV=$(curl -s -H "$C" -H 'Content-Type: application/json' -d '{"query":"{ myDevices { devices { id current } } }"}' $E/api/graphql | python3 -c 'import json,sys;print(next(d["id"] for d in json.load(sys.stdin)["data"]["myDevices"]["devices"] if not d["current"]))')
curl -s -H "$C" -H 'Content-Type: application/json' -d '{"query":"mutation($id: ID!){ removeMyDevice(id:$id) }","variables":{"id":"'$DEV'"}}' $E/api/graphql      # true: room for the reading app
RA=$(curl -s -H "$C" -H 'Content-Type: application/json' -d '{"query":"mutation { createReadingApp(label: \"Kobo\") { id url } }"}' $E/api/graphql | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"]["createReadingApp"]["url"])')
P=${RA#https://g3.tail3e401.ts.net:8443}
echo "feed $(curl -s -o /dev/null -w %{http_code} $E$P) post $(curl -s -o /dev/null -w %{http_code} -X POST $E$P) basic $(curl -s -o /dev/null -w %{http_code} -H 'Authorization: Basic Zm9vOmJhcg==' $E$P) graphql $(curl -s -o /dev/null -w %{http_code} -H "Authorization: Bearer ${P#/opds/}" $E/api/graphql)"   # 200 405 403 404 (the last is not a 401: a reading-app address opens OPDS only, and the edge refuses a pass-less /api/graphql before the app sees it)
# six wrong passwords: the wait (429) for that name and client; another client is not asked to wait
for n in 1 2 3 4 5 6; do curl -s -o /dev/null -w "$(printf '%s ' wrong$n)%{http_code}\n" -X POST -H 'Sec-Fetch-Site: same-origin' -H "X-Forwarded-For: 198.51.100.77" -d "username=walkthrough&password=nope$n" $E/api/_i/signin; done    # 401 ×5, then 429
curl -s -o /dev/null -w 'another origin %{http_code}\n' -X POST -H 'Origin: https://evil.example' -d "username=walkthrough&password=$PW" $E/api/_i/signin       # 401 (the generic page): never signs in
# clean up. First the link, with its accounts turned off (the account's devices are revoked and its sessions end:
# deleting the account alone would leave its passes valid as devices of nobody), then the account (hard); then the
# files that held the pass and the session cookie; then check the library has no new rows
LINK=$(sqlite3 -readonly config/stump.db "select id from access_signup_links where label='throwaway'")
curl -s -H "Authorization: Bearer $K" -H 'Content-Type: application/json' -d '{"query":"mutation { revokeAccessSignupLink(id: \"'$LINK'\", disableAccounts: true) }"}' $H     # {"data":{"revokeAccessSignupLink":1}}
UID_=$(sqlite3 -readonly config/stump.db "select id from users where username='walkthrough'")
curl -s -H "Authorization: Bearer $K" -H 'Content-Type: application/json' -d '{"query":"mutation { deleteUser(id: \"'$UID_'\", hardDelete: true) { id } }"}' $H
rm -f /tmp/join.h /tmp/in[123].html
sqlite3 -readonly config/stump.db "select count(*) from users where username='walkthrough'; select count(*) from access_people;"
```

(The `X-Forwarded-For` header is trusted only on the loopback listener when the peer is the compose gateway; through the
real Funnel the client address is Tailscale's. If your shell cannot tell clients apart, the wait still shows.)

### 5. Tell the owner

How to make a link (*Public access → Invite someone*: one person or several people, how long, what they get), what the preset gives, that a friend opens the address on any
device and signs in, *My devices* / *Reading apps* for them, how to read the warnings, how to turn it all off, and the one
decision left: **Funnel stays as it is** (the switch is the feature's, not Funnel's).

### Back out

```sh
# 1. the switch: the sign-in page and signup stop at once; reading-app keys stop through the edge; passes, device links,
#    the owner's tools and everything older keep working. SET the value (do not delete the line: Stump writes the value in effect back)
sed -i 's/STUMP_EDGE_PUBLIC_SIGNIN=true/STUMP_EDGE_PUBLIC_SIGNIN=false/' docker-compose.yml && docker compose up -d --no-build stump-fork stump-intake-probe
# 2. (optional) the old edge files: the reading-app location and the front door go
cp edge/nginx.conf.bak-<date>-before-onboarding edge/nginx.conf && cp edge/stump-gate.js.bak-<date>-before-onboarding edge/stump-gate.js
docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf && docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf
# 3. while you work: tailscale funnel --https=8443 off
# 4. to return the IMAGE to stump:fork-<previous>: stop, run the manual down (the migration's own file, or restore the snapshot), put the
#    previous compose and edge files back TOGETHER with the image (the old image does not know the new edge's pages), start
docker compose stop stump-fork stump-intake-probe stump-edge
sqlite3 config/stump.db < $S/crates/migrations/manual-down/m20261115_000000_friend_onboarding.sql     # its own checks print 0 0 0
cp -a docker-compose.yml.bak-<date>-friend-onboarding docker-compose.yml; rm -rf edge && cp -a $B/edge edge
docker compose up -d --no-build --force-recreate stump-fork stump-intake-probe stump-edge
```

What a rollback keeps on purpose: the accounts the links made (with their permissions and library exclusions, so they stay
"Biblioteca only") and every browser pass. What it loses: the links, each person's switch / limit / hide-new-libraries
choice, the decision log and the reading-app keys. Two notes: **restoring a snapshot forgets revocations made since** (check
*Devices*), and **rotating `STUMP_EDGE_KEY` invalidates every pass and every reading-app key at once** (the Reading apps page then
shows the new address; each e-reader needs it pasted again).

## Public login: the web app's own sign-in screen on the public address

Branch `feat/public-login` (design: `research/2026-11-12-public-spa-login-design.md`, with the squad lead's review applied;
it builds on `feat/friend-onboarding`). **No migration of its own.**

**Deploying the integrated image (`integ/friends-8`: this branch plus `feat/access-ux`, which carries a migration)? Follow
*Combined release (integ/friends-8)* below, not the numbered steps here one by one, and use its back-out: with that migration in
the database the previous image alone does not start.** The steps here are the public-login part of that release (the app's
capability header, the browser check, the edge).

What changes for a visitor:

* Without a pass, `https://g3.tail3e401.ts.net:8443/` is **Stump's own login screen** (the React app, the short version: a
  username, a password, one line, nothing about invite links) instead of the server-made page. A signup link
  (`/api/_i/<token>`) opens the app's **join screen** (`/join`; the token travels in the address's fragment and is removed at
  once). Signing in or up makes the device's pass, as before, and the app is then loaded again as a signed-in one.
* **The server-made pages stay** (`/api/_i/signin`, `/api/_i/<token>`'s old form) for a rollback and for an older edge; with the
  new edge `/api/_i/signin` sends a visitor on to `/auth`. They are not a path without JavaScript: the screen needs it.
* What the edge lets through **without a pass** is exactly this (`nginx.conf` `$edge_open`, decided on the request line as sent,
  case-sensitive; the same rows are data in `open-paths.json`): GET/HEAD of `/`, `/auth`, `/join`; GET/HEAD of a flat
  `/assets/<name>.<ext>` (js css woff2 png webp ico, no query: no source map, no compressed twin, no folder); and `POST
  /api/v2/auth/login`, `POST /api/v2/auth/join` (no query). Three questions the screen asks before anybody is signed in are
  **answered by the edge itself** (constants: the viewer's 401, "claimed, public", "no identity provider"). Everything else is
  404 as before; the owner and admins are refused through the edge as before.
* **The build's own files have a request budget of their own** (`nginx.conf` zone `pub_assets`: the general 20 a second, a burst of 1500).
  A browser that is new to the address fetches about 110 files for the sign-in screen and about a hundred more for the app after
  signing in; with the API calls and the covers' budget (burst 300) one cold load, or two devices in a household, got a `429` on a
  file and the screen stayed on its loading spinner (found by `spa-capture.sh`, test 5/6). The location serves only a flat
  `/assets/<name>.<ext>` as the client sent it (an escape or a doubled slash is a `404`); a query is fine with a pass. `verify-edge.sh`
  checks a cold load of 450 files, that the API's budget is untouched by it, and that a flood is still limited.
* **Open connections are counted per client too, and Tailscale serve opens many of them.** Behind the Funnel's relay (`:8443`) and
  the tailnet's `:10802`, one browser's HTTP/2 becomes many parallel HTTP/1.1 connections to the edge, so a cold load of the app has
  dozens in flight at once. The first caps refused the app's own files (`limit_conn pub_conn`, a `429` on the sign-in screen's chunks) and
  the owner's own calls over the tailnet (`limit_conn home_gql`) in the live walk-through. They are now: **60 a client** on the public
  listener (was 30: API calls, covers, the websocket); the build files' location has its own, **120 a client and 300 in all** (a location
  that sets `limit_conn` drops the ones it would inherit, so it names both); the owner's GraphQL on the home listener **32 an address**
  (was 8). They are still caps: a flood of connections gets a `429`. `verify-edge.sh` holds 20 to 150 slow requests at once (the stub
  answers a request that carries `X-Hold` at a byte a second) and counts the refusals: 50 files, 40 API calls and 20 GraphQL calls
  are never refused, 150 files, 90 API calls and 60 GraphQL calls are.
* **To add a path to that list**: a row in `open-paths.json`, the regex in `nginx.conf` (`$edge_open`) and the rule in
  `middleware/edge.rs` (`is_open_public_request`). `verify-edge.sh` checks the edge column of every row, a Rust test checks the app
  column. The policy of the shell a stranger gets (hash of each inline script) is made by the **app** from the `index.html` it serves,
  so a later web build cannot drift from it.

| piece | change |
|---|---|
| app (`stump-fork` image) | `sign_in_from_internet` / `join_from_internet` (one core for the server-made page and the JSON sign-in, so the guard, the one bcrypt, the eligibility rule, the device limit, the audit and the warnings are the same code), `POST /api/v2/auth/join`, a pass-less branch of `POST /api/v2/auth/login` that answers fixed bytes with **no cookie** and a 404 **before any work** unless the edge's capability header and the switch are both on, `claim.isPublic`, the policy and `no-store` on the pass-less shell, a revoked pass taken as no pass on the open requests |
| public edge | `edge/{nginx.conf,stump-gate.js}` (+ `open-paths.json` for the checks): the open list, the three answers, **one budget** for the server-made form and the screen's sign-in (6 a minute per client, 30 for all), zones of their own for a sign-in that holds a pass (so a flood of strangers never makes a friend wait), the capability header `X-Edge-Login: spa`, own header sets for sign-in and signup (no renewal of an older pass after the app's new one) |
| web app | the login screen's public mode (errors in the form, a redirect only to a path of this site, a full page load after sign-in, no live-events socket before sign-in), the join screen, the browser's language for a visitor nobody knows yet, no `preconnect` to a third party in `index.html` |

### 0. Pre-flight and backups

As in *Friend onboarding* step 0 (a snapshot, `edge/`, the compose file, 0 running jobs, `trust_proxy_headers = true`) and also:

```sh
cd /mnt/data2/containers/stump-fork; S=/mnt/data2/Code/worktrees/stump/integ8     # the checkout of the revision you deploy (the integrated release)
grep -c 'STUMP_EDGE_REQUIRE_PASS=true' docker-compose.yml     # 1: the app's own pass check stays on while this is live (never turn it off with Funnel on)
grep -c 'STUMP_EDGE_PUBLIC_SIGNIN=true' docker-compose.yml    # 1
cp edge/nginx.conf edge/nginx.conf.bak-$(date +%Y%m%d)-before-publogin && cp edge/stump-gate.js edge/stump-gate.js.bak-$(date +%Y%m%d)-before-publogin
```

### 1. The app first (it is backward compatible with the older edge)

New image on the shared `x-stump-image` line (comment: branch, date, the migration if the image carries one, rollback image), recreate **both** containers:

```sh
docker compose config -q && docker compose up -d --no-build stump-fork stump-intake-probe
docker logs --since 2m stump-intake-probe 2>&1 | grep -E 'WARN|FAIL|ready'        # 0 WARN/FAIL, then `stump-intake-probe <rev> ready`
echo "home $(curl -s -o /dev/null -w %{http_code} http://127.0.0.1:10801/api/v2/ping) tailnet $(curl -s -o /dev/null -w %{http_code} https://g3.tail3e401.ts.net:10802/api/v2/ping)"   # 200 200
curl -s http://127.0.0.1:10803/ | grep -c 'action="/api/_i/signin"'               # 1: the OLD edge still rewrites `/` to the server-made page
```

Without the edge's header the app behaves exactly as before, so this step changes nothing for anybody.

### 2. The browser check, on a throwaway copy, before the edge goes live

```sh
cd $S && bash deploy/public-edge/spa-capture.sh stump:fork-<rev>      # ~3 min; nothing live is touched
```

It stands up the image with the **new** `nginx.conf` as its edge, a scratch config and no library (own network, ports 18380/18381), and
runs `apps/web/tests/public/public-login.spec.ts` in Chromium: every request a stranger's page makes is one the edge lets
through, nothing goes to another site, **zero Content-Security-Policy violations**, the short screen, an invite link → the join
screen with the token gone from the address and every storage, a sign-in on another device (a pass is made), the fourth device
refused in words, the owner never signing in here, Spanish for a Spanish browser. **If a file the screen needs is refused, the
spec names the request: add its exact rule to the three places above, never a wildcard. If a vendor library's feature check shows up as a
policy violation, never add `'unsafe-eval'` to make it go away.** Do not go on until it passes.

### 3. The edge: stage, validate, install (no recreate)

```sh
mkdir -p edge-new && cp $S/deploy/public-edge/nginx.conf $S/deploy/public-edge/stump-gate.js $S/deploy/public-edge/revoked.map edge-new/
docker run --rm -v "$PWD/edge-new":/etc/nginx/edge:ro -v "$PWD/edge-state":/etc/nginx/state:ro -e EDGE_KEY=x$(printf 'x%.0s' $(seq 63)) -e EDGE_COOKIE=__Host-g3-stump \
  --add-host stump-fork:127.0.0.1 --entrypoint nginx nginx:alpine -t -c /etc/nginx/edge/nginx.conf
bash $S/deploy/public-edge/verify-edge.sh        # 321 checks, ~9 min, nothing live is touched (it runs the staged copy of this folder)
cp edge-new/nginx.conf edge/nginx.conf && cp edge-new/stump-gate.js edge/stump-gate.js
docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf && docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf
sleep 3; docker logs --since 1m stump-edge 2>&1 | grep -c emerg                    # 0: a zone whose key changed would make the reload fail and keep the OLD config running
docker exec stump-edge grep -c 'X-Edge-Login' /etc/nginx/edge/nginx.conf           # 2 or more: the new file is the one running
bash $S/deploy/public-edge/verify-edge.sh --live                                   # the live listener, incl. the policy of the pass-less shell
```

Check: `home 200`, `tailnet 200`; a pass-less `GET http://127.0.0.1:10803/` is the app's own shell (`id="root"`), and its response has a
`Content-Security-Policy` and `Cache-Control: no-store`; `GET /api/v2/auth/viewer` with no pass is `401`, with a pass the app's own answer.

### 4. Walk through it once, with a throwaway link and account (before telling anybody)

As in *Friend onboarding* step 4, but in a **browser** (a private window): the signup link opens the join screen → an account is made → the
app loads signed in; sign out; open the address in **another** browser and sign in (the device is made); try a wrong password (*Wrong username
or password.*); the fourth device (*You already use 3 devices …*). Clean up the same way (revoke the link with its accounts turned off, delete the
account, remove the temporary files).

### 5. Tell the owner

Nothing new to do: signup links, the 3-device limit, *My devices*, *Reading apps* and the warnings are as before; friends now see the app's own
login screen. The server-made sign-in page is gone from view.

### Back out (fastest first)

```sh
# 1. the EDGE only, one second, no restart: the capability header disappears and the app returns to the server-made pages by itself
cp edge/nginx.conf.bak-<date>-before-publogin edge/nginx.conf && cp edge/stump-gate.js.bak-<date>-before-publogin edge/stump-gate.js
docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf && docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf
# 2. the app's switch (SET the value, do not delete the line): the new open list closes and `/` shows the "Private site" page
sed -i 's/STUMP_EDGE_PUBLIC_SIGNIN=true/STUMP_EDGE_PUBLIC_SIGNIN=false/' docker-compose.yml && docker compose up -d --no-build stump-fork stump-intake-probe
# 3. the IMAGE: back to the previous tag on the shared line, recreate both. Do 1 BEFORE 3: the new edge with an older image has no shell to serve
#    If the image you are leaving carries a migration (the integrated one has m20261116), the previous image does NOT start on that
#    database until the migration's manual down has run: use *Combined release -> Back out* (R2), not this line alone
# 4. while you work: tailscale funnel --https=8443 off
```

Known limits: a browser whose service worker holds an **old** build and has no pass cannot update it (`/sw.js` is not served without a pass): if its
screen looks wrong, open the address in a private window. The shared sign-in budget can still be used by four or more clients to slow *new-device* sign-ins
for a few minutes (known devices sign in with their pass and are not counted there). A visitor's browser is not told to contact any third party.

## Combined release (integ/friends-8)

One image and one deploy for five branches, merged into `integ/friends-8` on top of what is live (`stump:fork-43fa3fe50`, database
at `m20261115_000000_friend_onboarding`): `feat/public-login` (Stump's own login screen on the public address), `feat/access-ux`
(*Public access* redone, with warnings the owner can dismiss), `feat/app-ux` (the usability fixes), `feat/app-search` (the best title
first, an *Authors* group) and `feat/es-strings` (the whole interface in Spanish). Their notes: `research/2026-11-12-public-spa-login-design.md`,
`research/2026-11-12-access-ux-deploy-notes.md`, `research/2026-11-12-app-usability-report.md`. `S` below is the integ8 checkout at the revision of
the image.

| piece | change |
|---|---|
| database | **one migration**, `m20261116_000000_access_warning_reviews` (one table, `access_warning_reviews`), applied by Stump **when it starts**. Manual down: `crates/migrations/manual-down/m20261116_000000_access_warning_reviews.sql`; it only forgets which warnings were dismissed |
| app (`stump-fork` image) | all five branches. Friends can reach the new `searchAuthors` and `media(relevanceTo:)` (they see only what the Browse page would show them); the new owner mutations `dismissAccessWarnings` and `restoreAccessWarnings` are owner-only and refused through the edge |
| public edge | `edge/nginx.conf` and `edge/stump-gate.js` (from `feat/public-login`; `feat/access-ux` changes nothing in `deploy/public-edge/`). `open-paths.json` is data for the checks and is not copied to the live edge; the live `revoked.map` is never overwritten |
| `stump-fork` compose | nothing new: `STUMP_EDGE_REQUIRE_PASS=true` and `STUMP_EDGE_PUBLIC_SIGNIN=true` are set already and **stay set** |
| names | the older sections say *Signup links* and *Invite links*; the page now has **Invite someone** (one person or several people) and, in a person's sheet, **Add a device with a link** |

**Order: the app first, then the edge.** The new app is backward compatible with the old edge (without the edge's `X-Edge-Login: spa`
header it keeps showing the server-made pages), and the edge is the switch: friends see the app's own login screen the moment its
`nginx.conf` is reloaded. **`STUMP_EDGE_PUBLIC_SIGNIN` stays `true` for the whole deploy** (this is not the "switch off" order of *Friend
onboarding*): with it off, any account that is not the owner and holds a pass gets through the edge whatever permissions it has,
reading-app keys stop working through the edge, it costs two more recreates, and it buys nothing (the old edge keeps the server-made pages
until step 5, and `spa-capture.sh` runs on its own throwaway stack, so it cannot test the live switch anyway).

Run the second block of step 1 and steps 2 to 5 in **one shell**, in `/mnt/data2/containers/stump-fork`: they share `S` and `B`.

### 0. Build the image and bring it to g3

The merges took one side of the generated client (`packages/graphql/src/client/gql.ts`); `crates/graphql/schema.graphql` and the client are never
merged by hand. They are regenerated from the merged code (`scripts/fork/schema.sh`, then `yarn codegen` in `packages/graphql`) and committed, and the
verification run (all web tests, the Rust tests per package) passes, **before** the build. Then, as in `AGENTS.md`:

```sh
cd /mnt/data2/Code/worktrees/stump/integ8; git status --short | wc -l; git rev-parse --short=9 HEAD     # 0 changes; the revision is <rev>
l5run -- 'bash scripts/fork/build-local.sh stump:fork-<rev>'
ssh l5 'docker save stump:fork-<rev> | gzip -1' | gunzip | docker load
docker image inspect stump:fork-<rev> --format '{{index .Config.Labels "org.opencontainers.image.revision"}}'      # <rev>
```

### 1. Checks that touch nothing live (on l5: g3 is short of memory while builds run)

```sh
cd /mnt/data2/Code/worktrees/stump/integ8
l5run -- 'bash scripts/fork/node.sh "yarn install --frozen-lockfile >/tmp/y.log 2>&1 || tail -5 /tmp/y.log" && bash deploy/public-edge/spa-capture.sh stump:fork-<rev>'
l5run -- 'bash deploy/public-edge/verify-edge.sh'          # 321 checks, ~9 min, 0 failed
```

`spa-capture.sh` (~3 min; the first run pulls the browser image if l5 does not have it) is **the gate of this release**: the real-browser check of the
public login had never run before it, and the merged build splits the app into different files than the ones it was designed against. It must pass
with no refused request and **0** Content-Security-Policy violations. If a file the login screen needs is refused (an open file is a flat
`/assets/<name>.<js|css|woff2|png|webp|ico>` with no query and no leading underscore), add its exact rule to the three places named in *Public login* (the
row in `open-paths.json`, `$edge_open` in `nginx.conf`, `is_open_public_request` in `middleware/edge.rs`), never a wildcard, and build again. On g3 itself
the same two commands (without `l5run`) are fine only when no build runs and `free -g` shows memory to spare.

Then stage the edge on g3 and let nginx read it (the live `revoked.map` is copied into the staging folder so the syntax check sees the real one):

```sh
cd /mnt/data2/containers/stump-fork; S=/mnt/data2/Code/worktrees/stump/integ8
test -f $S/deploy/public-edge/nginx.conf && git -C $S log --oneline -1                  # <rev>: the revision of the image
rm -rf edge-new; mkdir -p edge-new && cp $S/deploy/public-edge/nginx.conf $S/deploy/public-edge/stump-gate.js edge-new/ && cp edge/revoked.map edge-new/
docker run --rm -v "$PWD/edge-new":/etc/nginx/edge:ro -v "$PWD/edge-state":/etc/nginx/state:ro -e EDGE_KEY=x$(printf 'x%.0s' $(seq 63)) -e EDGE_COOKIE=__Host-g3-stump \
  --add-host stump-fork:127.0.0.1 --entrypoint nginx nginx:alpine -t -c /etc/nginx/edge/nginx.conf
```

### 2. Pre-flight and backups

```sh
cd /mnt/data2/containers/stump-fork; S=/mnt/data2/Code/worktrees/stump/integ8
sqlite3 -readonly config/stump.db "select count(*) from jobs where status in ('RUNNING','QUEUED','PAUSED')"     # 0: a restart interrupts a running job
grep -c 'STUMP_EDGE_REQUIRE_PASS=true' docker-compose.yml; grep -c 'STUMP_EDGE_PUBLIC_SIGNIN=true' docker-compose.yml     # 1 and 1: leave the switch ON
grep -E '^trust_proxy_headers *= *true' config/Stump.toml || echo 'STOP: trust_proxy_headers is not true'
T=$(date -u +%Y%m%dT%H%M%SZ); B=config.backup-$T-db; mkdir $B && sqlite3 config/stump.db ".backup $B/stump.db"; echo $B
cp config/Stump.toml $B/; cp -a edge $B/edge; cp $S/crates/migrations/manual-down/m20261116_000000_access_warning_reviews.sql $B/manual-down.sql
cp docker-compose.yml docker-compose.yml.bak-$(date +%Y%m%d)-integ8
cp -n edge/nginx.conf edge/nginx.conf.bak-$(date +%Y%m%d)-before-publogin; cp -n edge/stump-gate.js edge/stump-gate.js.bak-$(date +%Y%m%d)-before-publogin   # -n: a second run never overwrites the first backup
```

### 3. Rehearse the migration on a copy (never on the live database)

```sh
M=/tmp/migtest-integ8; rm -rf $M; docker rm -f migtest-integ8 >/dev/null 2>&1; mkdir -p $M/config; cp $B/stump.db $B/Stump.toml $M/config/
docker run -d --name migtest-integ8 --network none -v $M/config:/config -e PUID=1000 -e PGID=1000 -e STUMP_CONFIG_DIR=/config \
  -e STUMP_EDGE_KEY=$(head -c 32 /dev/urandom | xxd -p -c 64) stump:fork-<rev> >/dev/null
for i in $(seq 1 40); do sqlite3 -readonly $M/config/stump.db "select version from seaql_migrations order by version desc limit 1" 2>/dev/null | grep -q m20261116 && break; sleep 3; done
docker stop migtest-integ8 >/dev/null; docker rm migtest-integ8 >/dev/null     # (a file-watcher and local-address error in its log is the missing network and library: harmless)
sqlite3 -readonly $M/config/stump.db "select version from seaql_migrations order by version desc limit 1"     # m20261116_000000_access_warning_reviews
sqlite3 $M/config/stump.db < $B/manual-down.sql                                                                # prints 0 and 0
sqlite3 -readonly $M/config/stump.db "pragma integrity_check; select version from seaql_migrations order by version desc limit 1"     # ok, then m20261115_000000_friend_onboarding
rm -rf $M
```

### 4. The app (friends see a short outage while it restarts)

Put `stump:fork-<rev>` on the shared `x-stump-image` line, with a comment above it: `integ/friends-8 <rev>`, the date, the migration
`m20261116_000000_access_warning_reviews`, and the rollback (`stump:fork-43fa3fe50` **after** `<snapshot folder>/manual-down.sql`, plus
`docker-compose.yml.bak-<date>-integ8` and the `edge/*.bak-<date>-before-publogin` files). Then recreate **both** containers:

```sh
docker compose config -q && docker compose up -d --no-build stump-fork stump-intake-probe
for i in $(seq 1 40); do [ "$(curl -s -o /dev/null -w %{http_code} http://127.0.0.1:10801/api/v2/ping)" = 200 ] && break; sleep 3; done
docker logs --since 3m stump-fork 2>&1 | grep -iE 'panic| error' | grep -v FORCE_DB | head -3                           # nothing
docker logs --since 3m stump-intake-probe 2>&1 | grep -cE 'WARN|FAIL'                                               # 0
docker logs --since 3m stump-intake-probe 2>&1 | grep -o 'stump-intake-probe [0-9a-f]* ready' | tail -1            # `stump-intake-probe <rev> ready`
echo "home $(curl -s -o /dev/null -w %{http_code} http://127.0.0.1:10801/api/v2/ping) tailnet $(curl -s -o /dev/null -w %{http_code} https://g3.tail3e401.ts.net:10802/api/v2/ping)"     # 200 200
sqlite3 -readonly config/stump.db "select version from seaql_migrations order by version desc limit 1"             # m20261116_000000_access_warning_reviews
curl -s http://127.0.0.1:10803/ | grep -c 'action="/api/_i/signin"'                                                # 1: the OLD edge still shows the server-made page
```

Then look, as the owner at home, at *Public access* (the attention list, *Dismiss* and *Undo*, the badge on the settings button) and, as a friend's
test account, at *My devices*.

### 5. The edge: this is the switch

```sh
cp edge-new/nginx.conf edge/nginx.conf && cp edge-new/stump-gate.js edge/stump-gate.js        # only these two: the live revoked.map stays
docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf && docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf
sleep 3; docker logs --since 1m stump-edge 2>&1 | grep -c emerg                         # 0: a failed reload would keep the OLD config running
docker exec stump-edge grep -c 'X-Edge-Login' /etc/nginx/edge/nginx.conf                # 2 or more: the new file is the one running
bash $S/deploy/public-edge/verify-edge.sh --live                                        # on g3: the live listener, incl. the policy of the pass-less shell
curl -s http://127.0.0.1:10803/ | grep -c 'id="root"'                                  # 1: the app's own shell
curl -s -D- -o /dev/null http://127.0.0.1:10803/ | grep -iE '^(content-security-policy|cache-control)'     # a policy, and no-store
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:10803/api/v2/auth/viewer       # 401: a pass-less viewer
```

### 6. Walk through it once, in a private window, then clean up

As *Public login* step 4 (a throwaway signup link made with **Invite someone**, its account, the join screen, a sign-in on another browser, a wrong
password, the fourth device), then clean up the same way (revoke the link with its accounts turned off, delete the account, remove the temporary files).

### 7. Tell the owner

*Public access* is the new page (*Needs your attention* with *Dismiss* and *Clear all*, *People* with **Invite someone**, a collapsed *History*); friends
see the app's own login screen; the interface is complete in Spanish; search puts the best title first and has an *Authors* group; the cards say what
the numbers count. Funnel stays as it is.

### Back out (fastest first)

```sh
cd /mnt/data2/containers/stump-fork
# R1: the EDGE only (about a second, no restart). The app goes back to the server-made pages by itself; the rest of the release stays
cp edge/nginx.conf.bak-<date>-before-publogin edge/nginx.conf && cp edge/stump-gate.js.bak-<date>-before-publogin edge/stump-gate.js
docker exec stump-edge nginx -t -c /etc/nginx/edge/nginx.conf && docker exec stump-edge nginx -s reload -c /etc/nginx/edge/nginx.conf

# R2: the IMAGE. Only after R1: the new edge has no page to serve with the old image. NEVER the previous image alone: it does not start on a
# database that holds m20261116 (Stump migrates on start and an older image refuses a migration it does not know)
tailscale funnel --https=8443 off
docker compose stop stump-fork stump-intake-probe
sqlite3 config/stump.db < config.backup-<UTC>-db/manual-down.sql                    # prints 0 and 0
cp docker-compose.yml.bak-<date>-integ8 docker-compose.yml                         # the x-stump-image line is stump:fork-43fa3fe50 again
docker compose up -d --no-build stump-fork stump-intake-probe
docker logs --since 2m stump-intake-probe 2>&1 | grep -o 'stump-intake-probe [0-9a-f]* ready' | tail -1     # wait for `ready`
tailscale funnel --bg --https=8443 http://127.0.0.1:10803
```

Restoring the snapshot (`<snapshot folder>/stump.db`) instead of running the manual down also works, but it loses everything done since: the revocations,
the accounts the links made, what friends read. What R2 loses: only which warnings the owner had dismissed (they all show again). Accounts, passes,
libraries and reading history stay.

### Known limits of this release (from the integration review)

* **`searchAuthors` and `media(relevanceTo:)` have no query cost of their own**, and the schema does not limit aliases: one friend request can ask for
  thousands of aliased calls. The same weakness already exists for `metadataValueCounts`; the public edge bounds it (4 connections, 20 s, 256 KB). A fixed
  cost for both fields would close it (a follow-up). Their inputs are capped: a relevance text 200 characters and 8 words, an author search 100 characters and 20 results.
* **A Spanish friend who joins gets an English app**: the join screen follows the browser's language, but the account is created with language `en`.
  They change it under *Settings → Account settings → Language*.
* **EPUBs at the http home address** (`http://192.168.0.17:10801`) lose their images, styles and fonts (the reader asks the browser to upgrade the book's
  files to https, and the home address serves plain http only). The tailnet and public addresses are https and unaffected. Not patched, by decision: at home, open Stump at `https://g3.tail3e401.ts.net:10802`.
* The clipped badge on the settings button noted in the usability report (*for the other squads*) is not fixed (`Settings.tsx` is untouched); nobody has looked at it on screen.

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
* **What the edge decides without a pass** (invite links, `/assets/manifest.webmanifest`, the favicons) it decides on the
  request line exactly as the client sent it (`$request_uri`), never on nginx's cleaned-up path: a dot segment, an escaped
  slash or `//` is an ordinary request that needs a pass. `verify-edge.sh` checks the tricks with and without a pass.
  The pass of a device that `edge/revoked.map` refuses is not forwarded to Stump either.
* **The home listener limits password sign-ins** (LAN, this host, the tailnet: the app never locks the owner's account):
  `POST /api/v2/auth/login` 10 a minute per address (burst 5, then `429`), HTTP Basic on `/opds` 60 a minute (burst 120: a
  reading app sends it on every request). API-key (Bearer) traffic, book-curator's, is never counted. If you lock
  yourself out on the LAN, wait a minute (all of the host's own processes share one address, 192.168.192.1).
* **`stump-pdf-worker` has no network** (`network_mode: none`), a read-only root, no capabilities and no new privileges:
  it talks to Stump only through `config/workers/pdf`. If a future worker needs the network, give it its own `internal: true`
  network; never the compose default one, where it would reach `stump-fork:10801` as "home".
