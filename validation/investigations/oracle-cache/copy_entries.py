# SPDX-License-Identifier: Apache-2.0
"""Copies chosen entries of an oracle cache, with their blobs, into another cache: to run
``oracle-cache verify`` (or anything else that writes history or disputes) on them without
touching the workspace's cache.

Usage::

    python -P copy_entries.py <into> KEY... [--from <cache>]

``KEY`` is a key prefix, or ``@file`` with one per line (the first word of each line that is a key: the
log of a verify run, ``grep DIFFERS``, works). Blobs are hard-linked where the two caches share a file
system (they are written once and never changed), else copied. The entries are copied as they are
(their disputes too), and the copy is a cache of its own: ``LARMORX_ORACLE_CACHE=<into>`` or
``oracle-cache --cache <into>``.

Used on 2026-10-28 for the 89 entries the move's verify found different
(``docs/findings/machine-move-proof.md``, "Fixed").
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import sys
from pathlib import Path

from larmorx_validation.oracle_cache.store import Store


def blobs(entry: dict) -> list[str]:
    r = entry["result"]
    shas = [r["stdout"], r["stderr"], *(o["sha256"] for o in r["outputs"])]
    for i in entry.get("inputs", []):
        shas.append(i["sha256"])
        shas += [h for _s, h in i.get("companions", [])]
    return shas


def copy(src: Store, dst: Store, entry: dict) -> None:
    for sha in blobs(entry):
        a, b = src.blob_path(sha), dst.blob_path(sha)
        if b.exists() or not a.exists():
            continue
        b.parent.mkdir(parents=True, exist_ok=True)
        try:
            os.link(a, b)
        except FileExistsError:
            continue
        except OSError:
            shutil.copy2(a, b)
    dst.write(entry)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("into", type=Path)
    ap.add_argument("keys", nargs="+")
    ap.add_argument("--from", dest="source", type=Path, default=None)
    args = ap.parse_args(argv)
    prefixes: list[str] = []
    for k in args.keys:
        if k.startswith("@"):
            for ln in Path(k[1:]).read_text(encoding="utf-8").splitlines():
                words = [w for w in ln.split() if re.fullmatch(r"[0-9a-f]{8,64}", w)]
                prefixes += words[:1]
        else:
            prefixes.append(k)
    src, dst = Store(args.source), Store(args.into)
    dst.ensure()
    n = 0
    for e in src.entries():
        if any(e["key"].startswith(p) for p in prefixes):
            copy(src, dst, e)
            n += 1
    print(f"{n} entries of {len(prefixes)} named copied from {src.root} to {dst.root}")
    return 0 if n == len(prefixes) else 1


if __name__ == "__main__":
    sys.exit(main())
