#!/usr/bin/env python3
"""Lists the KWin releases to build the background input plugin for, with
an Arch Linux Archive snapshot day on which each was Arch's current KWin.

Usage: kwin-plugin-matrix.py [--index index.json] [--since 6.0.0] [--limit N]
Prints a GitHub Actions matrix: {"include": [{"kwin": "6.8.3", "day": "2026/11/20"}]}.
Versions already in --index (a published index.json) are skipped.
"""

import argparse
import datetime
import json
import re
import sys
import urllib.request

LISTING = "https://archive.archlinux.org/packages/k/kwin/"
ROW = re.compile(r'href="kwin-([0-9][0-9.]*)-([0-9.]+)-x86_64\.pkg\.tar\.zst">[^<]*</a>\s+(\d{2}-\w{3}-\d{4})')


def abi(version):
    """The part a plugin is tied to: 6.3.2.1 is 6.3.2."""
    return ".".join(version.split(".")[:3])


def key(version):
    return tuple(int(p) for p in version.split("."))


def releases(html):
    """(abi version, first day seen) for stable releases, oldest first."""
    first = {}
    for version, _rel, day in ROW.findall(html):
        parts = key(version)
        # 6.x.80 and up are betas, only ever in [kde-unstable] or testing.
        if len(parts) < 3 or parts[2] >= 80:
            continue
        seen = datetime.datetime.strptime(day, "%d-%b-%Y").date()
        name = abi(version)
        if name not in first or seen < first[name]:
            first[name] = seen
    return sorted(first.items(), key=lambda item: key(item[0]))


def snapshot_days(found, today):
    """The last day each release was current: the day before the next one
    was built, or yesterday for the newest."""
    out = []
    for i, (name, seen) in enumerate(found):
        if i + 1 < len(found):
            day = found[i + 1][1] - datetime.timedelta(days=1)
        else:
            day = today - datetime.timedelta(days=1)
        if day < seen + datetime.timedelta(days=1):
            day = seen + datetime.timedelta(days=1)
        out.append({"kwin": name, "day": day.strftime("%Y/%m/%d")})
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--index")
    parser.add_argument("--since", default="6.0.0")
    parser.add_argument("--limit", type=int, default=200)
    parser.add_argument("--listing", help="a saved copy of the archive listing, for tests")
    args = parser.parse_args()

    if args.listing:
        html = open(args.listing, encoding="utf-8").read()
    else:
        with urllib.request.urlopen(LISTING, timeout=60) as response:
            html = response.read().decode("utf-8", "replace")
    done = set()
    if args.index:
        try:
            done = set(json.load(open(args.index, encoding="utf-8")).get("builds", {}))
        except (OSError, ValueError):
            pass
    found = [r for r in releases(html) if key(r[0]) >= key(args.since)]
    rows = [r for r in snapshot_days(found, datetime.date.today()) if r["kwin"] not in done]
    # Newest first, so a limit keeps the versions most people run.
    rows = sorted(rows, key=lambda r: key(r["kwin"]), reverse=True)[: args.limit]
    json.dump({"include": rows}, sys.stdout)
    print()


if __name__ == "__main__":
    main()
