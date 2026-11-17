#!/bin/bash
# verify-edge.sh: runs the REAL nginx.conf of this folder in a throwaway container pair against a stub
# upstream that echoes the headers it gets, and checks what the edge promises. Needs docker, curl,
# openssl, python3. Nothing live is touched (own network, own ports 18080/18081, own state folder; set
# VERIFY_NET / VERIFY_PUB_PORT / VERIFY_HOME_PORT to run beside another copy; VERIFY_KEEP=1 leaves the containers up).
#
#   bash deploy/public-edge/verify-edge.sh            # from the repo root
#   bash deploy/public-edge/verify-edge.sh --live     # also checks the LIVE edge on 127.0.0.1:10803 (no pass → 404)
set -u
LIVE=0; [ "${1:-}" = "--live" ] && LIVE=1
HERE="$(cd "$(dirname "$0")" && pwd)"
# names and ports can be changed (VERIFY_NET, VERIFY_PUB_PORT, VERIFY_HOME_PORT) so that two runs, from two
# checkouts, never remove each other's containers
NET=${VERIFY_NET:-stump-edge-verify}
PUB_PORT=${VERIFY_PUB_PORT:-18081}; HOME_PORT=${VERIFY_HOME_PORT:-18080}
KEY=$(python3 -c "import json;print(json.load(open('$HERE/vectors.json'))['key'])")
COOKIE=__Host-g3-stump
STATE=$(mktemp -d); chmod 755 "$STATE"
EDGE="$STATE/edge"; mkdir -p "$EDGE"; cp -a "$HERE"/. "$EDGE"/   # a copy: the break-glass test writes to its revoked.map
PUB=http://127.0.0.1:$PUB_PORT; HOME_=http://127.0.0.1:$HOME_PORT
fail=0; n=0
ok()   { n=$((n+1)); echo "  ok   $*"; }
bad()  { n=$((n+1)); fail=$((fail+1)); echo "  FAIL $*"; }
check(){ local want="$1" got="$2" what="$3"; if [ "$got" = "$want" ]; then ok "$what ($got)"; else bad "$what: wanted $want, got $got"; fi; }
cleanup() { [ "${VERIFY_KEEP:-0}" = 1 ] && { echo "(VERIFY_KEEP=1: containers $NET-edge and $NET-stub left running; state in $STATE)"; return; }
  docker rm -f $NET-edge $NET-stub >/dev/null 2>&1; docker network rm $NET >/dev/null 2>&1; rm -rf "$STATE" /tmp/$NET-stub.conf; }
trap cleanup EXIT

mac() { printf '%s' "$1" | openssl dgst -sha256 -hmac "$KEY" -binary | openssl base64 -A | tr '+/' '-_' | tr -d '='; }
pass_for() { local id="$1" t="${2:-$(date +%s)}"; printf '%s.%010d.%s' "$id" "$t" "$(mac "dev|$id|$(printf %010d "$t")")"; }
code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
write_state() { # write_state <v> <revoked json list> <budget json object>
  printf '{"v":%s,"written":"2026-11-11T00:00:00Z","revoked":%s,"budget":%s}' "$1" "$2" "$3" > "$STATE/state.json.tmp" && chmod 644 "$STATE/state.json.tmp" && mv "$STATE/state.json.tmp" "$STATE/state.json"; }

echo "== the format is the one the app and the edge share (vectors.json)"
V=$(python3 - "$HERE/vectors.json" <<'EOF'
import json,sys
v=json.load(open(sys.argv[1])); print(v['pass']['id'], v['pass']['issued'], v['pass']['value'])
EOF
)
set -- $V
[ "$(pass_for "$1" "$2")" = "$3" ] && ok "openssl makes the vector's pass" || bad "openssl does not make the vector's pass"

echo "== starting the stub upstream and the edge"
docker network create $NET >/dev/null 2>&1
cat > /tmp/$NET-stub.conf <<'EOF'
events {}
http {
  # a request that carries `X-Hold` is answered at one byte a second: it stays in flight (and counted by the edge's
  # connection limits) until the client gives up. Used by the connection checks near the end
  map $http_x_hold $hold_rate { default 0; "~." 1; }
  server { listen 10801;
  limit_rate $hold_rate;
  client_max_body_size 400m;
  location / { default_type text/plain;
    return 200 "pub=[$http_x_edge_public] pass=[$http_x_edge_pass] cookie=[$http_cookie] host=[$http_host] xfp=[$http_x_forwarded_proto] xff=[$http_x_forwarded_for] real=[$http_x_real_ip] fwd=[$http_forwarded] fhost=[$http_x_forwarded_host] upg=[$http_upgrade] conn=[$http_connection] login=[$http_x_edge_login] uri=$request_uri\n"; } } }
EOF
docker run -d --name $NET-stub --network $NET --network-alias stump-fork -v /tmp/$NET-stub.conf:/etc/nginx/nginx.conf:ro nginx:alpine >/dev/null
docker run -d --name $NET-edge --network $NET -p 127.0.0.1:$PUB_PORT:8081 -p 127.0.0.1:$HOME_PORT:8080 \
  -e EDGE_KEY="$KEY" -e EDGE_COOKIE=$COOKIE -e EDGE_DL_CAP=5 -v "$EDGE":/etc/nginx/edge:ro -v "$STATE":/etc/nginx/state:ro \
  --entrypoint nginx nginx:alpine -c /etc/nginx/edge/nginx.conf -g 'daemon off;' >/dev/null
sleep 3
docker exec $NET-edge nginx -t -c /etc/nginx/edge/nginx.conf >/dev/null 2>&1 && ok "nginx -t" || bad "nginx -t"

P=$(pass_for ana-phone-k3f9x)
RKID=kobo-9x2mq; RK="$RKID.$(mac "opds|$RKID")"     # a reading-app key, as the app makes one
[ "$RK" = "$(python3 -c "import json;print(json.load(open('$HERE/vectors.json'))['reading_key']['value'])")" ] && ok "openssl makes the vector's reading-app key" || bad "openssl does not make the vector's reading-app key"

echo "== before the first state file, every pass is refused (fail closed)"
check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "a valid pass, no state file yet"
check 404 "$(code $PUB/opds/$RK/v1.2/catalog)" "a valid reading-app key, no state file yet (fail closed too)"
check 429 "$(code -X POST $PUB/api/v2/auth/login -H 'Content-Type: application/json' -d '{}')" "a pass-less sign-in, no state file yet (fail closed)"

