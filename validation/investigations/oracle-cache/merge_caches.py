# SPDX-License-Identifier: Apache-2.0
"""Copy the entries of oracle caches into another (the workspace's), with the blobs they use.

    python -P merge_caches.py <into> <from> [<from> ...]

An entry already in ``<into>`` is kept. Left out: disputed entries (findings, kept where they
are), entries written before 2026-10-25's fixes whose outputs include a rewritten input stored
under its name (``in<n>`` roles since), and entries keyed with Intel's ``KMP_*`` variables (AFNI
runs keyed under pytest before they left the key: never found again). Nothing is deleted.
"""

from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path


def _stale(entry: dict) -> str:
    if entry.get("disputed"):
        return "disputed"
    if any(name.startswith("KMP_") for name in entry["parts"].get("env", {})):
        return "keyed with KMP_ variables"
    ats = {i.get("at") for i in entry.get("inputs", []) if i.get("at")}
    for o in entry["result"]["outputs"]:
        role = "{" + o["place"] + "}" + (f"/{o['path']}" if o["path"] else "")
        if role in ats:
            return "a rewritten input stored under its name"
    return ""


def merge(into: Path, sources: list[Path]) -> None:
    (into / "entries").mkdir(parents=True, exist_ok=True)
    fmt = into / "FORMAT"
    if not fmt.exists():
        shutil.copyfile(sources[0] / "FORMAT", fmt)
        if (sources[0] / "README.txt").exists():
            shutil.copyfile(sources[0] / "README.txt", into / "README.txt")
    counts: dict[str, int] = {}
    for src in sources:
        for path in sorted((src / "entries").glob("*/*.json")):
            entry = json.loads(path.read_text(encoding="utf-8"))
            why = _stale(entry)
            target = into / "entries" / path.parent.name / path.name
            if why:
                counts[why] = counts.get(why, 0) + 1
                continue
            if target.exists():
                counts["already there"] = counts.get("already there", 0) + 1
                continue
            r = entry["result"]
            shas = [r["stdout"], r["stderr"], *(o["sha256"] for o in r["outputs"])]
            for i in entry.get("inputs", []):
                shas += [i["sha256"], *(h for _s, h in i.get("companions", []))]
            for sha in shas:
                blob = into / "blobs" / sha[:2] / sha
                if not blob.exists():
                    blob.parent.mkdir(parents=True, exist_ok=True)
                    tmp = blob.with_name(f".{sha}.merge.tmp")
                    shutil.copyfile(src / "blobs" / sha[:2] / sha, tmp)
                    tmp.replace(blob)
            target.parent.mkdir(parents=True, exist_ok=True)
            tmp = target.with_name(f".{target.name}.merge.tmp")
            shutil.copyfile(path, tmp)
            tmp.replace(target)
            counts["copied"] = counts.get("copied", 0) + 1
    for why, n in sorted(counts.items()):
        print(f"{n:6d}  {why}")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    merge(Path(sys.argv[1]), [Path(a) for a in sys.argv[2:]])
