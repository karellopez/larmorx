# SPDX-License-Identifier: Apache-2.0
"""Whether the site's examples that the glibc maths part did not compute again still show the
clean-room originals' current results: each original run of those figures-cache entries made
again from its recorded command, by this checkout's build, into a scratch directory, and its
outputs compared with the entry's, byte for byte after decompression.

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P evidence_unchanged.py --out <scratch dir> [--threads 14]

The entries (3dTshift's two examples, the head-motion correction's two, BET's three) hold the
original's runs of calls the part's wiring does not reach (3dTshift's Fourier interpolation,
mcflirt's normalised correlation, bet2's ``-f``/``-R`` runs on images where the record shows no
change): a run whose outputs now differ means the entry must be computed again before an
export. Paths inside the entry's directory are rewritten: an input given with ``@`` (a slice
timing file) is copied to the scratch directory, every other one is an output. The recorded
``PYTHONPATH`` and ``HOME`` are replaced (this checkout's package, a scratch home).
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import shutil
import subprocess
from pathlib import Path

#: entry -> the original's labels in it.
TARGETS = {
    "afni-tshift/fmriprep-ds005040-clip-1": ["original", "original-1", "original-reproduce",
                                             "neg-original", "neg-original-reproduce"],
    "afni-tshift/ds005040-sub-002-clip-1": ["larmorx-original"],
    "mri-hmc/fmriprep-ds000005-run-01": ["larmorx", "larmorx-1", "larmorx-fix"],
    "mri-hmc/ds000005-sub-01-run-1": ["reproduce", "fix"],
    "fsl-bet/fmriprep-ds000005-run-01-original": ["orig", "orig-reproduce", "t1-orig",
                                                  "t1-orig-reproduce"],
    "fsl-bet/panel-ds000228-pixar008-original": ["orig", "orig-reproduce"],
    "fsl-bet/betsurf-synthetic-head-original": ["orig", "orig-reproduce"],
}  # fmt: skip


def _cache() -> Path:
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles" / "figures-cache").is_dir():
            return parent / "oracles" / "figures-cache"
    raise SystemExit("the workspace's oracles/figures-cache was not found")


def _content(p: Path) -> bytes:
    raw = p.read_bytes()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def _one(cache: Path, entry: str, label: str, out: Path, threads: int, pythonpath: str) -> dict:
    edir = cache / entry
    info = json.loads((edir / "RUN_INFO.json").read_text(encoding="utf-8"))
    run = next(r for r in info["runs"] if r["label"] == label)
    work = out / entry.replace("/", "__") / label
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    argv, outputs, missing = [], [], []
    for a in run["argv"]:
        at, path = (a.startswith("@"), a.lstrip("@"))
        if path.startswith(str(edir) + "/"):
            rel = Path(path).relative_to(edir)
            new = work / rel
            if at:
                new.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy(edir / rel, new)
            else:
                outputs.append(rel)
            argv.append(("@" if at else "") + str(new))
        else:
            if path.startswith("/") and not Path(path).exists() and "python" not in path:
                missing.append(path)
            argv.append(a)
    if missing:
        return {"entry": entry, "label": label, "status": "skipped", "missing": missing[:3]}
    home = out / "home"
    home.mkdir(exist_ok=True)
    env = {k: v for k, v in os.environ.items()
           if not k.upper().startswith(("FSL", "LARMORX_", "OMP_", "AFNI"))}  # fmt: skip
    env.update(run["env"])
    env.update({"PYTHONPATH": pythonpath, "HOME": str(home)})
    env.setdefault("OMP_NUM_THREADS", str(threads))
    p = subprocess.run(argv, env=env, cwd=work, capture_output=True, text=True, timeout=3600)
    made = sorted(q.relative_to(work) for q in work.rglob("*") if q.is_file())
    differ, absent = [], []
    for rel in made:
        if (work / rel).suffix == ".1D" and rel.name == "slice_timing.1D":
            continue
        ref = edir / rel
        if not ref.is_file():
            absent.append(str(rel))
        elif _content(ref) != _content(work / rel):
            differ.append(str(rel))
    return {"entry": entry, "label": label, "exit": p.returncode, "recorded_exit": run["exit"],
            "files": len(made), "differ": differ, "not_in_entry": absent,
            "status": "same" if p.returncode == run["exit"] and not differ and made else "CHANGED",
            "stderr_tail": p.stderr[-300:] if p.returncode else ""}  # fmt: skip


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--threads", type=int, default=14)
    a = ap.parse_args()
    here = Path(__file__).resolve().parents[3]
    pythonpath = f"{here / 'python'}:{here / 'validation' / 'src'}"
    cache = _cache()
    results = []
    for entry, labels in TARGETS.items():
        for label in labels:
            r = _one(cache, entry, label, a.out, a.threads, pythonpath)
            results.append(r)
            print(json.dumps(r), flush=True)
    (a.out / "evidence-unchanged.json").write_text(json.dumps(results, indent=1) + "\n",
                                                   encoding="utf-8")  # fmt: skip
    n = {s: sum(r["status"] == s for r in results) for s in ("same", "CHANGED", "skipped")}
    print(f"EVIDENCE-UNCHANGED same {n['same']}, changed {n['CHANGED']}, skipped {n['skipped']}")
    return 1 if n["CHANGED"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
