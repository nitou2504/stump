#!/bin/bash
# Regenerates crates/graphql/schema.graphql (runs on l5 from the repo root)
docker run --rm -v "$PWD":/app -w /app \
  -v stump-cargo-reg:/usr/local/cargo/registry -v stump-check-target:/app/target \
  rust:1.97-bookworm bash -c 'apt-get -qq update >/dev/null && apt-get -qq install -y cmake libsqlite3-dev >/dev/null; cp Cargo.toml /tmp/C.orig; sed -i -e "/core\/integration-tests/d" -e "/apps\/desktop\/src-tauri/d" Cargo.toml; cargo run -q -p graphql --bin graphql-gen 2>&1 | tail -5; cp /tmp/C.orig Cargo.toml; chown --reference=Cargo.toml crates/graphql/schema.graphql'
