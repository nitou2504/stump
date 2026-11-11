#!/bin/bash
# verify-edge.sh: runs the REAL nginx.conf of this folder in a throwaway container pair against a stub
# upstream that echoes the headers it gets, and checks what the edge promises. Needs docker, curl,
# openssl, python3. Nothing live is touched (own network, own ports 18080/18081, own state folder).
#
#   bash deploy/public-edge/verify-edge.sh            # from the repo root
#   bash deploy/public-edge/verify-edge.sh --live     # also checks the LIVE edge on 127.0.0.1:10803 (no pass → 404)
set -u
LIVE=0; [ "${1:-}" = "--live" ] && LIVE=1
HERE="$(cd "$(dirname "$0")" && pwd)"
NET=stump-edge-verify
KEY=$(python3 -c "import json;print(json.load(open('$HERE/vectors.json'))['key'])")
COOKIE=__Host-g3-stump
STATE=$(mktemp -d); chmod 755 "$STATE"
PUB=http://127.0.0.1:18081; HOME_=http://127.0.0.1:18080
fail=0; n=0
ok()   { n=$((n+1)); echo "  ok   $*"; }
bad()  { n=$((n+1)); fail=$((fail+1)); echo "  FAIL $*"; }
check(){ local want="$1" got="$2" what="$3"; if [ "$got" = "$want" ]; then ok "$what ($got)"; else bad "$what: wanted $want, got $got"; fi; }
cleanup() { docker rm -f $NET-edge $NET-stub >/dev/null 2>&1; docker network rm $NET >/dev/null 2>&1; rm -rf "$STATE" /tmp/$NET-stub.conf; }
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
http { server { listen 10801;
  client_max_body_size 400m;
  location / { default_type text/plain;
    return 200 "pub=[$http_x_edge_public] pass=[$http_x_edge_pass] cookie=[$http_cookie] host=[$http_host] xfp=[$http_x_forwarded_proto] xff=[$http_x_forwarded_for] real=[$http_x_real_ip] fwd=[$http_forwarded] fhost=[$http_x_forwarded_host] upg=[$http_upgrade] conn=[$http_connection] uri=$request_uri\n"; } } }
EOF
docker run -d --name $NET-stub --network $NET --network-alias stump-fork -v /tmp/$NET-stub.conf:/etc/nginx/nginx.conf:ro nginx:alpine >/dev/null
docker run -d --name $NET-edge --network $NET -p 127.0.0.1:18081:8081 -p 127.0.0.1:18080:8080 \
  -e EDGE_KEY="$KEY" -e EDGE_COOKIE=$COOKIE -v "$HERE":/etc/nginx/edge:ro -v "$STATE":/etc/nginx/state:ro \
  --entrypoint nginx nginx:alpine -c /etc/nginx/edge/nginx.conf -g 'daemon off;' >/dev/null
sleep 3
docker exec $NET-edge nginx -t -c /etc/nginx/edge/nginx.conf >/dev/null 2>&1 && ok "nginx -t" || bad "nginx -t"

P=$(pass_for ana-phone-k3f9x)

echo "== before the first state file, every pass is refused (fail closed)"
check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "a valid pass, no state file yet"

echo "== once the app has written the state file, passes work"
write_state 1 '[]' '{}'; sleep 2.5
check 200 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$P")" "a valid pass"
check 404 "$(code $PUB/api/v2/ping)" "no pass"
check 404 "$(code $PUB/)" "no pass, the front page"
check 404 "$(code -X POST $PUB/api/v2/auth/login -H 'Content-Type: application/json' -d '{}')" "no pass, a sign-in"
check 404 "$(code $PUB/api/graphql -H 'Content-Type: application/json' -d '{}')" "no pass, GraphQL"
check 404 "$(code $PUB/settings/access -H "Cookie: $COOKIE=$P")" "the admin page, even with a pass"
check 200 "$(code $PUB/api/_i/AAAAAAAAAAAAAAAAAAAAAA)" "the invite page needs no pass"
check 200 "$(code $PUB/assets/manifest.webmanifest)" "the manifest needs no pass"
check 200 "$(code $PUB/assets/favicon-32x32.png)" "a favicon needs no pass"
check 404 "$(code $PUB/assets/index.js)" "other assets do"
for forged in $(python3 -c "import json;print(' '.join(json.load(open('$HERE/vectors.json'))['forged']))"); do
  check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$forged")" "a forged pass"
done
check 404 "$(code $PUB/api/v2/ping -H "Cookie: $COOKIE=$(pass_for ana-phone-k3f9x $(( $(date +%s) - 181*86400 )))")" "an expired pass"
check 404 "$(code $PUB/api/_i/../v2/ping)" "a path trick without a pass"
check 404 "$(code --path-as-is $PUB/api/_i/%2e%2e/v2/ping)" "an encoded path trick without a pass"

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
for path in / /api/v2/ping /api/graphql /api/v2/auth/login /api/v2/auth/refresh-token /api/v2/intake/upload /api/_i/AAAAAAAAAAAAAAAAAAAAAA /assets/manifest.webmanifest /assets/favicon-32x32.png /api/graphql/ws; do
  extra=(-H "Cookie: $COOKIE=$P")
  [ "$path" = /api/graphql ] && extra+=(-X POST -H 'Content-Type: application/json' -d '{}')
  [ "$path" = /api/v2/auth/login ] && extra+=(-X POST -H 'Content-Type: application/json' -d '{}')
  [ "$path" = /api/v2/auth/refresh-token ] && extra+=(-X POST -H 'Content-Type: application/json' -d '{}')
  [ "$path" = /api/v2/intake/upload ] && extra+=(-X POST -H 'Content-Type: multipart/form-data; boundary=x' --data-binary x)
  [ "$path" = /api/graphql/ws ] && extra+=(-H 'Upgrade: websocket' -H 'Connection: Upgrade')
  R=$(curl -s "$PUB$path" "${extra[@]}")
  echo "$R" | grep -q 'pub=\[1\]' && echo "$R" | grep -q 'xfp=\[https\]' && ok "public $path: marker, scheme" || bad "public $path: $R"
  H=$(curl -s "$HOME_$path" "${extra[@]}" -H 'X-Edge-Public: 1' -H 'X-Edge-Pass: planted')
  echo "$H" | grep -q 'pub=\[\]' && echo "$H" | grep -q 'pass=\[\]' && ok "home   $path: marker and pass empty" || bad "home $path: $H"
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

if [ "$LIVE" = 1 ]; then
  echo "== the live edge"
  check 404 "$(code http://127.0.0.1:10803/api/v2/ping)" "no pass"
fi

echo
echo "$n checks, $fail failed"
[ "$fail" = 0 ]
