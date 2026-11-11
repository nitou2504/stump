#!/usr/bin/env python3
"""Patch /mnt/data2/containers/stump-fork/docker-compose.yml for "Public access" (idempotent; backup first).

usage: apply-compose.py <docker-compose.yml> [--dry-run] [--require-pass]
  default        stump-fork gains STUMP_EDGE_KEY / _PUBLIC_URL / _STATE_DIR / _REQUIRE_PASS=false and the
                 /edge-state volume; stump-edge gains the read-only state mount (its ../edge-common mount stays, so the
                 old edge can still restart until DEPLOY.md step 7 switches it)
  --require-pass flips STUMP_EDGE_REQUIRE_PASS from false to true (DEPLOY.md, step 8)
Prints a summary only (the file has no secrets; .env is never read).
"""
import shutil
import sys
import time
from pathlib import Path

target = Path(sys.argv[1])
dry = "--dry-run" in sys.argv
text = target.read_text()

if "--require-pass" in sys.argv:
    if "STUMP_EDGE_REQUIRE_PASS=true" in text:
        sys.exit("already strict (STUMP_EDGE_REQUIRE_PASS=true): nothing to do")
    if text.count("STUMP_EDGE_REQUIRE_PASS=false") != 1:
        sys.exit("STUMP_EDGE_REQUIRE_PASS=false not found: apply the default patch first")
    text = text.replace("STUMP_EDGE_REQUIRE_PASS=false", "STUMP_EDGE_REQUIRE_PASS=true", 1)
else:
    if "STUMP_EDGE_STATE_DIR" in text:
        sys.exit("already patched (STUMP_EDGE_STATE_DIR present): nothing to do")

    fork_old = """      - STUMP_CONFIG_DIR=/config
    restart: unless-stopped
"""
    fork_new = """      - STUMP_CONFIG_DIR=/config
      # Public access (Settings -> Public access): the key the passes are signed with (the app mints
      # passes and checks the pass the edge reports), the address invite links are made for, the
      # folder the app writes the edge's state file in, and whether a request through the public
      # edge must carry a pass ("false" until the new edge is live, then "true": DEPLOY.md, step 8)
      - STUMP_EDGE_KEY=${STUMP_EDGE_KEY:?set STUMP_EDGE_KEY in ./.env}
      - STUMP_EDGE_PUBLIC_URL=https://g3.tail3e401.ts.net:8443
      - STUMP_EDGE_STATE_DIR=/edge-state
      - STUMP_EDGE_REQUIRE_PASS=false
    restart: unless-stopped
"""
    if text.count(fork_old) != 1:
        sys.exit("the stump-fork environment is not what this script expects; patch by hand (see compose-snippet.yaml)")
    text = text.replace(fork_old, fork_new, 1)

    vol_old = """      - ./config/:/config
"""
    vol_new = """      - ./config/:/config
      # where the app writes the file the public edge follows (revoked devices, budget resets)
      - ./edge-state:/edge-state
"""
    if text.count(vol_old) != 1:
        sys.exit("the stump-fork volumes are not what this script expects; patch by hand (see compose-snippet.yaml)")
    text = text.replace(vol_old, vol_new, 1)

    edge_old = """      - ../edge-common:/etc/nginx/njs:ro
"""
    edge_new = """      - ../edge-common:/etc/nginx/njs:ro
      # the app's state file (revoked devices, budget resets): read-only here; stump-gate.js (in the
      # edge folder) follows it once a second. The ../edge-common mount above STAYS on purpose: the
      # live nginx.conf imports gate.js from it until DEPLOY.md step 7 switches the edge, so a recreate
      # of stump-edge before then (an unscoped `up -d`, a --force-recreate) still starts. Nothing
      # loads it after the switch; it is harmless and can go the next time the edge is recreated
      - ./edge-state:/etc/nginx/state:ro
"""
    if text.count(edge_old) != 1:
        sys.exit("the stump-edge volumes are not what this script expects; patch by hand (see compose-snippet.yaml)")
    text = text.replace(edge_old, edge_new, 1)

if dry:
    print(text)
    sys.exit(0)
backup = target.with_name(target.name + ".bak-" + time.strftime("%Y%m%d-%H%M%S"))
shutil.copy2(target, backup)
target.write_text(text)
print(f"patched {target} (backup: {backup})")
