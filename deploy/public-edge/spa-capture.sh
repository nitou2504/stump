#!/bin/bash
# spa-capture.sh <stump image>: the web app's own sign-in screen on the PUBLIC address, in a real browser.
# Stands up a throwaway copy of the stack (the image, the REAL nginx.conf of this folder as its edge, a scratch config with
# no library, its own docker network and ports 18380/18381; nothing live is touched), then runs the Playwright spec
# apps/web/tests/public/public-login.spec.ts against it:
#   * every request a stranger's page makes is one the edge lets through, nothing goes to another site, and the page
#     raises no Content-Security-Policy violation;
#   * the short login screen, an invite link -> the join screen (the token leaves the address and storage), a sign-in on
#     another device (a pass is made), the fourth device refused in words, the owner never signs in here.
# Run it BEFORE the edge goes live (DEPLOY.md, "Public login", step 2). Needs docker, python3, and `yarn install`
# done in the repo (the spec runs with the repo's own Playwright, in Microsoft's browser image).
#
#   bash deploy/public-edge/spa-capture.sh stump:fork-<rev>          # from the repo root
# Set CAPTURE_KEEP=1 to leave the containers up afterwards (CAPTURE_NET, CAPTURE_PUB_PORT, CAPTURE_HOME_PORT to run beside another copy).
set -u
IMAGE=${1:?usage: spa-capture.sh <stump image>}
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
NET=${CAPTURE_NET:-stump-spa-capture}
PUB_PORT=${CAPTURE_PUB_PORT:-18381}; HOME_PORT=${CAPTURE_HOME_PORT:-18380}
PLAYWRIGHT_IMAGE=${CAPTURE_PLAYWRIGHT_IMAGE:-mcr.microsoft.com/playwright:v1.62.1-noble}
KEY=$(python3 -c "import json;print(json.load(open('$HERE/vectors.json'))['key'])")
W=$(mktemp -d); chmod 755 "$W"; mkdir -p "$W/config" "$W/state" "$W/edge"
cp -a "$HERE"/. "$W/edge"/
printf 'trust_proxy_headers = true\n' > "$W/config/Stump.toml"   # the per-client limits need the address the edge forwards

cleanup() {
  [ "${CAPTURE_KEEP:-0}" = 1 ] && { echo "(CAPTURE_KEEP=1: containers $NET-app and $NET-edge left running; scratch in $W)"; return; }
  docker rm -f $NET-app $NET-edge >/dev/null 2>&1; docker network rm $NET >/dev/null 2>&1
  docker run --rm -v "$W":/w alpine rm -rf /w/config /w/state >/dev/null 2>&1; rm -rf "$W"
}
trap cleanup EXIT

docker image inspect "$IMAGE" >/dev/null 2>&1 || { echo "FAIL: no image $IMAGE"; exit 2; }
docker network create $NET >/dev/null 2>&1
echo "== starting the app and the edge ($IMAGE)"
docker run -d --name $NET-app --network $NET --network-alias stump-fork \
  -e PUID="$(id -u)" -e PGID="$(id -g)" -e STUMP_CONFIG_DIR=/config -e STUMP_EDGE_KEY="$KEY" \
  -e STUMP_EDGE_PUBLIC_URL="http://localhost:$PUB_PORT" -e STUMP_EDGE_STATE_DIR=/edge-state \
  -e STUMP_EDGE_REQUIRE_PASS=true -e STUMP_EDGE_PUBLIC_SIGNIN=true -e STUMP_TRUST_PROXY_HEADERS=true \
  -v "$W/config":/config -v "$W/state":/edge-state "$IMAGE" >/dev/null
docker run -d --name $NET-edge --network $NET -p 127.0.0.1:$PUB_PORT:8081 -p 127.0.0.1:$HOME_PORT:8080 \
  -e EDGE_KEY="$KEY" -e EDGE_COOKIE=__Host-g3-stump \
  -v "$W/edge":/etc/nginx/edge:ro -v "$W/state":/etc/nginx/state:ro \
  --entrypoint nginx nginx:alpine -c /etc/nginx/edge/nginx.conf -g 'daemon off;' >/dev/null

up=0
for i in $(seq 1 60); do
  [ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$HOME_PORT/api/v2/ping)" = 200 ] && { up=1; break; }
  sleep 2
done
if [ "$up" != 1 ]; then echo "FAIL: the app did not come up"; docker logs --tail 30 $NET-app 2>&1 | cut -c1-200; docker logs --tail 10 $NET-edge 2>&1 | cut -c1-200; exit 1; fi
sleep 3   # the edge reads the app's state file once a second

echo "== the browser run"
docker run --rm --network host -v "$REPO":/app -w /app/apps/web -e HOME=/tmp \
  -e STUMP_PUBLIC_URL="http://localhost:$PUB_PORT" -e STUMP_HOME_URL="http://127.0.0.1:$HOME_PORT" \
  "$PLAYWRIGHT_IMAGE" bash -c "npx playwright test -c playwright.public.config.ts --output=/tmp/pw-out; rc=\$?; chown -R $(id -u):$(id -g) /app/apps/web 2>/dev/null; exit \$rc"
rc=$?
if [ "$rc" != 0 ]; then
  echo "== it failed: the last of the app's log"
  docker logs --tail 25 $NET-app 2>&1 | cut -c1-200
fi
exit $rc
