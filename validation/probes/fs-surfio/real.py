# SPDX-License-Identifier: Apache-2.0
"""mriflood.cpp's surface functions on the recorded heads: FreeSurfer 8.2.0's official code
against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fssurfio
is a black-box run of FreeSurfer's code).

Runs the calls of the pipeline's programs on three heads' recorded surfaces and volumes
(``<workspace>/oracles/freesurfer-8.2.0/runs/<head>-classic/subjects/<subject>/``) through
``fssurfio`` (its ``flr`` command) by the oracle-output cache at one thread and through the port
with ``reproduce`` and ``fix``: the interior of each hemisphere's white and pial surfaces in a copy
of ``aseg.mgz`` (``MRISfillInterior(surface, 0, volume)``, mri_surf2volseg's call), the ribbon
between each hemisphere's white and pial surfaces in ``orig.mgz``'s geometry (``MRISribbon``,
mris_volmask's), and the white surface's interior in its bounding volume at 0.5 mm
(``MRISmakeBoundingVolume`` then ``MRISfillInterior``, mris_place_surface's T2 and FLAIR steps).
Compares the result lines (extent, type, frames, geometry, the count of voxels that are not 0 and
the FNV-1a of the voxels). With ``--curv`` (relay 35 of track D): each head's recorded per-vertex
files (``CURVS``) read into its white surface by ``MRISreadCurvatureFile``, written by
``MRISwriteCurvature`` in the new binary format, MGZ, compressed NIfTI, GIFTI and VTK, the new-format
file compared with the recorded one byte for byte, and the MGZ and new-format files read back. Prints the counts; the lines name machine paths, so no hash is recorded
from them.

    python -P validation/probes/fs-surfio/real.py [--json out.json]
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
# relay 35 of track D: the per-vertex files recon-all writes with MRISwriteCurvature (the new binary
# format, and the MGH overlay of mri_segstats' grey/white contrast)
CURVS = (
    "curv",
    "sulc",
    "thickness",
    "area",
    "curv.pial",
    "jacobian_white",
    "volume",
    "w-g.pct.mgh",
)


def dx(v: float) -> str:
    return struct.pack(">d", v).hex()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument(
        "--curv",
        action="store_true",
        help="relay 35 of track D: the curvature files (flr crv): each head's recorded curvature "
        "files read into the white surface, written in five formats, read back",
    )
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = check.probe_binary(ws)
    port = check.port_binary(a.jobs)
    lines = []
    for head in HEADS:
        sub = head.split("-", 1)[1]
        s = ws / "oracles" / "freesurfer-8.2.0" / "runs" / f"{head}-classic" / "subjects" / sub
        for hemi in ("lh", "rh"):
            if a.curv:
                for c in CURVS:
                    lines.append(
                        f"flr crv {s / 'surf' / f'{hemi}.white'} {s / 'surf' / f'{hemi}.{c}'}"
                    )
                continue
            for surf in ("white", "pial"):
                lines.append(f"flr fil {s / 'surf' / f'{hemi}.{surf}'} {s / 'mri' / 'aseg.mgz'}")
            lines.append(
                f"flr rib {s / 'surf' / f'{hemi}.white'} {s / 'surf' / f'{hemi}.pial'} "
                f"{s / 'mri' / 'orig.mgz'}"
            )
            lines.append(f"flr bbf {s / 'surf' / f'{hemi}.white'} {dx(0.5)}")
    inputs = ws / "oracles" / "inputs" / "fs-surfio"
    inputs.mkdir(parents=True, exist_ok=True)
    cases_path = inputs / ("real-curv-cases.txt" if a.curv else "real-flood-cases.txt")
    cases_path.write_text("".join(x + "\n" for x in lines), encoding="utf-8")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    env.update(freesurfer_env())
    t0 = time.monotonic()
    done = oracle_cache.run(
        [exe, cases_path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=36000,
        runner="fs-surfio-probe-real",
    )
    t_official = time.monotonic() - t0
    if done.returncode != 0:
        raise SystemExit(f"fssurfio failed ({done.returncode}): {done.stderr[-2000:]}")
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
    notices: dict[str, int] = {}
    for case, c, (r, _, _), (f, _, ids) in zip(
        lines, official, rows["reproduce"][0], rows["fix"][0], strict=True
    ):
        for x in filter(None, ids.split(",")):
            notices[x] = notices.get(x, 0) + 1
        if c == r:
            same += 1
        else:
            differ += 1
            names = " ".join(Path(x).name for x in case.split()[2:])
            bad.append({"case": f"{case.split()[1]} {names}", "o": c[:2000], "p": r[:2000]})
        if f == c:
            fix_same += 1
    summary = {
        "cases": len(lines),
        "served": done.cached,
        "reproduce_identical": same,
        "reproduce_differ": differ,
        "fix_identical": fix_same,
        "fix_notices": notices,
        "seconds_official": round(t_official, 1),
        "seconds_port_reproduce": round(rows["reproduce"][1], 1),
        "seconds_port_fix": round(rows["fix"][1], 1),
        "differ": bad,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ else 0


if __name__ == "__main__":
    raise SystemExit(main())
