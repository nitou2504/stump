#!/bin/bash
# Builds the fork image on l5: scripts/fork/build-image.sh <tag>
set -e
TAG=${1:-stump:fork}
# make every file changed on the fork newer than any cached build artifact
git config --global --add safe.directory "$PWD" 2>/dev/null || true
{ git diff --name-only fdbb2df7; git ls-files -o --exclude-standard; } | sort -u | while read -r f; do [ -f "$f" ] && touch "$f"; done
docker buildx build --load -f docker/Dockerfile -t "$TAG" \
  --build-arg GIT_REV="$(git rev-parse --short HEAD)" --build-arg BUILD_CHANNEL=fork . 2>&1 | grep -vE "^#[0-9]+ (sha256|extracting|\[internal\])" | tail -60