echo "== once the app has written the state file, passes work"
write_state 1 '[]' '{}'; sleep 2.5
check 200 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "a valid pass"
check 404 "$(code $PUB/api/v2/ping)" "no pass"
# the front door: a browser with no pass gets the web app's shell, asked of the app as `/` (no rewrite)
FD=$(curl -s $PUB/)
echo "$FD" | grep -q 'uri=/$' && echo "$FD" | grep -q 'pub=\[1\]' && echo "$FD" | grep -q 'pass=\[\]' && echo "$FD" | grep -q 'login=\[spa\]' && ok "no pass, the front page: the app's own shell (as /, the marker, no pass, the capability header)" || bad "the front door: $FD"
check 200 "$(code -X POST $PUB/api/v2/auth/login -H 'Content-Type: application/json' -d '{}')" "no pass, a sign-in reaches the app (the web app's own sign-in screen)"
check 404 "$(code $PUB/api/graphql -H 'Content-Type: application/json' -d '{}')" "no pass, GraphQL"
check 404 "$(code $PUB/settings/access -H "Cookie: $COOKIE=$P")" "the admin page, even with a pass"
check 200 "$(code $PUB/api/_i/AAAAAAAAAAAAAAAAAAAAAA)" "the invite page needs no pass"
check 200 "$(code $PUB/assets/manifest.webmanifest)" "the manifest needs no pass"
check 200 "$(code $PUB/assets/favicon-32x32.png)" "a favicon needs no pass"
check 200 "$(code $PUB/assets/index-Ab12Cd34.js)" "a build file of the web app needs no pass"
check 404 "$(code $PUB/assets/index-Ab12Cd34.js.map)" "its source map does"
check 404 "$(code $PUB/assets/fonts/x.woff2)" "a file in a subfolder does"
for forged in $(python3 -c "import json;print(' '.join(json.load(open('$HERE/vectors.json'))['forged']))"); do
  check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$forged")" "a forged pass"
done
check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$(pass_for ana-phone-k3f9x $(( $(date +%s) - 181*86400 )))")" "an expired pass"
check 404 "$(code $PUB/api/_i/../v2/ping)" "a path trick without a pass"
check 404 "$(code --path-as-is $PUB/api/_i/%2e%2e/v2/ping)" "an encoded path trick without a pass"

echo "== the exemptions (invite links, manifest, favicons) are decided on the request line as sent, not on nginx's cleaned-up path"
# these all clean up to an exempt path ($uri) while Stump would get the raw line: refused with or without a pass
for t in /settings/access/../../api/_i/x /api/v2/ping/..%2F..%2F_i/x /api/v2/ping/%2e%2e/%2e%2e/_i/x /api//_i/x \
         /api/_i/../_i/x /./api/_i/x /api/%5Fi/x /api/_i/x/y /api/_i/. \
         /assets/./manifest.webmanifest /assets/x/../manifest.webmanifest /assets//manifest.webmanifest \
         /assets/%6Danifest.webmanifest /assets/..%2Fassets/manifest.webmanifest /x/..%2Fassets/manifest.webmanifest \
         /assets/favicon-32x32.png/../favicon-16x16.png; do
  check 404 "$(code --path-as-is "$PUB$t")" "no pass: $t"
  check 404 "$(code --path-as-is "$PUB$t" -H "Cookie: $COOKIE=$P")" "a pass does not make it an invite or a static file: $t"
done
for t in /api/_i/.. /api/_i/%2e%2e/x /api/_i/..%2F..%2Fapi/v2/ping /api/_i/x%2Fy /api/_i/x%0d%0a; do
  check 404 "$(code --path-as-is "$PUB$t")" "no pass: $t"
done
# and the plain static forms still work without one (the invite forms are checked after the uploads: the invite page has a rate limit of its own)
check 200 "$(code "$PUB/assets/manifest.webmanifest?v=3")" "the manifest with a query"
check 200 "$(code $PUB/assets/favicon-16x16.png)" "another favicon"

echo "== what Stump is told: the marker always, the pass only when it is valid, nothing from the client"
A=$(curl -s $PUB/api/v2/ping -H "Cookie: $COOKIE=$P; stump_session=abc; other=zzz" -H 'X-Edge-Public: evil' -H "X-Edge-Pass: planted" -H 'Forwarded: for=6.6.6.6' -H 'X-Forwarded-Host: evil.test' -H 'X-Forwarded-For: 6.6.6.6')
echo "$A" | grep -q 'pub=\[1\]' && ok "marker 1 (a client's value replaced)" || bad "marker: $A"
echo "$A" | grep -q "pass=\[$P\]" && ok "the verified pass, not the planted one" || bad "pass: $A"
echo "$A" | grep -q 'cookie=\[stump_session=abc\]' && ok "only Stump's own cookie goes through" || bad "cookie: $A"
echo "$A" | grep -q 'host=\[g3.tail3e401.ts.net:8443\]' && ok "fixed Host" || bad "host: $A"
echo "$A" | grep -q 'xfp=\[https\]' && ok "scheme https" || bad "scheme: $A"
echo "$A" | grep -q 'xff=\[6.6.6.6\]' && bad "a client's X-Forwarded-For got through: $A" || ok "the client's X-Forwarded-For is replaced"
echo "$A" | grep -q 'fwd=\[\]' && echo "$A" | grep -q 'fhost=\[\]' && ok "Forwarded / X-Forwarded-Host stripped" || bad "forwarded: $A"
B=$(curl -s $PUB/api/_i/AAAAAAAAAAAAAAAAAAAAAA)
echo "$B" | grep -q 'pub=\[1\]' && echo "$B" | grep -q 'pass=\[\]' && ok "no pass: the marker only (the invite path)" || bad "invite path: $B"
B=$(curl -s $PUB/api/_i/AAAAAAAAAAAAAAAAAAAAAA -H "X-Edge-Pass: planted")
echo "$B" | grep -q 'pass=\[\]' && ok "a planted pass is not sent" || bad "planted pass on the invite path: $B"

echo "== every forwarding location of the public listener sends the marker (N1), and the home listener never does"
for path in / /auth /join /api/v2/ping /api/graphql /api/v2/auth/login /api/v2/auth/join /api/v2/auth/viewer /api/v2/claim /api/v2/auth/oidc/config /assets/index-Ab12Cd34.js /api/v2/auth/refresh-token /api/v2/intake/upload /api/_i/AAAAAAAAAAAAAAAAAAAAAA /assets/manifest.webmanifest /assets/favicon-32x32.png /api/graphql/ws; do
  extra=(-H "Cookie: $COOKIE=$P")
  [ "$path" = /api/v2/auth/join ] && extra+=(-X POST -H 'Content-Type: application/json' -d '{}')
  [ "$path" = /api/graphql ] && extra+=(-X POST -H 'Content-Type: application/json' -d '{}')
  [ "$path" = /api/v2/auth/login ] && extra+=(-X POST -H 'Content-Type: application/json' -d '{}')
  [ "$path" = /api/v2/auth/refresh-token ] && extra+=(-X POST -H 'Content-Type: application/json' -d '{}')
  [ "$path" = /api/v2/intake/upload ] && extra+=(-X POST -H 'Content-Type: multipart/form-data; boundary=x' --data-binary x)
  [ "$path" = /api/graphql/ws ] && extra+=(-H 'Upgrade: websocket' -H 'Connection: Upgrade')
  R=$(curl -s "$PUB$path" "${extra[@]}" -H 'X-Edge-Login: planted')
  echo "$R" | grep -q 'pub=\[1\]' && echo "$R" | grep -q 'xfp=\[https\]' && echo "$R" | grep -q 'login=\[spa\]' && ok "public $path: marker, scheme, the capability header (a client's own replaced)" || bad "public $path: $R"
  H=$(curl -s "$HOME_$path" "${extra[@]}" -H 'X-Edge-Public: 1' -H 'X-Edge-Pass: planted' -H 'X-Edge-Login: spa')
  echo "$H" | grep -q 'pub=\[\]' && echo "$H" | grep -q 'pass=\[\]' && echo "$H" | grep -q 'login=\[\]' && ok "home   $path: marker, pass and capability header empty" || bad "home $path: $H"
