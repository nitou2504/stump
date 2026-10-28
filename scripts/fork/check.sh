#!/bin/bash
# usage: check.sh  (runs from repo root on l5)
docker run --rm -v "$PWD":/app -w /app \
  -v stump-cargo-reg:/usr/local/cargo/registry -v stump-check-target:/app/target \
  rust:1.97-bookworm bash -c 'apt-get -qq update >/dev/null && apt-get -qq install -y cmake libsqlite3-dev >/dev/null; sed -e "/core\/integration-tests/d" -e "/apps\/desktop\/src-tauri/d" Cargo.toml > /tmp/C && cp Cargo.toml /tmp/C.orig && cp /tmp/C Cargo.toml; git config --global --add safe.directory /app; { git diff --name-only HEAD; git diff --name-only fdbb2df7; git ls-files -o --exclude-standard; } | grep "\.rs$" | sort -u | xargs -r touch; cargo check -p stump_server --message-format short 2>&1 | grep -E "^(error|warning)|\.rs:[0-9]+:[0-9]+: (error|warning)|Finished|could not compile" | head -80; cp /tmp/C.orig Cargo.toml'
