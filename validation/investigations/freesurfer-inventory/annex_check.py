# SPDX-License-Identifier: Apache-2.0
"""Where FreeSurfer serves an annexed file on its own: the git-annex object's address on
FreeSurfer's public annex remote, checked with HEAD requests only (nothing is downloaded).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the tag's annex links).

    python -P validation/investigations/freesurfer-inventory/annex_check.py \\
        <workspace>/tags/freesurfer-8.2.0 --paths <tag path> [...] | --toml docs/freesurfer/data-files.toml \\
        --tsv <scratch>/annex-urls.tsv [--spacing 20]

The tag (GitHub's v8.2.0 archive) keeps each annexed file as a symbolic link whose name is its
git-annex key (``SHA256E-s<size>--<sha256><extension>``). FreeSurfer's remote
(``https://surfer.nmr.mgh.harvard.edu/pub/dist/freesurfer/repo/annex.git``, a bare repository,
annex version 8) stores the object at ``annex/objects/<md5(key)[0:3]>/<md5(key)[3:6]>/<key>/<key>``
(git-annex's lower-case hash directories for bare repositories). The server refuses clients that
ask quickly (measured 2026-11-12: about 100 requests in a minute brought HTTP 429 and then
refused connections for about an hour; later, requests 20 seconds apart got 429 again after nine),
so the requests go one at a time, ``--spacing`` seconds apart (60 by default), a 429 waits
``--backoff`` seconds (300) before asking again (twice at most), and the run stops at the first
refused connection; rows already answered are kept, so a later run continues.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import os
import subprocess
import sys
import time
import tomllib
from pathlib import Path

REMOTE = "https://surfer.nmr.mgh.harvard.edu/pub/dist/freesurfer/repo/annex.git/annex/objects/"
FIELDS = ["path", "key", "url", "http_status", "content_length"]


def object_url(key: str) -> str:
    h = hashlib.md5(key.encode()).hexdigest()  # git-annex's hash directories, not a checksum
    return f"{REMOTE}{h[:3]}/{h[3:6]}/{key}/{key}"


def head(url: str) -> tuple[str, str]:
    out = subprocess.run(
        ["curl", "-sS", "-m", "30", "-I", "-o", "/dev/null", "-D", "-", url],
        capture_output=True,
        text=True,
    ).stdout
    status = out.split(" ", 2)[1] if out.startswith("HTTP") else "000"
    length = next(
        (
            h.split(":", 1)[1].strip()
            for h in out.splitlines()
            if h.lower().startswith("content-length:")
        ),
        "",
    )
    return status, length


def write(tsv: Path, rows: dict[str, dict[str, str]]) -> None:
    with tsv.open("w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, FIELDS, delimiter="\t", lineterminator="\n")
        w.writeheader()
        for r in rows.values():
            w.writerow({k: r.get(k, "") for k in FIELDS})


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("tag", type=Path)
    ap.add_argument("--paths", nargs="*", default=[])
    ap.add_argument("--toml", type=Path, help="data-files.toml: check every annexed tag_path in it")
    ap.add_argument("--tsv", type=Path, required=True)
    ap.add_argument("--spacing", type=float, default=60.0)
    ap.add_argument("--backoff", type=float, default=300.0, help="seconds to wait after a 429")
    ap.add_argument("--again", action="store_true", help="re-check rows that already answered 200")
    args = ap.parse_args(argv)
    paths = list(args.paths)
    if args.toml:
        table = tomllib.loads(args.toml.read_text(encoding="utf-8"))
        paths += [f["tag_path"] for f in table["file"] if f["tag_path"]]
    paths = sorted({p for p in paths if (args.tag / p).is_symlink()})
    rows: dict[str, dict[str, str]] = {}
    if args.tsv.exists():
        with args.tsv.open(encoding="utf-8") as f:
            rows = {r["path"]: r for r in csv.DictReader(f, delimiter="\t")}
    for p in paths:
        r = rows.get(p)
        if r and r["http_status"] == "200" and not args.again:
            continue
        key = os.path.basename(os.readlink(args.tag / p))
        status, length = head(object_url(key))
        for _ in range(2):  # HTTP 429: the server's rate limit; wait it out, then ask again
            if status != "429":
                break
            time.sleep(args.backoff)
            status, length = head(object_url(key))
        rows[p] = {
            "path": p,
            "key": key,
            "url": object_url(key),
            "http_status": status,
            "content_length": length,
        }
        print(status, length, p, flush=True)
        write(args.tsv, rows)  # after every answer, so a stopped run keeps what it learnt
        if status == "000":
            print("connection refused: stopping (run again later)", flush=True)
            break
        time.sleep(args.spacing)
    write(args.tsv, rows)
    print("ANNEX-CHECK DONE", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
