# SPDX-License-Identifier: Apache-2.0
"""mri_normalize's own functions on the recorded heads: FreeSurfer 8.2.0's official code against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsnormalize
is a black-box run of FreeSurfer's code).

Runs, on three heads' recorded volumes
(``<workspace>/oracles/freesurfer-8.2.0/runs/<head>-classic/subjects/<subject>/mri/``), the mask of
``-atlas`` (``build_outside_of_brain_mask``: ``nu.mgz`` through the head's ``talairach.lta`` into
FreeSurfer's ``RB_all_2020-01-02.gca``, brain distances 1 and 3) and ``-aseg``'s outlier removal
(``MRIremoveWMOutliersAndRetainMedialSurface`` on ``norm.mgz`` with the white matter of
``aseg.presurf.mgz``, as recon-all's ``-mprage`` gives it, 15, and the default 10) through
``fsnormalize`` by the oracle-output cache at one thread and through the port with ``reproduce``
and ``fix``; compares the result lines (extent, type, count of voxels above 0, FNV-1a of the
voxels, and the text printed). The lines name machine paths, so no hash is recorded from them.

    python -P validation/probes/fs-normalize/real.py [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import check  # noqa: E402
from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

HEADS = ("ds000005-sub-01", "ds000228-sub-pixar008", "ds002422-sub-02")
ATLAS = "root/usr/local/freesurfer/8.2.0/average/RB_all_2020-01-02.gca"


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
    fs = ws / "oracles" / "freesurfer-8.2.0"
    lines = []
    for head in HEADS:
        sub = head.split("-", 1)[1]
        m = fs / "runs" / f"{head}-classic" / "subjects" / sub / "mri"
        for whalf in (1, 3):
            lines.append(
                f"bobmf {m / 'nu.mgz'} {fs / ATLAS} {m / 'transforms' / 'talairach.lta'} {whalf}"
            )
        for ib in (15, 10):
            lines.append(f"wmomf {m / 'norm.mgz'} {m / 'aseg.presurf.mgz'} {ib}")
    inputs = ws / "oracles" / "inputs" / "fs-normalize"
    inputs.mkdir(parents=True, exist_ok=True)
    cases_path = inputs / "real-cases.txt"
    cases_path.write_text("".join(x + "\n" for x in lines), encoding="utf-8")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    env.update(freesurfer_env())
    t0 = time.monotonic()
    done = oracle_cache.run(
        [exe, cases_path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=36000,
        runner="fs-normalize-probe-real",
    )
    t_official = time.monotonic() - t0
    if done.returncode != 0:
        raise SystemExit(f"fsnormalize failed ({done.returncode}): {done.stderr[-2000:]}")
    official = done.stdout.splitlines()
    rows = {}
    for mode in ("reproduce", "fix"):
        t0 = time.monotonic()
        text = subprocess.run(
            [str(port), "run", str(cases_path), mode], check=True, capture_output=True, text=True
        ).stdout
        rows[mode] = ([x.partition("\t") for x in text.splitlines()], time.monotonic() - t0)
    same = differ = fix_same = 0
    bad = []
    for case, c, (r, _, _), (f, _, _) in zip(
        lines, official, rows["reproduce"][0], rows["fix"][0], strict=True
    ):
        if c == r:
            same += 1
        else:
            differ += 1
            bad.append({"case": case, "o": c[:800], "p": r[:800]})
        if f == c:
            fix_same += 1
    summary = {
        "cases": len(lines),
        "served": done.cached,
        "reproduce_identical": same,
        "reproduce_differ": differ,
        "fix_identical": fix_same,
        "seconds_official": round(t_official, 1),
        "seconds_port_reproduce": round(rows["reproduce"][1], 1),
        "differ": bad,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ else 0


if __name__ == "__main__":
    raise SystemExit(main())
