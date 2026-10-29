#!/usr/bin/env python3
"""Map a minified browser stack trace (e.g. from the error screen's "Copy" button) back to
source using the hidden source maps shipped in the image.

Usage:
  docker cp stump-fork:/app/client/assets /tmp/stump-assets   # once per deployed build
  python3 scripts/fork/map_stack.py /tmp/stump-assets < stack.txt
"""
import json
import re
import sys
from bisect import bisect_right
from pathlib import Path

B64 = {c: i for i, c in enumerate("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/")}


def vlq_values(segment):
    values, shift, value = [], 0, 0
    for char in segment:
        digit = B64[char]
        value += (digit & 31) << shift
        if digit & 32:
            shift += 5
        else:
            values.append(-(value >> 1) if value & 1 else value >> 1)
            shift = value = 0
    return values


def parse_map(path):
    data = json.loads(path.read_text())
    lines = []
    source = src_line = src_col = name = 0
    for line in data["mappings"].split(";"):
        col = 0
        entries = []
        for segment in filter(None, line.split(",")):
            v = vlq_values(segment)
            col += v[0]
            if len(v) >= 4:
                source += v[1]
                src_line += v[2]
                src_col += v[3]
                if len(v) == 5:
                    name += v[4]
                    entries.append((col, source, src_line, src_col, name))
                else:
                    entries.append((col, source, src_line, src_col, None))
        lines.append(entries)
    return data, lines


def lookup(cache, assets, file, line, col):
    if file not in cache:
        map_path = assets / f"{file}.map"
        cache[file] = parse_map(map_path) if map_path.exists() else None
    parsed = cache[file]
    if not parsed:
        return None
    data, lines = parsed
    entries = lines[line - 1] if 0 < line <= len(lines) else []
    index = bisect_right([e[0] for e in entries], col - 1) - 1
    if index < 0:
        return None
    _, source, src_line, src_col, name = entries[index]
    names = data.get("names", [])
    return (
        data["sources"][source].split("/src/")[-1],
        src_line + 1,
        src_col + 1,
        names[name] if name is not None and name < len(names) else None,
    )


def main():
    assets = Path(sys.argv[1])
    cache = {}
    for raw in sys.stdin:
        line = raw.rstrip("\n")
        m = re.search(r"assets/([^/:\s]+\.js):(\d+):(\d+)", line)
        if not m:
            print(line)
            continue
        hit = lookup(cache, assets, m.group(1), int(m.group(2)), int(m.group(3)))
        if hit:
            src, ln, cl, name = hit
            print(f"{line}\n      -> {src}:{ln}:{cl}{f' ({name})' if name else ''}")
        else:
            print(line)


if __name__ == "__main__":
    main()
