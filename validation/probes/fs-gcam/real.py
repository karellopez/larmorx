# SPDX-License-Identifier: Apache-2.0
"""MRIStransform on the recorded heads' surfaces and transforms, FreeSurfer 8.2.0 against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions).

No recorded step of recon-all reaches MRIStransform (its mri_normalize calls pass
``identity.nofile``, which skips it; its mris_sphere calls give no transform), so the heads serve
as real inputs: for every head under ``<workspace>/oracles/freesurfer-8.2.0/runs/`` the white
surfaces through ``mri_normalize``'s call (``msf ... nn``: the classic stream's
``talairach.m3z``, ``talairach.xfm``, ``cc_up.lta``) and ``mris_sphere``'s (``msf ... ss``:
``talairach.lta``, voxel to voxel, and ``rawavg2orig.lta``, RAS to RAS, with ``orig.mgz``). Runs
``fsgcam`` through the oracle-output cache at one thread and the port's example in both modes,
compares the lines (the steps' returns, a hash of every vertex's position, the extent, the
geometry, and the text printed). Run it inside ``scripts/orchestration/boxed.sh`` (12 GiB: a
morph holds about 2 million nodes).

    python -P validation/probes/fs-gcam/real.py [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[2] / "validation" / "src"))

import check  # noqa: E402
from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402


def head_cases(ws: Path) -> list[str]:
    runs = ws / "oracles" / "freesurfer-8.2.0" / "runs"
    lines = []
    for run in sorted(p for p in runs.iterdir() if p.is_dir() and not p.name.endswith("1thread")):
        subj = next((run / "subjects").glob("sub-*"), None)
        if subj is None:
            continue
        mri, surf = subj / "mri", subj / "surf"
        xf = mri / "transforms"
        for hemi in ("lh", "rh"):
            s = surf / f"{hemi}.white"
            if not s.is_file():
                continue
            for t in ("talairach.m3z", "talairach.xfm", "cc_up.lta"):
                if (xf / t).is_file():
                    lines.append(f"msf {s} {xf / t} nn")
            for t in (xf / "talairach.lta", mri / "rawavg2orig.lta"):
                if t.is_file() and (mri / "orig.mgz").is_file():
                    lines.append(f"msf {s} {t} ss {mri / 'orig.mgz'}")
    return lines


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = check.probe_binary(ws)
    port = check.port_binary(a.jobs)
    inputs = ws / "oracles" / "inputs" / "fs-gcam"
    inputs.mkdir(parents=True, exist_ok=True)
    cases = head_cases(ws)
    cases_path = inputs / "real-heads.txt"
    cases_path.write_text("".join(c + "\n" for c in cases), encoding="utf-8")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    done = oracle_cache.run(
        [exe, cases_path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=7200,
        runner="fs-gcam-probe-real",
    )
    if done.returncode != 0:
        raise SystemExit(f"fsgcam failed ({done.returncode}): {done.stderr[-2000:]}")
    official = done.stdout.splitlines()
    rows = {}
    for mode in ("reproduce", "fix"):
        out = subprocess.run(
            [str(port), "run", str(cases_path), mode], check=True, capture_output=True, text=True
        ).stdout
        rows[mode] = [line.partition("\t") for line in out.splitlines()]
    assert len(official) == len(cases) == len(rows["reproduce"]) == len(rows["fix"])
    counts = Counter()
    notes = Counter()
    differ = []
    for case, c, (r, _, rids), (f, _, fids) in zip(
        cases, official, rows["reproduce"], rows["fix"], strict=True
    ):
        kind = Path(case.split()[2]).name
        counts[(kind, "reproduce", c == r)] += 1
        counts[(kind, "fix", c == f)] += 1
        for x in (rids + "," + fids).split(","):
            if x:
                notes[x] += 1
        if c != r and len(differ) < 5:
            differ.append({"case": case, "official": c[:500], "port": r[:500]})
    summary = {
        "cases": len(cases),
        "served": done.cached,
        "identical": {f"{k} {m}": n for (k, m, same), n in sorted(counts.items()) if same},
        "different": {f"{k} {m}": n for (k, m, same), n in sorted(counts.items()) if not same},
        "notices": dict(notes),
        "first_differ": differ,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
