#!/bin/bash
# Faster image build for l5 when its network is busy: compiles the server and the web
# client in containers that reuse the already-downloaded crates and node_modules, then
# assembles the runtime image with docker/Dockerfile.fork. Usage: build-local.sh <tag>
set -e
TAG=${1:-stump:fork}
REV=$(git rev-parse --short HEAD)
UIDGID="$(id -u):$(id -g)"
docker run --rm -v "$PWD":/app -w /app \
  -v stump-cargo-reg:/usr/local/cargo/registry -v stump-release-target:/app/target \
  -e GIT_REV="$REV" -e BUILD_CHANNEL=fork \
  rust:1.97-bookworm bash -c "apt-get -qq update >/dev/null && apt-get -qq install -y cmake libsqlite3-dev >/dev/null; cp Cargo.toml /tmp/C.orig; sed -i -e '/core\/integration-tests/d' -e '/apps\/desktop\/src-tauri/d' Cargo.toml; cargo build -q --package stump_server --bin stump_server --release 2>&1 | grep -E '^(error|warning: unused)' | head; mkdir -p dist-fork && cp target/release/stump_server dist-fork/stump_server; cp /tmp/C.orig Cargo.toml; chown -R $UIDGID dist-fork"
bash scripts/fork/node.sh 'yarn web build >/tmp/web.log 2>&1 || tail -30 /tmp/web.log; rm -rf dist-fork/client; mkdir -p dist-fork && cp -r apps/web/dist dist-fork/client'
ls -la dist-fork dist-fork/client | head
docker build -f docker/Dockerfile.fork -t "$TAG" --build-arg GIT_REV="$REV" . 2>&1 | tail -5
