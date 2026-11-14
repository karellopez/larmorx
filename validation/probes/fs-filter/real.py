# SPDX-License-Identifier: Apache-2.0
"""fs.filter on the recorded heads: FreeSurfer 8.2.0's official code against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsfilter
is a black-box run of FreeSurfer's code).

Runs the distance maps the pipeline's programs compute (mri_fill's and the surface placement's
signed maps of the white matter, the cortex and the filled hemispheres, mriset's unsigned map,
mris_distance_to_label's outside and inside maps) on three heads' recorded volumes
(``<workspace>/oracles/freesurfer-8.2.0/runs/<head>-classic/subjects/<subject>/mri/``) through
``fsfilter`` by the oracle-output cache at one thread and through the port with ``reproduce`` and
``fix``, and compares the result lines (extent, type, frames, outside value, column size and the
FNV-1a digest of the voxels). Prints the counts; the lines name machine paths, so no hash is
recorded from them.

    python -P validation/probes/fs-filter/real.py [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import struct
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
# (command, file, label, max distance, mode): MRIdistanceTransform's modes are DTRANS_MODE_*,
# MRIextractDistanceMap's its own
CALLS = (
    ("dtrf", "aseg.mgz", 2, 100.0, 1),
    ("dtrf", "aseg.mgz", 3, 100.0, 1),
    ("dtrf", "filled.mgz", 255, 100.0, 1),
    ("dtrf", "wm.mgz", 110, -1.0, 2),
    ("edmf", "aseg.mgz", 17, 5.0, 1),
    ("edmf", "aseg.mgz", 4, 10.0, 2),
)


def fx(v: float) -> str:
    return struct.pack(">f", v).hex()


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
    lines = []
    for head in HEADS:
        sub = head.split("-", 1)[1]
        mri = ws / "oracles" / "freesurfer-8.2.0" / "runs" / f"{head}-classic" / "subjects" / sub
        for cmd, name, label, maxd, mode in CALLS:
            lines.append(f"{cmd} {mri / 'mri' / name} {label} {fx(maxd)} {mode}")
    inputs = ws / "oracles" / "inputs" / "fs-filter"
    inputs.mkdir(parents=True, exist_ok=True)
    cases_path = inputs / "real-cases.txt"
    cases_path.write_text("".join(x + "\n" for x in lines), encoding="utf-8")
    # MRIread runs FreeSurfer's licence check: FREESURFER_HOME and FS_LICENSE (read in place)
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    env.update(freesurfer_env())
    t0 = time.monotonic()
    done = oracle_cache.run(
        [exe, cases_path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=36000,
        runner="fs-filter-probe-real",
    )
    t_official = time.monotonic() - t0
    if done.returncode != 0:
        raise SystemExit(f"fsfilter failed ({done.returncode}): {done.stderr[-2000:]}")
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
        # the outputs only (the text FreeSurfer printed is compared too)
        if c == r:
            same += 1
        else:
            differ += 1
            bad.append({"case": case.split()[0] + " " + Path(case.split()[1]).name, "o": c, "p": r})
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