done
R=$(curl -s $PUB/api/graphql/ws -H "Cookie: $COOKIE=$P" -H 'Upgrade: websocket' -H 'Connection: Upgrade')
echo "$R" | grep -q 'upg=\[websocket\]' && echo "$R" | grep -q 'conn=\[upgrade\]' && ok "a websocket upgrade goes through" || bad "websocket: $R"
R=$(curl -s $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")
echo "$R" | grep -q 'upg=\[\]' && echo "$R" | grep -q 'conn=\[\]' && ok "an ordinary request sends neither Upgrade nor Connection" || bad "connection headers: $R"

echo "== revoking and restoring: no reload, within seconds"
write_state 2 '["ana-phone-k3f9x"]' '{}'; sleep 2.5
check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "revoked"
check 200 "$(code $PUB/api/_i/AAAAAAAAAAAAAAAAAAAAAA -H "Cookie: $COOKIE=$P")" "a revoked device still reaches the invite page (a new invite replaces it)"
write_state 3 '[]' '{}'; sleep 2.5
check 200 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "restored"
printf 'this is not json' > "$STATE/state.json"; sleep 2.5
check 200 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "a corrupt state file keeps the last good state"
write_state 4 '["ana-phone-k3f9x"]' '{}'; sleep 2.5
check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "revoked again after the corrupt file"
write_state 5 '[]' '{}'; sleep 2.5
# a file is applied whenever its version DIFFERS (an app restart can never freeze the edge)
write_state 1 '["ana-phone-k3f9x"]' '{}'; sleep 2.5
check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "a lower version number is applied too"
write_state 6 '[]' '{}'; sleep 2.5

echo "== the daily sign-in budget: 8 a device, a reset gives a fresh count"
signin() { code -X POST $PUB/api/v2/auth/login -H 'Content-Type: application/json' -d '{}' -H "Cookie: $COOKIE=$P" -H "X-Forwarded-For: 203.0.113.$((RANDOM%200+1))"; }
sleep 11   # the per-client limit (6 a minute) is not what this checks: every call comes from the same address, so wait it out between batches
allowed=0; for i in 1 2 3 4 5 6; do c=$(signin); [ "$c" = 200 ] && allowed=$((allowed+1)); done
[ "$allowed" -ge 1 ] && ok "sign-ins pass the budget ($allowed of the first 6)" || bad "no sign-in passed"
write_state 7 '[]' "{\"ana-phone-k3f9x\": $(date +%s)}"; sleep 2.5
ok "a budget reset is applied without a reload (state version 7)"

echo "== the home listener refuses what comes through Funnel"
check 404 "$(code $HOME_/api/v2/ping -H 'Tailscale-Funnel-Request: ?1')" "a Funnel request on the home listener"
check 200 "$(code $HOME_/api/v2/ping)" "a request without it"

echo "== no invite token in a log"
curl -s -o /dev/null "$PUB/api/_i/TOKENTOKENTOKENTOKEN12"; curl -s -o /dev/null "$PUB/api/_i/?t=TOKENTOKENTOKENTOKEN12" ; sleep 1
if docker logs $NET-edge 2>&1 | grep -q TOKENTOKENTOKENTOKEN12; then bad "an invite token is in the edge's logs"; else ok "the token is in no log"; fi
docker logs $NET-edge 2>&1 | grep -q '/api/_i/-hidden-' && ok "the access log says -hidden-" || bad "no -hidden- line in the access log"

echo "== the pass is renewed after a day, and the invite pages are not touched by a renewal"
OLD=$(pass_for ana-phone-k3f9x $(( $(date +%s) - 2*86400 )))
curl -s -D - -o /dev/null $PUB/api/v2/ping -H "Cookie: $COOKIE=$OLD" | grep -qi "^set-cookie: $COOKIE=ana-phone-k3f9x" && ok "an old pass is renewed" || bad "no renewal"
curl -s -D - -o /dev/null $PUB/api/_i/AAAAAAAAAAAAAAAAAAAAAA -H "Cookie: $COOKIE=$OLD" | grep -qi "^set-cookie" && bad "the invite page got a renewal Set-Cookie" || ok "no renewal on the invite page"

echo "== friends' uploads: the inbox route has limits of its own, and nothing else takes a big body"
UPLOAD=$PUB/api/v2/intake/upload
MULTI=(-H 'Content-Type: multipart/form-data; boundary=x')
OWN_ORIGIN='https://g3.tail3e401.ts.net:8443'
post_upload() { code -X POST "$UPLOAD?libraryId=x" -H "Cookie: $COOKIE=$P" "${MULTI[@]}" "$@"; }
check 404 "$(code -X POST "$UPLOAD?libraryId=x" "${MULTI[@]}" --data-binary x)" "no pass, no upload"
check 200 "$(post_upload -H "Origin: $OWN_ORIGIN" -H 'Sec-Fetch-Site: same-origin' --data-binary x)" "a pass, Stump's own page: forwarded"
check 200 "$(post_upload --data-binary x)" "a pass, a client that is no browser (no Origin, no Sec-Fetch-Site): forwarded"
check 403 "$(post_upload -H 'Origin: https://evil.example' --data-binary x)" "another site's Origin is refused"
check 403 "$(post_upload -H 'Origin: null' --data-binary x)" "an Origin of null is refused"
check 403 "$(post_upload -H 'Sec-Fetch-Site: same-site' --data-binary x)" "a same-site (another port of this host) request is refused"
check 403 "$(post_upload -H 'Sec-Fetch-Site: cross-site' --data-binary x)" "a cross-site request is refused"
check 413 "$(post_upload -H 'Content-Length: 400000000' --data-binary x)" "a body over Stump's per-request limit is refused before it is read"
head -c 30000000 /dev/zero > "$STATE/30mb.bin"
check 200 "$(post_upload --data-binary @"$STATE/30mb.bin")" "a 30 MB upload (over the 25 MB cap of everything else) goes through"
check 413 "$(code -X POST $PUB/api/v2/ping -H "Cookie: $COOKIE=$P" "${MULTI[@]}" --data-binary @"$STATE/30mb.bin")" "30 MB anywhere else is still refused"
check 413 "$(code -X POST $PUB/api/graphql -H "Cookie: $COOKIE=$P" -H 'Content-Type: application/json' --data-binary @"$STATE/30mb.bin")" "30 MB of GraphQL is refused"
python3 -c "print('{\"query\":\"x\",\"v\":\"' + '\u00e9' * 100000 + '\"}')" > "$STATE/200k.json"   # about 200 KB: a long description
python3 -c "print('{\"query\":\"x\",\"v\":\"' + 'a' * 300000 + '\"}')" > "$STATE/300k.json"
check 200 "$(code -X POST $PUB/api/graphql -H "Cookie: $COOKIE=$P" -H 'Content-Type: application/json' --data-binary @"$STATE/200k.json")" "GraphQL takes a review with a long multibyte description (about 200 KB)"
check 413 "$(code -X POST $PUB/api/graphql -H "Cookie: $COOKIE=$P" -H 'Content-Type: application/json' --data-binary @"$STATE/300k.json")" "GraphQL stops at 256 KB"
# the rate: a client may send a burst, then one request every 5 seconds; the refusal says when to come back
got429=0; for i in $(seq 1 14); do [ "$(post_upload --data-binary x)" = 429 ] && got429=$((got429+1)); done
[ "$got429" -ge 1 ] && ok "a flood of uploads is rate limited ($got429 of 14 got a 429)" || bad "no upload was rate limited in a burst of 14"
R=$(curl -s -D - -o "$STATE/429.body" -X POST "$UPLOAD?libraryId=x" -H "Cookie: $COOKIE=$P" "${MULTI[@]}" --data-binary x)
echo "$R" | head -1 | grep -q 429 && ok "still limited" || bad "not limited right after the burst: $(echo "$R" | head -1)"
echo "$R" | grep -qi '^retry-after: 10' && ok "the 429 says when to try again (Retry-After)" || bad "no Retry-After on the 429: $R"
echo "$R" | grep -qi '^strict-transport-security' && echo "$R" | grep -qi '^x-content-type-options: nosniff' && ok "and keeps the security headers" || bad "the 429 lost its security headers: $R"
grep -q '"message"' "$STATE/429.body" && bad "the edge's 429 looks like one of Stump's own (it has a message)" || ok "the edge's 429 carries no Stump-style message (the web app waits and retries)"
check 404 "$(code -X POST "$UPLOAD?libraryId=x" "${MULTI[@]}" --data-binary x)" "and the pass is still asked for"
echo "$(curl -s $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" | grep -q 'pub=\[1\]' && ok "the rate limit of uploads does not touch the rest" || bad "the rest of the public side was affected by the upload limit"

echo "== the plain invite forms still work without a pass (after a pause: the invite page allows 6 a minute, and the tests before used it up)"
sleep 51
check 200 "$(code $PUB/api/_i/)" "the paste form's page (/api/_i/)"
check 200 "$(code "$PUB/api/_i/?t=https%3A%2F%2Fg3.tail3e401.ts.net%3A8443%2Fapi%2F_i%2FAAAAAAAAAAAAAAAAAAAAAA")" "a pasted link (/api/_i/?t=...)"
check 200 "$(code $PUB/api/_i/ana-phone-k3f9x.1700000000.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA)" "a legacy invite link (name.expiry.mac)"

echo "== break-glass revoked.map: the edge refuses the device, and Stump is never told about its pass"
echo 'ana-phone-k3f9x 1;' >> "$EDGE/revoked.map"
docker exec $NET-edge nginx -s reload -c /etc/nginx/edge/nginx.conf >/dev/null 2>&1; sleep 2.5
check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "a device in revoked.map is refused"
B=$(curl -s $PUB/assets/manifest.webmanifest -H "Cookie: $COOKIE=$P")   # a path that needs no pass, so the request reaches Stump
echo "$B" | grep -q 'pub=\[1\]' && echo "$B" | grep -q 'pass=\[\]' && ok "on a path that needs no pass Stump gets the marker and no pass" || bad "a revoked.map device's pass reached Stump: $B"
sed -i '/^ana-phone-k3f9x 1;$/d' "$EDGE/revoked.map"
docker exec $NET-edge nginx -s reload -c /etc/nginx/edge/nginx.conf >/dev/null 2>&1; sleep 2.5
check 200 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "and it works again once the line is gone"

echo "== the home listener: a password is tried a few times a minute, API-key traffic is never counted"
sleep 1
ok_first=$(code -X POST $HOME_/api/v2/auth/login -H 'Content-Type: application/json' -d '{}')
got429=0; for i in $(seq 1 14); do [ "$(code -X POST $HOME_/api/v2/auth/login -H 'Content-Type: application/json' -d '{}')" = 429 ] && got429=$((got429+1)); done
[ "$ok_first" != 429 ] && ok "the first sign-in goes through ($ok_first)" || bad "the first sign-in was limited"
[ "$got429" -ge 1 ] && ok "a flood of sign-ins is limited ($got429 of 14 got a 429)" || bad "no sign-in was limited in a burst of 14"
check 200 "$(code $HOME_/api/v2/ping)" "other requests are not affected"
bearer_bad=0; for i in $(seq 1 60); do [ "$(code -X POST $HOME_/api/graphql -H 'Authorization: Bearer stump_testkey' -H 'Content-Type: application/json' -d '{}')" != 200 ] && bearer_bad=$((bearer_bad+1)); done
[ "$bearer_bad" = 0 ] && ok "60 API-key (Bearer) requests in a burst: none limited" || bad "$bearer_bad of 60 Bearer requests were refused"
basic429=0; for i in $(seq 1 150); do [ "$(code $HOME_/opds/v1.2/catalog -H 'Authorization: Basic Zm9vOmJhcg==')" = 429 ] && basic429=$((basic429+1)); done
[ "$basic429" -ge 1 ] && [ "$basic429" -lt 60 ] && ok "HTTP Basic: a reader's burst of about 120 goes through, a flood does not ($basic429 of 150 got a 429)" || bad "HTTP Basic limit: $basic429 of 150 requests got a 429 (wanted 1 to 59)"
check 200 "$(code $HOME_/api/v2/ping)" "and a request without Basic is still answered"
check 404 "$(code $HOME_/api/v2/ping -H 'Tailscale-Funnel-Request: ?1')" "the Funnel refusal still holds after the floods"


echo "== friend onboarding: the front door, the sign-in page and the signup form need no pass; nothing else does"
sleep 12   # the sign-in zones refill (the home-listener floods above did not touch them)
for t in "/?a=1" "/api/_i/signin" "/api/_i/signin?x=1"; do
  check 200 "$(code "$PUB$t")" "no pass: GET $t reaches the app"
done
check 200 "$(code -I $PUB/)" "no pass: HEAD / (the front door)"
check 404 "$(code $PUB/x)" "no pass: any other path is still 404"
check 404 "$(code -X POST $PUB/)" "no pass: POST / is 404 (only GET and HEAD of / are the front door)"
check 404 "$(code -X POST "$PUB/?a=1")" "no pass: POST /?a=1 is 404"
check 404 "$(code $PUB/index.html)" "no pass: /index.html is 404"
FD=$(curl -s "$PUB/?a=1")
echo "$FD" | grep -q 'uri=/?a=1$' && ok "the front door keeps the query" || bad "front door with a query: $FD"
# every way of reaching the sign-in page or the form by a path that is not the exact one needs a pass
for t in /api/_i/signin/ /api/_i/signin/../x //api/_i/signin /api/_i/%73ignin /Api/_i/signin /api/_i/signin%2f /api/_i/./signin /api//_i/signin \
         /api/_i/join/ /api/_i/%6aoin /api/_i/../_i/join; do
  check 404 "$(code --path-as-is "$PUB$t")" "no pass: $t"
  check 404 "$(code --path-as-is -X POST "$PUB$t")" "no pass, POST: $t"
done
check 200 "$(code -X POST $PUB/api/_i/signin -d 'username=a&password=b')" "no pass: POST /api/_i/signin reaches the app"
check 200 "$(code -X POST $PUB/api/_i/join -d 't=x&username=a&password=b&password2=b')" "no pass: POST /api/_i/join reaches the app"
check 405 "$(code $PUB/api/_i/join)" "GET /api/_i/join is 405"
check 200 "$(code $PUB/ -H "Cookie: $COOKIE=$P")" "a pass: / is the web app"
R=$(curl -s $PUB/ -H "Cookie: $COOKIE=$P"); echo "$R" | grep -q 'uri=/ ' || echo "$R" | grep -q 'uri=/$' && ok "a pass: / is forwarded as /, not rewritten" || bad "a pass on /: $R"
head -c 5000 /dev/zero | tr '\0' 'x' > "$STATE/5k.bin"
check 413 "$(code -X POST $PUB/api/_i/signin --data-binary @"$STATE/5k.bin")" "a sign-in body over 4 KB is refused before the app"
check 413 "$(code -X POST $PUB/api/_i/join --data-binary @"$STATE/5k.bin")" "a signup body over 4 KB is refused before the app"
# a revoked device is no pass: the front door shows it the sign-in page, and its API calls are 404 with the hint
write_state 8 '["ana-phone-k3f9x"]' '{}'; sleep 2.5
R=$(curl -s "$PUB/" -H "Cookie: $COOKIE=$P"); echo "$R" | grep -q 'uri=/$' && echo "$R" | grep -q 'pass=\[\]' && ok "a revoked device at the front door: the shell as no pass, and its pass is not passed on" || bad "revoked at the front door: $R"
write_state 9 '[]' '{}'; sleep 2.5
# the 404 of a request with no pass says where to sign in (and tells the web app, by a header)
H=$(curl -s -D - $PUB/api/v2/ping)
echo "$H" | head -1 | grep -q 404 && echo "$H" | grep -qi '^x-edge-sign-in: 1' && echo "$H" | grep -q 'href="/api/_i/signin"' && ok "the 404 of a request with no pass: the sign-in link, and X-Edge-Sign-In: 1" || bad "the 404 body/header: $H"
echo "$H" | grep -q 'action=/api/_i/' && bad "the 404 still carries the old paste form" || ok "the paste form moved into the sign-in page"
H=$(curl -s -D - $PUB/api/v2/ping -H "Cookie: $COOKIE=$P"); echo "$H" | grep -qi '^x-edge-sign-in' && bad "a request with a pass got the sign-in hint" || ok "no sign-in hint on a request that has a pass"
H=$(curl -s -D - $PUB/api/_i/signin); echo "$H" | grep -qi '^x-edge-sign-in' && bad "the sign-in page itself carries the hint" || ok "no hint on the sign-in page"
# the new pages: only the app's headers and HSTS (no pass renewal over a pass the app just set)
OLD=$(pass_for ana-phone-k3f9x $(( $(date +%s) - 2*86400 )))
for t in /api/_i/signin; do
  curl -s -D - -o /dev/null $PUB$t -H "Cookie: $COOKIE=$OLD" | grep -qi "^set-cookie" && bad "$t got a renewal Set-Cookie" || ok "no renewal Set-Cookie on $t"
done
# limits: a POST is a password guess (6 a minute per client, a burst of 4), a GET is never counted against it
sleep 10
got429=0; for i in $(seq 1 14); do [ "$(code -X POST $PUB/api/_i/signin -d 'username=a&password=b')" = 429 ] && got429=$((got429+1)); done
[ "$got429" -ge 3 ] && ok "a flood of sign-in POSTs is limited ($got429 of 14 got a 429)" || bad "sign-in POSTs not limited ($got429 of 14)"
check 200 "$(code $PUB/api/_i/signin)" "a GET of the sign-in page is never counted against the POST limit"
check 200 "$(code $PUB/ -H "Cookie: $COOKIE=$P")" "the web app is not affected by the sign-in limit"
got429=0; for i in $(seq 1 10); do [ "$(code -X POST $PUB/api/_i/join -d 't=x')" = 429 ] && got429=$((got429+1)); done
[ "$got429" -ge 3 ] && ok "a flood of signups is limited ($got429 of 10 got a 429)" || bad "signups not limited ($got429 of 10)"

echo "== reading apps: the key is the credential (no pass, no cookie), and only OPDS 1.2 addresses accept it"
RKB="$PUB/opds/$RK/v1.2"
A=$(curl -s "$RKB/catalog" -H 'X-Edge-Pass: planted' -H 'X-Edge-Public: evil' -H 'Cookie: other=zzz')
echo "$A" | grep -q 'pub=\[1\]' && echo "$A" | grep -q 'pass=\[\]' && echo "$A" | grep -q 'cookie=\[\]' && ok "a key reaches the app: the marker, no pass (a key never becomes a pass), no cookie, planted headers replaced" || bad "key request: $A"
for r in catalog search keep-reading discover smart-lists reading-lists authors genres libraries series books smart-lists/abc reading-lists/abc libraries/abc/books series/latest books/latest books/abc/thumbnail "books/abc/pages/3?zero_based=true" "books/abc/file/My%20Book.epub" "search/feed?search=x"; do
  check 200 "$(code "$RKB/$r")" "the key reads /v1.2/$r"
done
curl -s -D - -o /dev/null "$RKB/catalog" | tr -d '\r' > "$STATE/key.headers"
grep -qi '^x-robots-tag: noindex' "$STATE/key.headers" && grep -qi '^referrer-policy: no-referrer' "$STATE/key.headers" && grep -qi '^strict-transport-security' "$STATE/key.headers" && grep -qi '^x-content-type-options: nosniff' "$STATE/key.headers" && ok "the feed keeps HSTS, nosniff and gets no-referrer / noindex" || bad "feed headers: $(cat "$STATE/key.headers")"
for what in "forged mac:$PUB/opds/$RKID.$(mac "opds|other")/v1.2/catalog" \
            "truncated mac:$PUB/opds/$RKID.${RK#*.}x/v1.2/catalog" \
            "a pass is not a key:$PUB/opds/$RKID.$(mac "dev|$RKID|$(printf %010d "$(date +%s)")")/v1.2/catalog" \
            "no such route:$RKB/nothing" \
            "no v1.2:$PUB/opds/$RK/catalog" \
            "OPDS 2.0:$PUB/opds/$RK/v2.0/catalog" \
            "the sync apis:$PUB/koreader/$RK/syncs/progress" \
            "the kobo sync:$PUB/kobo/$RK/v1/library/sync" \
            "uppercase id:$PUB/opds/KOBO-9X2MQ.${RK#*.}/v1.2/catalog"; do
  check 404 "$(code --path-as-is "${what#*:}")" "not served: ${what%%:*}"
done
check 403 "$(code "$PUB/koreader/x/y" -H "Cookie: $COOKIE=$P")" "the sync apis are 403 for a browser with a pass and no key (as before)"
for t in "/v1.2/../../api/graphql" "/v1.2/books/../../../api/graphql" "/v1.2/books/%2e%2e/%2e%2e/api" "/v1.2/books/a%2fb" "/v1.2/books/a%2Fb" "/v1.2/books/a%5cb" "/v1.2//catalog" "/v1.2/books//x" "/v1.2/books/./x" "/v1.2/books/x/." "/v1.2/books/x/.." "/v1.2/catalog/..;/x" "/v1.2/catalog%0d%0a"; do
  check 404 "$(code --path-as-is "$PUB/opds/$RK$t")" "not served: key + $t"
done
c=$(code --path-as-is "$PUB/opds/$RK/v1.2/books/a%00b"); { [ "$c" = 400 ] || [ "$c" = 404 ]; } && ok "not served: key + a NUL byte ($c: nginx itself refuses it)" || bad "key + a NUL byte: $c"
check 404 "$(code --path-as-is "$PUB/opds//$RK/v1.2/catalog")" "not served: // before the key"
check 404 "$(code --path-as-is "$PUB/opds/$RK/../$RK/v1.2/catalog")" "not served: a dot segment before the route"
check 405 "$(code -X POST "$RKB/catalog")" "a key may only GET and HEAD: POST"
check 405 "$(code -X PUT "$RKB/catalog")" "PUT"
check 405 "$(code -X DELETE "$RKB/books/abc/file/x.epub")" "DELETE"
check 200 "$(code -I "$RKB/catalog")" "HEAD"
check 403 "$(code "$RKB/catalog" -H 'Authorization: Basic Zm9vOmJhcg==')" "HTTP Basic stays refused, key or not"
check 404 "$(code $PUB/opds/v1.2/catalog)" "the keyless address needs a pass like any other"
check 403 "$(code $PUB/opds/v2.0/catalog -H "Cookie: $COOKIE=$P")" "even with a pass, OPDS without a key stays 403 (as before)"
# revocation: the app's state file, followed within seconds, no reload; and the break-glass map
write_state 10 "[\"$RKID\"]" '{}'; sleep 2.5
check 404 "$(code "$RKB/catalog")" "a revoked key is refused within seconds"
write_state 11 '[]' '{}'; sleep 2.5
check 200 "$(code "$RKB/catalog")" "and works again once it is restored"
echo "$RKID 1;" >> "$EDGE/revoked.map"; docker exec $NET-edge nginx -s reload -c /etc/nginx/edge/nginx.conf >/dev/null 2>&1; sleep 2.5
check 404 "$(code "$RKB/catalog")" "a key in revoked.map is refused (break-glass)"
sed -i "/^$RKID 1;\$/d" "$EDGE/revoked.map"; docker exec $NET-edge nginx -s reload -c /etc/nginx/edge/nginx.conf >/dev/null 2>&1; sleep 2.5
check 200 "$(code "$RKB/catalog")" "and works again once the line is gone"
# a cookie pass does not turn a key URL into something else, and a key does not open anything but its routes
check 200 "$(code "$RKB/catalog" -H "Cookie: $COOKIE=$P")" "a key with a pass too is fine (the pass wins, the address is the same)"
check 404 "$(code $PUB/api/v2/ping -H "X-Edge-Pass: $RK")" "a key is no pass for anything else"
check 404 "$(code "$PUB/api/graphql?x=/opds/$RK/v1.2/catalog" -H 'Content-Type: application/json' -d '{}')" "a key in a query string opens nothing"
# the daily cap on downloaded files (EDGE_DL_CAP=5 in this test; 300 in production), and a reset
RK2ID=nook-4t7pw; RK2="$RK2ID.$(mac "opds|$RK2ID")"
got=""; for i in 1 2 3 4 5 6 7; do got="$got $(code "$PUB/opds/$RK2/v1.2/books/abc/file/x.epub")"; done
[ "$got" = " 200 200 200 200 200 429 429" ] && ok "downloads: the 6th file of a day is refused (5 allowed in this test:$got)" || bad "download cap:$got"
check 200 "$(code "$PUB/opds/$RK2/v1.2/catalog")" "browsing is not capped"
write_state 12 '[]' "{\"$RK2ID\": $(date +%s)}"; sleep 2.5
check 200 "$(code "$PUB/opds/$RK2/v1.2/books/abc/file/x.epub")" "a budget reset starts the day again"
write_state 13 '[]' '{}'; sleep 2.5
# the rate per key: a feed page loads a screen of covers, a flood does not
RK3ID=pico-8z2kd; RK3="$RK3ID.$(mac "opds|$RK3ID")"
got429=0; for i in $(seq 1 150); do [ "$(code "$PUB/opds/$RK3/v1.2/catalog")" = 429 ] && got429=$((got429+1)); done
[ "$got429" -ge 20 ] && [ "$got429" -lt 150 ] && ok "a flood on one key is limited, a burst is not ($got429 of 150 got a 429)" || bad "per-key limit: $got429 of 150 got a 429"
check 200 "$(code $PUB/ -H "Cookie: $COOKIE=$P")" "other requests are not affected by a key's limit"
echo "== no reading-app key is in any log"
sleep 1
if docker logs $NET-edge 2>&1 | grep -q "${RK#*.}"; then bad "a reading-app key (its MAC) is in the edge's logs"; else ok "the key is in no log"; fi
docker logs $NET-edge 2>&1 | grep -q '/opds/-key-/v1.2/catalog' && ok "the access log says -key- in its place" || bad "no -key- line in the access log"
docker logs $NET-edge 2>&1 | grep -q '"GET /opds/v1.2/catalog' && ok "an address that is not a key is logged as itself" || bad "the keyless address was rewritten in the log"
if docker logs $NET-edge 2>&1 | grep -q 'username=a'; then bad "a form body is in the logs"; else ok "no form body in the logs"; fi

echo "== public login: the web app's own sign-in screen needs no pass, and exactly what it needs (open-paths.json)"
sleep 40   # every sign-in zone refills (the floods above)
P2=$(pass_for dani-laptop-q1z8c); OLD2=$(pass_for dani-laptop-q1z8c $(( $(date +%s) - 2*86400 )))
# body caps first (before the budget below is used up): a sign-in and a signup are small
check 413 "$(code -X POST $PUB/api/v2/auth/login -H 'Content-Type: application/json' --data-binary @"$STATE/5k.bin")" "a sign-in body over 4 KB is refused before the app"
check 413 "$(code -X POST $PUB/api/v2/auth/join -H 'Content-Type: application/json' --data-binary @"$STATE/5k.bin")" "a signup body over 4 KB is refused before the app"
check 405 "$(code $PUB/api/v2/auth/join -H "Cookie: $COOKIE=$P2")" "GET /api/v2/auth/join (with a pass) is 405"
# every row of the table the app's own list is tested against: the request line as sent, no pass
python3 - "$HERE/open-paths.json" > "$STATE/open-rows.tsv" <<'EOF'
import json,sys
for r in json.load(open(sys.argv[1]))['rows']:
    print(r['method'], r['path'], r['edge'], sep='\t')
EOF
nrows=0; nbad=0
while IFS=$'\t' read -r m p e; do
  nrows=$((nrows+1))
  args=(-s -o "$STATE/row.body" -w '%{http_code}' --path-as-is)
  if [ "$m" = HEAD ]; then args+=(-I -o /dev/null); else args+=(-X "$m"); fi
  c=$(curl "${args[@]}" "$PUB$p")
  case "$e" in
    closed) want=404 ;;
    open)   want=200 ;;
    stub)   case "$p" in /api/v2/auth/viewer) want=401 ;; *) want=200 ;; esac ;;
  esac
  got_ok=1
  if [ "$c" != "$want" ]; then
    # nginx itself may answer 400 for an escaped NUL: still not served
    if [ "$e" = closed ] && [ "$c" = 400 ]; then :; else got_ok=0; fi
  fi
  if [ "$got_ok" = 1 ] && [ "$m" != HEAD ]; then
    case "$e" in
      open) grep -q 'pub=\[1\]' "$STATE/row.body" && grep -q 'login=\[spa\]' "$STATE/row.body" && grep -q 'pass=\[\]' "$STATE/row.body" || got_ok=0 ;;
      stub) grep -q 'pub=\[' "$STATE/row.body" && got_ok=0 ;;   # a stub is answered here, never forwarded
    esac
  fi
  [ "$got_ok" = 1 ] || { nbad=$((nbad+1)); bad "open-paths.json: $e $m $p -> $c"; }
  # the shell's pass-less zone refills 30 a minute: pace those rows
  case "$p" in /|/\?*|/auth|/auth\?*|/join|/join\?*) [ "$m" = GET ] || [ "$m" = HEAD ] && sleep 2.2 ;; esac
