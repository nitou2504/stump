#!/bin/bash
# usage: [STUMP_TARGET_VOLUME=stump-check-target-<lane>] test.sh '<cargo test args>'  (runs on l5 from the repo root)
# Every lane builds in its own target volume, so two lanes never wait for each other's build lock
docker run --rm -v "$PWD":/app -w /app \
  -v stump-cargo-reg:/usr/local/cargo/registry -v "${STUMP_TARGET_VOLUME:-stump-check-target}":/app/target \
  rust:1.97-bookworm bash -c "apt-get -qq update >/dev/null && apt-get -qq install -y cmake libsqlite3-dev >/dev/null; cp Cargo.toml /tmp/C.orig; sed -i -e '/core\/integration-tests/d' -e '/apps\/desktop\/src-tauri/d' Cargo.toml; git config --global --add safe.directory /app; { git diff --name-only fdbb2df7; git ls-files -o --exclude-standard; } | grep '\.rs$' | sort -u | xargs -r touch; cargo test $1 2>&1 | grep -E -A12 '^test .*FAILED|^test result|panicked|error(\[|:)|^failures' | head -60; cp /tmp/C.orig Cargo.toml"
