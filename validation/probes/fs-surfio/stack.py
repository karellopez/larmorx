# SPDX-License-Identifier: Apache-2.0
"""The depth at which FreeSurfer 8.2.0's cluster growth overflows its stack (FS-901).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fssurfio
is a black-box run of FreeSurfer's code).

Writes flat grids of ``rows x cols`` vertices 0.1 mm apart (MRISread refuses coordinates beyond
10,000 mm; binary triangle files without a suffix, two triangles a cell) to ``<workspace>/oracles/inputs/fs-surfio/stack/``, then runs ``fssurfio``'s ``flr sgd``
on each (every vertex's value 1, one ``sclustMapSurfClusters``: one cluster of every vertex) by
the oracle-output cache at one thread, at the stack the probe inherits (8 MiB here), and the
port's ``flr sgd``, which also prints the deepest path of its growth (the nested calls
FreeSurfer's recursion makes); also each recorded head's white surfaces (``real.py``'s heads), one
cluster of the whole hemisphere. Prints one row per surface: the vertex count, the port's depth, the
official result (the cluster count and the cluster's vertex count, or its crash) and whether the
port's result is the same apart from the depth.

    python -P validation/probes/fs-surfio/stack.py [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import struct
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import check  # noqa: E402
from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402
from real import HEADS  # noqa: E402

# (rows, cols): strips first (their growth's path is about as long as the strip), then squares
GRIDS = [
    (2, 20000),
    (2, 50000),
    (2, 50500),
    (2, 51000),
    (2, 51250),
    (2, 51500),
    (2, 51750),
    (2, 52000),
    (2, 54000),
    (2, 80000),
    (3, 40000),
    (100, 1000),
    (200, 500),
    (300, 600),
    (400, 400),
]


def grid_bytes(rows: int, cols: int) -> bytes:
    b = bytearray(b"\xff\xff\xfe")
    b += b"created by stack.py\n\n"
    nv, nf = rows * cols, 2 * (rows - 1) * (cols - 1)
    b += struct.pack(">ii", nv, nf)
    for j in range(rows):
        for i in range(cols):
            b += struct.pack(">fff", i / 10, j / 10, 0.0)
    for j in range(rows - 1):
        for i in range(cols - 1):
            a = j * cols + i
            b += struct.pack(">iii", a, a + 1, a + cols)
            b += struct.pack(">iii", a + 1, a + cols + 1, a + cols)
    return bytes(b)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--json", type=Path)
    ap.add_argument("--jobs", type=int, default=6)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = check.probe_binary(ws)
    port = check.port_binary(a.jobs)
    inputs = ws / "oracles" / "inputs" / "fs-surfio" / "stack"
    inputs.mkdir(parents=True, exist_ok=True)
    lines = []
    names = []
    for rows, cols in GRIDS:
        # no suffix: MRISread takes a name's suffix as its format (`.tri` is the icosahedron's)
        # the spacing in the name: the cache keys the case file, which names the surfaces
        p = inputs / f"lh.grid-{rows}x{cols}-0.1mm"
        if not p.exists():
            p.write_bytes(grid_bytes(rows, cols))
        lines.append(f"flr sgd {p}")
        names.append((f"{rows}x{cols}", rows * cols))
    # the recorded heads' white surfaces: one cluster of the whole hemisphere (what
    # mrisMarkIntersections' hole filling and mri_label2label --annot-fill-holes cluster)
    for head in HEADS:
        sub = head.split("-", 1)[1]
        sd = ws / "oracles" / "freesurfer-8.2.0" / "runs" / f"{head}-classic" / "subjects" / sub
        for hemi in ("lh", "rh"):
            lines.append(f"flr sgd {sd / 'surf' / f'{hemi}.white'}")
            names.append((f"{sub} {hemi}.white", 0))
    cases = inputs / "stack-cases.txt"
    cases.write_text("".join(x + "\n" for x in lines), encoding="utf-8")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    env.update(freesurfer_env())
    done = oracle_cache.run(
        [exe, cases],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=7200,
        runner="fs-surfio-probe-stack",
    )
    if done.returncode != 0:
        raise SystemExit(f"fssurfio failed ({done.returncode}): {done.stderr[-2000:]}")
    official = done.stdout.splitlines()
    text = subprocess.run(
        [str(port), "run", str(cases), "reproduce"], check=True, capture_output=True, text=True
    ).stdout
    rows = []
    for (name, nv), o, p in zip(names, official, text.splitlines(), strict=True):
        res = p.partition("\t")[0].split(" | ")[0]
        depth = int(res.split("depth ")[1].split()[0])
        port_res = res.split(" depth ")[0]
        off = o.split(" | ")[0]
        rows.append(
            {
                "grid": name,
                "vertices": nv or int(port_res.split()[2]),
                "depth": depth,
                "stack_bytes": depth * 80,
                "official": off,
                "port": port_res,
                "same": off == port_res,
            }
        )
        print(
            f"{name:<26} {rows[-1]['vertices']:>7} vertices  depth {depth:>7} "
            f"({depth * 80:>9} bytes)  official {off:<16} port {port_res}"
        )
    if a.json:
        a.json.write_text(json.dumps(rows, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