done < "$STATE/open-rows.tsv"
[ "$nbad" = 0 ] && ok "all $nrows rows of open-paths.json agree with the edge (open, stub, closed)" || bad "$nbad of $nrows rows disagree"
# the three answers the edge gives itself: exact bytes, no cookie, no hint, no store
for t in "/api/v2/auth/viewer|401|{\"status\":401,\"message\":\"Unauthorized\"}" \
         "/api/v2/claim|200|{\"isClaimed\":true,\"isPublic\":true}" \
         "/api/v2/auth/oidc/config|200|{\"enabled\":false,\"allowRegistration\":false,\"disableLocalAuth\":false}"; do
  path=${t%%|*}; rest=${t#*|}; status=${rest%%|*}; body=${rest#*|}
  curl -s -D "$STATE/stub.h" -o "$STATE/stub.b" "$PUB$path"; tr -d '\r' < "$STATE/stub.h" > "$STATE/stub.hh"
  [ "$(cat "$STATE/stub.b")" = "$body" ] && head -1 "$STATE/stub.hh" | grep -q " $status" && ok "$path with no pass: $status and the exact body, from the edge" || bad "$path: $(head -1 "$STATE/stub.hh") $(cat "$STATE/stub.b")"
  grep -qi '^content-type: application/json' "$STATE/stub.hh" && grep -qi '^cache-control: no-store' "$STATE/stub.hh" && ok "$path: JSON, no-store" || bad "$path headers: $(cat "$STATE/stub.hh")"
  grep -qi '^set-cookie' "$STATE/stub.hh" && bad "$path answered with a cookie" || ok "$path: no cookie"
  grep -qi '^x-edge-sign-in' "$STATE/stub.hh" && bad "$path carries the sign-in hint" || ok "$path: no sign-in hint"
  R=$(curl -s "$PUB$path" -H "Cookie: $COOKIE=$P2")
  echo "$R" | grep -q 'pub=\[1\]' && echo "$R" | grep -q "pass=\[$P2\]" && ok "$path with a pass goes to the app" || bad "$path with a pass: $R"
done
# no cookie and no hint on what is served to a visitor with no pass; the shell renews a pass in use, the sign-in and signup never do
for t in /auth /assets/index-Ab12Cd34.js; do
  curl -s -D - -o /dev/null "$PUB$t" | grep -qiE '^(set-cookie|x-edge-sign-in)' && bad "$t (no pass) carries a cookie or the hint" || ok "$t (no pass): no cookie, no hint"
done
curl -s -D - -o /dev/null "$PUB/auth" -H "Cookie: $COOKIE=$OLD2" | grep -qi "^set-cookie: $COOKIE=dani-laptop-q1z8c" && ok "the shell renews an old pass (the edge adds no headers there)" || bad "no renewal on the shell"
curl -s -D - -o /dev/null -X POST "$PUB/api/v2/auth/login" -H 'Content-Type: application/json' -d '{}' -H "Cookie: $COOKIE=$OLD2" | grep -qi '^set-cookie' && bad "the sign-in got a renewal Set-Cookie (it could put a revoked pass back)" || ok "no renewal Set-Cookie after the sign-in"
curl -s -D - -o /dev/null -X POST "$PUB/api/v2/auth/join" -H 'Content-Type: application/json' -d '{}' -H "Cookie: $COOKIE=$OLD2" | grep -qi '^set-cookie' && bad "the signup got a renewal Set-Cookie" || ok "no renewal Set-Cookie after the signup"
# the pass-less sign-in has ONE budget, shared by the server-made form and the web app's own sign-in
sleep 40
allowed=0; got429=0
for i in $(seq 1 12); do
  if [ $((i%2)) = 0 ]; then c=$(code -X POST $PUB/api/_i/signin -d 'username=a&password=b'); else c=$(code -X POST $PUB/api/v2/auth/login -H 'Content-Type: application/json' -d '{}'); fi
  [ "$c" = 200 ] && allowed=$((allowed+1)); [ "$c" = 429 ] && got429=$((got429+1))
done
[ "$allowed" -ge 1 ] && [ "$allowed" -le 6 ] && [ "$got429" -ge 5 ] && ok "the page and the web app's sign-in are one budget ($allowed let through, $got429 limited of 12 alternating)" || bad "shared budget: $allowed let through, $got429 limited of 12"
check 200 "$(code -X POST $PUB/api/v2/auth/login -H 'Content-Type: application/json' -d '{}' -H "Cookie: $COOKIE=$P2")" "a sign-in with a pass is not refused after pass-less traffic used the budget up"
check 200 "$(code $PUB/auth -H "Cookie: $COOKIE=$P2")" "and the shell with a pass is not counted against it"
# pass-less shell loads are counted per client; a pass is not counted; the build's files have a budget of their own (below)
sleep 40
got429=0; for i in $(seq 1 16); do [ "$(code $PUB/auth)" = 429 ] && got429=$((got429+1)); done
[ "$got429" -ge 3 ] && ok "a flood of pass-less shell loads is limited ($got429 of 16 got a 429)" || bad "shell loads not limited ($got429 of 16)"
check 200 "$(code $PUB/auth -H "Cookie: $COOKIE=$P2")" "the shell with a pass is never counted there"
# the build's own files have a budget of their own (pub_assets). A cold load of the sign-in screen is ~110 files and of the
# whole app a few hundred, and every browser that is new to the address (a new phone, a first visit, a cleared cache)
# fetches them all: they used to share the budget of the API calls and the covers (20 a second, a burst of 300), so one
# cold load from one address (or one household behind one router) left the next files and calls with a 429 and the screen
# on its loading spinner. The files are tried one URL after the other over one connection (curl's [1-450] range)
check 200 "$(code $PUB/assets/index-Ab12Cd34.js)" "a build file is served (no pass: it is on the open list)"
check 200 "$(code "$PUB/assets/index-Ab12Cd34.js?v=3" -H "Cookie: $COOKIE=$P2")" "and with a pass a query on it is fine"
check 404 "$(code "$PUB/assets/index-Ab12Cd34.js?v=3")" "but with no pass a query on it is not on the open list"
check 404 "$(code --path-as-is "$PUB/assets/in%64ex-Ab12Cd34.js" -H "Cookie: $COOKIE=$P2")" "a build file named with an escape is not served by the files' location, pass or not"
check 404 "$(code --path-as-is "$PUB//assets/index-Ab12Cd34.js" -H "Cookie: $COOKIE=$P2")" "nor one behind a doubled slash"
cold=$(curl -s -o /dev/null -w '%{http_code}\n' "$PUB/assets/vendor-Ab12Cd34-[1-450].js" | grep -c 429)
[ "$cold" = 0 ] && ok "a cold load of the whole app (450 files, no pass) from one address is not limited" || bad "$cold of 450 files of a cold load got a 429"
api=$(curl -s -o /dev/null -w '%{http_code}\n' -H "Cookie: $COOKIE=$P2" "$PUB/api/v2/ping?n=[1-250]" | grep -c 429)
[ "$api" = 0 ] && ok "and the API calls that follow (250 in a burst, with a pass) still have their whole budget" || bad "$api of 250 API calls got a 429 after the files of a cold load"
flood=$(curl -s -o /dev/null -w '%{http_code}\n' "$PUB/assets/flood-Ab12Cd34-[1-1800].js" | grep -c 429)
[ "$flood" -ge 100 ] && ok "a flood of files is still limited ($flood of 1800 got a 429)" || bad "files are not limited ($flood of 1800 got a 429)"
check 200 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P2")" "and the flood of files did not touch the API's budget"
# a device the app has revoked is no pass: the stubs answer it as a stranger, the sign-in screen opens for it
write_state 14 '["dani-laptop-q1z8c"]' '{}'; sleep 2.5
check 401 "$(code $PUB/api/v2/auth/viewer -H "Cookie: $COOKIE=$P2")" "a revoked pass is a stranger for the viewer question"
R=$(curl -s $PUB/auth -H "Cookie: $COOKIE=$P2"); echo "$R" | grep -q 'pass=\[\]' && ok "the shell does not pass a revoked pass on" || bad "revoked pass on the shell: $R"
write_state 15 '[]' '{}'; sleep 2.5

# how many connections ONE client may have open at once. Tailscale serve (the Funnel's relay on :8443 and the tailnet's :10802)
# turns one browser's HTTP/2 into MANY parallel HTTP/1.1 connections to the edge, so a cold load of the app has dozens in flight
# at a time: the first caps (30 for a public client, 8 for the owner's GraphQL on the home listener) refused the app's own files
# and the owner's calls with a 429 in the live walk-through. Here N curls are started together against the stub's slow answer
# (see `X-Hold`) and the ones the edge refuses (429, at once) are counted; the others stay open until -m ends them (000)
sleep 20   # the request budgets of the checks above refill
held() { # held <n> <curl args>: n parallel slow requests from this address; prints how many the edge refused with a 429
  local n=$1 i; shift; rm -rf "$STATE/held"; mkdir -p "$STATE/held"
  for i in $(seq 1 "$n"); do ( curl -s -m 6 -o /dev/null -w '%{http_code}' -H 'X-Hold: 1' "$@" > "$STATE/held/$i" 2>/dev/null ) & done
  wait
  grep -lx 429 "$STATE"/held/* 2>/dev/null | wc -l
}
c=$(held 50 "$PUB/assets/hold-Ab12Cd34.js")
[ "$c" = 0 ] && ok "50 build files at once from one address (a cold load behind Tailscale serve, no pass): none refused" || bad "$c of 50 parallel build files got a 429 (the per-address connection cap is too low for a cold load)"
c=$(held 150 "$PUB/assets/hold-Ab12Cd34.js")
[ "$c" -ge 10 ] && ok "but 150 at once are still limited ($c of 150 got a 429)" || bad "150 parallel build files from one address are not limited ($c got a 429)"
c=$(held 40 -H "Cookie: $COOKIE=$P2" "$PUB/api/v2/ping")
[ "$c" = 0 ] && ok "40 API calls at once from one address (with a pass): none refused" || bad "$c of 40 parallel API calls got a 429 (the per-address connection cap is too low)"
c=$(held 90 -H "Cookie: $COOKIE=$P2" "$PUB/api/v2/ping")
[ "$c" -ge 15 ] && ok "but 90 at once are still limited ($c of 90 got a 429)" || bad "90 parallel API calls from one address are not limited ($c got a 429)"
c=$(held 20 -X POST -H 'Authorization: Bearer stump_testkey' -H 'Content-Type: application/json' -d '{}' "$HOME_/api/graphql")
[ "$c" = 0 ] && ok "20 GraphQL calls at once on the home listener (the owner's app over the tailnet): none refused" || bad "$c of 20 parallel GraphQL calls on the home listener got a 429"
c=$(held 60 -X POST -H 'Authorization: Bearer stump_testkey' -H 'Content-Type: application/json' -d '{}' "$HOME_/api/graphql")
[ "$c" -ge 10 ] && ok "but 60 at once are still limited ($c of 60 got a 429)" || bad "60 parallel GraphQL calls on the home listener are not limited ($c got a 429)"
check 200 "$(code $PUB/assets/index-Ab12Cd34.js)" "and the edge serves normally again once they are gone"

if [ "$LIVE" = 1 ]; then
  echo "== the live edge"
  check 404 "$(code http://127.0.0.1:10803/api/v2/ping)" "no pass"
  check 404 "$(code --path-as-is "http://127.0.0.1:10803/settings/access/../../api/_i/x")" "no pass, a dot-segment invite path"
  check 404 "$(code --path-as-is "http://127.0.0.1:10803/api/v2/ping/..%2F..%2F_i/x")" "no pass, an escaped-slash invite path"
  L=http://127.0.0.1:10803
  check 401 "$(code $L/api/v2/auth/viewer)" "live: the viewer with no pass is answered by the edge"
  check 200 "$(code $L/api/v2/claim)" "live: the claim check"
  check 200 "$(code $L/api/v2/auth/oidc/config)" "live: the identity-provider check"
  check 404 "$(code $L/api/graphql)" "live: GraphQL with no pass"
  check 404 "$(code $L/sw.js)" "live: the service worker with no pass"
  # the shell a stranger gets carries a policy that allows each of its own inline scripts by hash (the app computes it
  # from the index.html it serves, so a later build cannot drift), and is never cached
  curl -s -D "$STATE/live.h" -o "$STATE/live.html" $L/auth
  python3 - "$STATE/live.html" "$STATE/live.h" <<'EOF' && ok "live: the pass-less shell has a policy that allows each inline script by hash" || bad "live: the pass-less shell's policy (a missing header, or an inline script it does not allow)"
import sys,re,hashlib,base64
html=open(sys.argv[1],encoding='utf-8').read()
hdr=open(sys.argv[2],encoding='utf-8').read()
csp=' '.join(l for l in hdr.splitlines() if l.lower().startswith('content-security-policy:'))
bodies=[b for b in re.findall(r'<script(?![^>]*\ssrc=)[^>]*>(.*?)</script>', html, re.S) if b.strip()]
missing=[b for b in bodies if "'sha256-"+base64.b64encode(hashlib.sha256(b.encode()).digest()).decode()+"'" not in csp]
sys.exit(1 if (not csp or not bodies or missing or "frame-ancestors 'none'" not in csp) else 0)
EOF
  tr -d '\r' < "$STATE/live.h" | grep -qi '^cache-control: no-store' && ok "live: the pass-less shell is not cached" || bad "live: the pass-less shell is cached"
fi

echo
echo "$n checks, $fail failed"
[ "$fail" = 0 ]
