#!/bin/bash
# usage: node.sh '<command>'  — runs a command in node:24 at the repo root on l5
docker run --rm -v "$PWD":/app -w /app -v stump-yarn-cache:/usr/local/share/.cache/yarn \
  -e HOME=/tmp node:24-bookworm bash -c "git config --global --add safe.directory /app >/dev/null 2>&1; $1; find /app -xdev -user 0 -exec chown $(id -u):$(id -g) {} + 2>/dev/null; true"
