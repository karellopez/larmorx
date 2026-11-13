# SPDX-License-Identifier: Apache-2.0
"""What of FreeSurfer's own text the recordings hold, for the clean-room audit's restriction table.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's files).

    python -P validation/investigations/freesurfer-inventory/recording_source.py \\
        <workspace>/oracles/freesurfer-8.2.0 \\
        --manifest <workspace>/oracles/inputs/freesurfer-replay/manifest-<tree>.json

A recording (``runs/<run>/record/``, and ``failed/<run>/record/`` for a stopped one) keeps every
file a step saw once, by SHA-256, in ``blobs/<2 hex>/<sha256>``. recon-all copies FreeSurfer's own
configuration into the subject (``scripts/recon-config.yaml``; ``scripts/recon-all.local-copy`` is
never recorded), so some blobs are FreeSurfer's text under a name that says nothing. This prints:

1. every blob recorded under ``scripts/recon-config.yaml`` or ``scripts/recon-all.local-copy`` (by
   the steps' and the state's file maps), with the runs that hold it, and the line for the audit;
2. every blob whose content is a file of the installed package that the audit treats as source (its
   ``bin/``, ``python/``, ``lib/``, ``etc/``, ... and top-level scripts: the replay tool's manifest of
   the installation gives each file's SHA-256), so a copy under another name is seen too.

Run names are printed, never the machine's paths.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import defaultdict
from pathlib import Path

NAMED = re.compile(r"(^|/)scripts/(recon-config\.yaml|recon-all\.local-copy)$")
SOURCE = re.compile(r"^(bin|python|lib|etc|fsfast|matlab|tktools|fsafd)/|^[^/]+\.(c?sh|py)$")
SHA = re.compile(r"[0-9a-f]{64}")


def recordings(root: Path):
    for top in ("runs", "failed"):
        d = root / top
        if d.is_dir():
            for run in sorted(d.iterdir()):
                if (run / "record").is_dir():
                    yield run


def named_blobs(obj, out: set[str]) -> None:
    """The SHA-256s a recording's JSON maps to the two FreeSurfer paths (any nesting)."""
    if isinstance(obj, dict):
        for k, v in obj.items():
            if isinstance(k, str) and NAMED.search(k):
                if isinstance(v, str) and SHA.fullmatch(v):
                    out.add(v)
                elif isinstance(v, dict):
                    out.update(x for x in v.values() if isinstance(x, str) and SHA.fullmatch(x))
                elif isinstance(v, list):
                    out.update(x for x in v if isinstance(x, str) and SHA.fullmatch(x))
            named_blobs(v, out)
    elif isinstance(obj, list):
        for x in obj:
            named_blobs(x, out)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("root", type=Path, help="oracles/freesurfer-<version>")
    ap.add_argument("--manifest", type=Path, help="the replay tool's manifest of the installation")
    a = ap.parse_args()

    named: dict[str, set[str]] = defaultdict(set)
    present: dict[str, set[str]] = defaultdict(set)
    nruns = nblobs = 0
    for run in recordings(a.root):
        nruns += 1
        rec = run / "record"
        for f in [*sorted((rec / "steps").glob("*.json")), rec / "state.json"]:
            if f.is_file():
                shas: set[str] = set()
                named_blobs(json.loads(f.read_text()), shas)
                for s in shas:
                    named[s].add(run.name)
        for b in (rec / "blobs").glob("*/*"):
            nblobs += 1
            present[b.name].add(run.name)
    print(f"recordings: {nruns}; blobs: {nblobs} files, {len(present)} distinct")

    print("\n1. blobs recorded as scripts/recon-config.yaml or scripts/recon-all.local-copy:")
    for s in sorted(named):
        held = sorted(present.get(s, ()))
        print(
            f"   {s}  named in {len(named[s])} runs, stored in {len(held)}: {', '.join(held[:3])}"
        )
    print("   for restrictions.py: FREESURFER_CONFIG_BLOBS = frozenset({")
    for s in sorted(named):
        print(f'       "{s}",')
    print("   })")

    if a.manifest:
        man = json.loads(a.manifest.read_text())
        src = defaultdict(list)
        for path, entry in man.items():
            if SOURCE.search(path):
                src[entry[2]].append(path)
        print(
            f"\n2. blobs equal to one of the package's {sum(map(len, src.values()))} source files:"
        )
        hits = [(s, src[s]) for s in sorted(present) if s in src]
        for s, paths in hits:
            size = man[paths[0]][0]
            print(f"   {s[:16]}  {size} bytes, in {len(present[s])} runs: {paths[:3]}")
        if not hits:
            print("   none")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
